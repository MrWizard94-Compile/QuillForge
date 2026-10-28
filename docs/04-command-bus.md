# Command Bus Specification

**Document ID:** QF-CMD-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Role

The command bus translates **intent** into **operations** under validation and authorization. It is the only supported ingress for mutations that will enter the journal.

```
Intent sources → Command → [validate] → [authorize] → [resolve targets]
    → CommandHandler::execute → Vec<Operation> → CommitCoordinator → Journal → Kernel
```

---

## 2. Required trait

```rust
pub trait CommandHandler: Send + Sync {
    fn execute(&self, command: Command) -> Result<Vec<Operation>, CommandError>;
}
```

Handlers are pure with respect to durable state: they read canonical state via read-only handles and produce ops. They MUST NOT write the journal or mutate kernel state.

---

## 3. Command model

```rust
pub struct CommandEnvelope {
    pub command_id: CommandId,
    pub document_id: DocumentId,
    pub actor: ActorContext,
    pub issued_at: Timestamp,      // not used in apply
    pub client_meta: ClientMeta,   // UI selection snapshot, locale (non-authoritative)
    pub command: Command,
}

pub struct ActorContext {
    pub actor_id: ActorId,
    pub session_id: SessionId,
    pub roles: Vec<Role>,
    pub capabilities: CapabilitySet, // effective grants for this action
}

pub enum Command {
    // Text
    InsertText { target: TextTarget, text: String },
    DeleteRange { range: DocRange },
    ReplaceRange { range: DocRange, text: String },

    // Structure
    InsertBlock { parent: ParentRef, index: usize, block: BlockSpec },
    DeleteBlock { block_id: BlockId },
    MoveBlock { block_id: BlockId, new_parent: ParentRef, index: usize },
    SplitBlock { block_id: BlockId, offset: TextOffset },
    MergeBlocks { left: BlockId, right: BlockId },

    // Marks
    ApplyMark { range: DocRange, mark: Mark },
    RemoveMark { range: DocRange, mark_kind: MarkKind },

    // Comments / suggestions / review
    AddComment { anchor: Anchor, body: String },
    ResolveComment { comment_id: CommentId },
    CreateSuggestion { ops: Vec<Operation>, note: Option<String> },
    AcceptSuggestion { suggestion_id: SuggestionId },
    RejectSuggestion { suggestion_id: SuggestionId },

    // History
    Undo { steps: u32 },
    Redo { steps: u32 },

    // Assets / sources
    AttachAsset { asset: AssetRef, at: ParentRef },
    DetachAsset { asset_id: AssetId },

    // Admin / package (still go through commit where they affect canonical)
    SetDocumentMeta { patch: MetaPatch },

    // Plugin-originated (already capability-checked at host)
    PluginCommand { plugin_id: PluginId, name: String, payload: Vec<u8> },

    // AI accept only (payload is suggestion id, not free-form silent edit)
    AcceptAiSuggestion { suggestion_id: SuggestionId },
}
```

---

## 4. Pipeline stages

### 4.1 Validation

| Check | Failure |
|-------|---------|
| Schema of command fields | `CommandError::Invalid` |
| Document open and writable | `CommandError::NotWritable` |
| Selection/range within bounds | `CommandError::InvalidTarget` |
| Block types allow operation | `CommandError::TypeMismatch` |
| Text UTF-8 scalar safety | `CommandError::InvalidText` |
| Undo stack depth available | `CommandError::NothingToUndo` |

Validation is **fail-closed**. No partial ops on validation failure.

### 4.2 Authorization

| Check | Failure |
|-------|---------|
| Actor has role for mutation class | `CommandError::Forbidden` |
| Region locks (enterprise) | `CommandError::Locked` |
| Capability for plugin/AI path | `CommandError::MissingCapability` |
| Suggestion accept permissions | `CommandError::Forbidden` |

Authorization events SHOULD be audit-logged (without document body by default).

### 4.3 Target resolution

Resolve UI-facing anchors into stable IDs:

```rust
pub struct TextTarget {
    pub block_id: BlockId,
    pub span_id: Option<SpanId>,
    pub offset: TextOffset, // UTF-8 byte or Unicode scalar — CHOSEN: Unicode scalar offset
}

pub struct DocRange {
    pub start: TextTarget,
    pub end: TextTarget,
}
```

**Decision:** Text offsets are **Unicode scalar value (char) offsets** within the concatenated visible text of the target span sequence for the block. Justification: stability across UTF-8 representation and alignment with editing UX. Stored ops MUST use the same definition.

Resolution maps:

- Caret → `(block_id, span_id, offset)`  
- Block index paths → `ParentRef` + index  
- Stale IDs → `CommandError::StaleTarget` (client refreshes selection)

### 4.4 Emitted operations

Handlers emit fully specified operations (all new IDs pre-generated):

```rust
pub enum Operation {
    InsertText { block_id: BlockId, at: TextOffset, text: String },
    DeleteText { block_id: BlockId, range: OffsetRange },
    InsertBlock { block_id: BlockId, parent: ParentRef, index: usize, body: BlockBody },
    RemoveBlock { block_id: BlockId },
    MoveBlock { block_id: BlockId, new_parent: ParentRef, index: usize },
    SetMarks { span_id: SpanId, marks: Vec<Mark> },
    UpdateBlockMeta { block_id: BlockId, meta: BlockMetadata },
    UpsertComment { comment: Comment },
    DeleteComment { comment_id: CommentId },
    UpsertSuggestion { suggestion: Suggestion },
    ApplySuggestionOps { suggestion_id: SuggestionId }, // kernel resolves a stored Accepted proposal only
    SetDocumentMeta { patch: MetaPatch },
    // Inverse markers are normal ops; no special non-journaled undo
    Noop { reason: NoopReason },
}
```

Handlers MAY emit multiple ops. Ordering in the vector is apply order.

### 4.5 Failure behavior

| Stage | Journal written? | Kernel applied? | UI effect |
|-------|------------------|-----------------|-----------|
| Validation fail | No | No | Error toast |
| Auth fail | No | No | Error + audit |
| Handler error | No | No | Error |
| Dry-run apply fail | No | No | Error (stale?) |
| Journal fail | No | No | Hard error, retry |
| Live apply fail | Yes | No | **Recovery mode** (critical) |

### 4.6 Serialization and replay concerns

| Concern | Rule |
|---------|------|
| Commands vs ops | Only **operations** are journaled for replay. Commands are not required for recovery. |
| Command log | Optional debug telemetry; never source of truth |
| IDs in ops | All entity IDs created at command handling time |
| Floats | Forbidden in ops that affect layout-critical content identity; use rationals or fixed integers where needed |
| Map iteration | Serialize maps with sorted keys |
| Plugin payloads | Opaque bytes with type tags; preserved |

**Replay** never re-executes command handlers. Replay only applies journaled operations.

---

## 5. Command bus API

```rust
pub struct CommandBus {
    handlers: HashMap<TypeId, Arc<dyn CommandHandler>>,
    // or enum dispatch table
}

pub struct CommitCoordinator {
    journal: JournalHandle,
    kernel: KernelHandle,
    undo: UndoService,
}

impl CommitCoordinator {
    pub fn commit(&mut self, env: CommandEnvelope) -> Result<CommitResult, CommandError> {
        // 1. auth + validate
        // 2. handler.execute
        // 3. dry_run apply
        // 4. journal.append_batch
        // 5. kernel.apply_batch
        // 6. undo.record_forward_ops
        // 7. return CommitResult { op_seq_range, events }
    }
}

pub struct CommitResult {
    pub first_seq: OpSeq,
    pub last_seq: OpSeq,
    pub events: Vec<KernelEvent>,
}
```

---

## 6. Undo / redo integration

```rust
pub struct UndoService {
    // stacks store op batches (forward), not full document snapshots
    undo_stack: Vec<OpBatch>,
    redo_stack: Vec<OpBatch>,
}

impl UndoService {
    pub fn inverse_batch(state_before: &CanonicalState, forward: &[Operation]) -> Vec<Operation>;
}
```

**Rules:**

- Undo command → compute inverses against state, commit as new journal ops  
- After undo, push original forward batch to redo stack (memory); durable form is the inverse ops in journal  
- New edit after undo clears redo stack  
- Inverse generation MUST be deterministic  

---

## 7. Handler registration map (v1)

| Command class | Handler module |
|---------------|----------------|
| Text insert/delete/replace | `qf-doc` / editing handlers |
| Block structure | `qf-doc` |
| Marks | `qf-doc` |
| Comments/suggestions | `qf-collab` handlers |
| Undo/redo | `qf-core` undo |
| Assets | `qf-storage` + doc handlers |
| PluginCommand | `qf-plugin` host expansion |
| AcceptAiSuggestion | `qf-ai` + collab suggestion accept |

---

## 8. Concurrency

- One `CommitCoordinator` per open document.  
- Commands are queued FIFO per document.  
- Parallel command execution across different documents is allowed.  

---

## 9. Testing requirements (command bus)

1. Every command type has golden tests: input command + state → expected ops.  
2. Auth matrix tests for roles.  
3. Stale target rejection.  
4. Undo inverse round-trip state hash.  
5. Journal never written on validation failure (fault injection).  

---

*End of command bus specification.*
