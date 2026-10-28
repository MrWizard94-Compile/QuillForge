# Kernel and Journal Constitution

**Document ID:** QF-CONST-001  
**Version:** 1.0.0  
**Status:** Binding — constitutional  

This document has the force of a constitution for implementers. Violations are release blockers.

---

## 1. Purpose

Define the immutable laws governing:

- What is canonical state  
- How operations are recorded  
- How the kernel applies them  
- How undo, recovery, and audit interact with both  

---

## 2. Definitions

| Term | Definition |
|------|------------|
| **Operation** | Smallest pure, serializable mutation unit applied by the kernel |
| **Command** | User/system intent that may expand to zero or more operations |
| **Journal** | Append-only, durable log of committed operations for a document |
| **Kernel** | Sole applier of operations to canonical document state |
| **Canonical state** | The document AST + associated entity tables after applying ops 1..=N |
| **Checkpoint** | Durable snapshot of canonical state at a known `OpSeq` |
| **Commit** | Atomic act of durability-appending ops and applying them in order |

---

## 3. Journal constitution

### 3.1 Immutability

**J1:** Once an operation record is committed, its bytes are immutable.  
**J2:** Journals MUST NOT rewrite, insert mid-stream, or delete historical ops.  
**J3:** Compaction MAY create a new checkpoint and a truncated journal **only** by writing a new package generation and retaining the old generation until success (see recovery). Compaction does not mutate historical truth; it supersedes with a verified new baseline.

### 3.2 Append semantics

**J4:** Append is strictly sequential per document: `OpSeq = last + 1`.  
**J5:** A commit MAY contain a batch of ops that share one commit header; individual ops still receive consecutive `OpSeq` values.  
**J6:** Append durability requires: write payload → write commit record → `fsync` journal file (and parent dir on platforms that require it) **before** acknowledging success to the caller.

### 3.3 Operation record schema

```rust
pub struct JournalRecord {
    pub schema_version: u32,          // journal record schema, not package version
    pub op_id: OpId,
    pub op_seq: OpSeq,
    pub document_id: DocumentId,
    pub actor_id: ActorId,
    pub session_id: SessionId,
    pub committed_at: Timestamp,      // audit only; not used in apply logic
    pub parents: Vec<OpSeq>,          // local: [prev]; multi-replica: causal parents
    pub content_hash: Hash256,        // hash of op payload bytes
    pub prev_record_hash: Hash256,    // hash-chain
    pub payload: Operation,           // enum, versioned
    pub attribution: Attribution,     // optional human label, device id hash
}

pub struct CommitBatch {
    pub batch_id: Uuid,
    pub first_seq: OpSeq,
    pub last_seq: OpSeq,
    pub record_hashes: Vec<Hash256>,
    pub batch_hash: Hash256,
}
```

**J7:** Hash-chain integrity: `prev_record_hash` of op N MUST equal hash of record N-1 (or genesis sentinel for seq 1).  
**J8:** On open, verify hash chain for all records after last checkpoint (minimum). Full-chain verify on explicit integrity check.

### 3.4 Replay semantics

**J9:** Replay is pure: `fold(checkpoint_state, ops[checkpoint.seq+1 ..= N]) → state_N`.  
**J10:** Replay MUST ignore wall-clock fields for apply decisions.  
**J11:** Replay MUST be deterministic across machines given identical checkpoint + ops + schema version.  
**J12:** If apply fails mid-replay, recovery MUST NOT mark package healthy; surface corruption.

### 3.5 Checkpoints

```rust
pub struct Checkpoint {
    pub checkpoint_id: Uuid,
    pub at_op_seq: OpSeq,
    pub document_snapshot: DocumentSnapshot, // canonical AST + entity tables
    pub snapshot_hash: Hash256,
    pub created_at: Timestamp,
    pub package_format_version: Version,
}
```

**J13:** Checkpoints are optional optimizations for open latency, not sources of operations.  
**J14:** A checkpoint is valid only if `snapshot_hash` matches recomputed hash of snapshot bytes.  
**J15:** After loading checkpoint at seq S, journal ops with `op_seq > S` MUST be replayed in order.  
**J16:** Checkpoint creation MUST fsync snapshot, then fsync metadata pointer, without deleting prior checkpoint until new one is verified.

### 3.6 Branching

**J17 (v1 choice):** Linear primary journal per document replica. Collaboration concurrency is expressed via CRDT/merge ops on the linear log after merge, not unlimited git-like branches in the primary package.

**J18:** Experimental local branches (e.g. “suggestion branch”) MAY exist as **separate suggestion entities** or staging journals, but promotion into primary journal occurs only via command→ops on the primary path.

**Justification:** Linear journal maximizes recovery simplicity and auditability (priorities 1–3). Full multi-branch package history is post-v1 if needed.

### 3.7 Auditability

**J19:** Every committed op retains actor, session, and content hash.  
**J20:** Audit export MAY redact payloads under policy but MUST retain op identity and actor for enterprise modes.  
**J21:** Journal is the audit source of truth for content mutations.

### 3.8 Durability guarantees

| Guarantee | Requirement |
|-----------|-------------|
| Committed op durability | Survives power loss after successful commit return |
| Ordering | `OpSeq` total order on a replica |
| Atomic multi-op commit | Batch either fully durable or not acknowledged |
| No torn records | Length-prefix + checksum per record |

---

## 4. Kernel constitution

### 4.1 Sole authority

**K1:** Only the kernel applies operations to canonical state.  
**K2:** Kernel apply is the only writer of `Document` and entity tables (comments, suggestions, revisions, assets refs).  
**K3:** Kernel MUST refuse ops that violate invariants; refusal after journal commit is a critical failure (see recovery).

### 4.2 Canonical state

```rust
pub struct CanonicalState {
    pub document_id: DocumentId,
    pub package_format_version: Version,
    pub op_seq: OpSeq,
    pub document: Document,
    pub entities: EntityTables,
    pub integrity: StateIntegrityMeta,
}

pub struct EntityTables {
    pub comments: BTreeMap<CommentId, Comment>,
    pub suggestions: BTreeMap<SuggestionId, Suggestion>,
    pub revisions: BTreeMap<RevisionId, Revision>,
    pub assets: BTreeMap<AssetId, AssetRef>,
    pub sources: BTreeMap<SourceId, SourceRef>,
}
```

### 4.3 Apply semantics

```rust
pub trait Kernel {
    fn state(&self) -> &CanonicalState;
    fn apply(&mut self, op: &Operation) -> Result<Vec<KernelEvent>, ApplyError>;
    fn apply_batch(&mut self, ops: &[Operation]) -> Result<Vec<KernelEvent>, ApplyError>;
}
```

**K4:** `apply` is transactional in memory: on error, state unchanged for that op.  
**K5:** Successful apply increments `op_seq` to match the applied op.  
**K6:** Apply MUST be a pure function of `(state, op)` — no I/O, no RNG, no clock.  
**K7:** ID allocation needed by ops MUST be present in the op payload (generated at command time), never by kernel RNG during apply.

### 4.4 Snapshot semantics

**K8:** `DocumentSnapshot` is a complete serializable freeze of `CanonicalState` at `op_seq`.  
**K9:** Snapshot serialization MUST preserve unknown fields/blocks (see unknown-content spec).  
**K10:** Snapshots used for checkpoints MUST be bit-stable for identical state.

### 4.5 Read models

**K11:** Kernel MAY expose read-only projections (`get_block`, `iter_sections`).  
**K12:** Callers MUST NOT hold mutable references that allow out-of-band mutation.  
**K13:** `state_gen` equals current `op_seq` for cache coherence.

### 4.6 Kernel invariants (MUST hold after every successful apply)

| ID | Invariant |
|----|-----------|
| KI-01 | All `BlockId`s unique within document |
| KI-02 | All spans belong to exactly one block |
| KI-03 | Section tree is acyclic |
| KI-04 | Parent/child block references resolve or are tombstoned consistently |
| KI-05 | Marks ranges lie within span bounds |
| KI-06 | Comment anchors resolve to existing or intentionally orphaned with flag |
| KI-07 | Asset references either resolve or are marked missing (not deleted silently) |
| KI-08 | `UnknownBlock.original_type` and `original_payload` unchanged unless explicit replace op |
| KI-09 | Suggestion application only via dedicated accept ops; `ApplySuggestionOps` resolves only a stored **Accepted** proposal and rejects nested proposal markers |
| KI-10 | No mark refers to missing comment/citation without orphan policy |

### 4.7 Validation rules at apply time

- Structural: IDs exist, parents valid, indices in range  
- Type: operation matches target block kind (or is generic structural op)  
- Policy: certain ops forbidden on locked regions (enterprise)  
- Unknown: ops targeting unknown blocks limited to structural move/delete/wrap; payload edits require plugin or raw replace with explicit op  

### 4.8 Failure behavior

| Case | Behavior |
|------|----------|
| Invalid op before journal | Command rejects |
| Invalid op after journal (bug) | Panic path: freeze writes, recovery mode |
| Invariant break detected | `ApplyError::Invariant`; no partial state |

---

## 5. Interaction laws (command / journal / kernel)

**C1:** Command handlers emit ops; they never call `kernel.apply` directly in production commit path without going through the commit coordinator.  

**Commit coordinator sequence (normative):**

1. Validate + authorize command  
2. Expand to `Vec<Operation>`  
3. **Dry-run apply** on a copy or transactional shadow (preferred)  
4. If dry-run OK → journal durable append  
5. Apply to live kernel (must succeed if dry-run pure)  
6. Fan out events  

**C2:** Dry-run is required for v1 to prevent journal poison. Dry-run uses the same pure apply function.

**C3:** Undo emits **inverse operations** (or explicit undo ops that are themselves journaled). Undo does not truncate the journal. Inverse restoration of a removed block preserves its complete typed body, spans/marks/citation, placement, and block metadata.

```rust
// Undo is a new commit of inverse ops — journal grows, history preserved
Command::Undo { steps: 1 } → Vec<Operation> // inverses
```

**C4:** Redo reapplies previously undone forward ops via journaled ops, not by moving a pointer into forgotten history as sole source (pointer may exist as UI state, but durability is still ops).

---

## 6. Event fanout (non-canonical)

```rust
pub enum KernelEvent {
    DocumentHydrated { op_seq: OpSeq },
    BlockInserted { id: BlockId, parent: ParentRef, index: usize },
    BlockRemoved { id: BlockId },
    BlockMoved { id: BlockId, new_parent: ParentRef, index: usize },
    TextChanged { block: BlockId, span: SpanId },
    MarksChanged { span: SpanId },
    CommentChanged { id: CommentId },
    SuggestionChanged { id: SuggestionId },
    UnknownBlockPreserved { id: BlockId, original_type: String },
    CheckpointHint { op_seq: OpSeq }, // advisory
}
```

**E1:** Events are not durable truth; journal is.  
**E2:** Subscribers MUST tolerate replay (idempotent handling with `op_seq`).

---

## 7. Forbidden acts (summary)

| Actor | Forbidden |
|-------|-----------|
| UI | Mutate AST fields |
| Search | Write document body |
| Layout/Render | Write journal |
| Sync | Apply remote ops without command/merge pipeline |
| Plugin | Open journal files |
| AI | Call apply or invent commits without user accept command |
| Storage | Rewrite historical journal records in place |

---

## 8. Constitutional tests (must exist)

1. **Replay equality:** random op stream → state hash == replay from genesis.  
2. **Power-loss commit:** kill after fsync → op present; kill before fsync → op absent.  
3. **Undo correctness:** undo inverses restore prior state hash.  
4. **Unknown preserve:** ops around unknown blocks leave payload bytes identical.  
5. **Hash chain:** tamper detection on journal.  

---

*End of constitution.*
