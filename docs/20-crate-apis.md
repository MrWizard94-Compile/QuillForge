# Module-by-Module API Outline

**Document ID:** QF-API-001  
**Version:** 1.0.0  
**Status:** Binding  

For each crate: purpose, responsibilities, main types/traits, dependencies, API shape, invariants, errors.

All crates: `version = "0.0.0"`, `publish = false`.

---

## 1. `qf-core`

**Purpose:** Shared primitive types, errors, IDs, versioning, command/operation enums, preserved maps.

**Responsibilities:**

- ID types, hashing, timestamps  
- `Command`, `Operation`, `CommandHandler` trait  
- `Version`, `Hash256`, `PreservedMap`  
- Common error traits  
- Undo batch types  

**Dependencies:** `uuid`, `thiserror`, `serde`, `bytes` — no other `qf-*`.

```rust
pub trait CommandHandler: Send + Sync {
    fn execute(&self, command: Command) -> Result<Vec<Operation>, CommandError>;
}

pub trait IdGen {
    fn new_uuid(&mut self) -> Uuid;
}

pub struct OpSeq(pub u64);
pub struct Hash256(pub [u8; 32]);
pub struct Version { pub major: u16, pub minor: u16, pub patch: u16 }

pub enum CommandError {
    Invalid(String),
    InvalidTarget,
    StaleTarget,
    Forbidden,
    Locked,
    MissingCapability,
    NotWritable,
    NothingToUndo,
    NothingToRedo,
    Handler(String),
    DryRun(ApplyError),
    Journal(JournalError),
    Internal(String),
}
```

**Invariants:** Ops fully specify new IDs; no ambient RNG in apply path types.

**Read/write:** Pure types only.

---

## 2. `qf-blocks`

**Purpose:** Native block body types + `UnknownBlock`.

**Responsibilities:** Block enums, mark types, validation helpers.

**Dependencies:** `qf-core`.

```rust
pub enum BlockBody { /* see AST */ }
pub struct UnknownBlock {
    pub original_type: String,
    pub original_payload: Vec<u8>,
    pub metadata: BlockMetadata,
}
pub fn validate_block_body(body: &BlockBody) -> Result<(), ValidationError>;
```

**Invariant U1:** Unknown encode/decode identity.

---

## 3. `qf-doc`

**Purpose:** Document AST structures and editing command handlers.

**Responsibilities:**

- `Document`, `Section`, `Block`, `Span`  
- Text projection / offsets  
- Handlers for insert/delete/marks/structure  

**Dependencies:** `qf-core`, `qf-blocks`.

```rust
pub struct Document { /* ... */ }
pub struct TextProjection<'a> { /* ... */ }
pub fn project_block_text(block: &Block) -> String;

pub struct EditingHandlers { /* IdGen + read state */ }
impl CommandHandler for EditingHandlers { /* ... */ }
```

**Errors:** `DocError::Invariant`, `DocError::OffsetOutOfRange`.

---

## 4. `qf-journal`

**Purpose:** Append-only durable operation log.

**Responsibilities:** Framing, CRC, hash chain, fsync append, scan/replay iterator.

**Dependencies:** `qf-core`.

```rust
pub struct Journal {
    path: PathBuf,
    last_seq: OpSeq,
    last_hash: Hash256,
}

impl Journal {
    pub fn open_or_create(path: &Path) -> Result<Self, JournalError>;
    pub fn append_batch(&mut self, records: &[JournalRecord]) -> Result<(), JournalError>;
    pub fn iter_from(&self, seq: OpSeq) -> impl Iterator<Item = Result<JournalRecord, JournalError>>;
    pub fn verify_chain(&self) -> Result<(), JournalError>;
    pub fn last_seq(&self) -> OpSeq;
}

pub struct JournalRecord { /* constitution */ }
```

**Invariants:** J1–J8; append durability before `Ok(())`.

**Write boundary:** Only journal crate writes journal files.

---

## 5. `qf-kernel`

**Purpose:** Apply operations; own canonical state.

**Dependencies:** `qf-core`, `qf-doc`, `qf-blocks`, `qf-journal` (types only for records; apply pure).

```rust
pub struct Kernel {
    state: CanonicalState,
}

impl Kernel {
    pub fn new(state: CanonicalState) -> Self;
    pub fn state(&self) -> &CanonicalState;
    pub fn apply(&mut self, op: &Operation) -> Result<Vec<KernelEvent>, ApplyError>;
    pub fn apply_batch(&mut self, ops: &[Operation]) -> Result<Vec<KernelEvent>, ApplyError>;
    pub fn snapshot(&self) -> DocumentSnapshot;
    pub fn from_snapshot(snap: DocumentSnapshot) -> Result<Self, ApplyError>;
}

pub enum ApplyError {
    Invariant(String),
    MissingEntity,
    TypeMismatch,
    OffsetOutOfRange,
    Schema,
}
```

**Invariant:** Apply pure; no I/O.

---

## 6. `qf-storage`

**Purpose:** `.qfdoc` packaging, atomic saves, open/recovery orchestration, migration runner.

**Dependencies:** `qf-core`, `qf-journal`, `qf-kernel`, `qf-doc`, `qf-security` (optional encrypt).

```rust
pub trait Migration: Send + Sync {
    fn from_version(&self) -> Version;
    fn to_version(&self) -> Version;
    fn migrate(&self, input: Package) -> Result<Package, MigrationError>;
}

pub struct PackageStore { /* ... */ }
pub struct RecoveryService { /* ... */ }
pub struct Migrator { /* ... */ }

impl PackageStore {
    pub fn create(&self, path: &Path, meta: PackageMetadata) -> Result<DocumentSession>;
    pub fn open(&self, path: &Path, opts: OpenOptions) -> Result<OpenOutcome>;
    pub fn save(&self, session: &mut DocumentSession) -> Result<()>;
}
```

**Errors:** `StorageError::Corrupt`, `Io`, `Migration`, `Encryption`.

---

## 7. `qf-layout`

**Purpose:** Deterministic layout/pagination.

**Dependencies:** `qf-core`, `qf-doc`, `qf-blocks`.

```rust
pub struct LayoutEngine { /* cache */ }
impl LayoutEngine {
    pub fn full_layout(&mut self, state: &CanonicalState, cfg: &LayoutConfig) -> LayoutTree;
    pub fn reflow_dirty(&mut self, state: &CanonicalState, cfg: &LayoutConfig, dirty: DirtySet) -> LayoutTree;
}
```

**Invariant:** Pure w.r.t. system time; fixed-point math.

---

## 8. `qf-gpu`

**Purpose:** Document GPU abstraction — **software RGBA8 framebuffer** (headless). Native `wgpu` is roadmap, not shipped.

**Dependencies:** `thiserror` (no `wgpu`).

```rust
pub struct GpuContext { /* framebuffer + atlases */ }
impl GpuContext {
    pub fn headless(w: u32, h: u32) -> Result<Self, GpuError>;
    pub fn begin_frame(&mut self) -> Result<FrameEncoder<'_>, GpuError>;
    pub fn framebuffer_rgba(&self) -> &[u8];
}
pub struct FrameEncoder<'a> { /* writes pixels */ }
pub struct GlyphAtlas { /* cell occupancy */ }
pub struct ImageAtlas { /* id → RGBA upload */ }
```

**Errors:** `GpuError::{DeviceLost, Init, OutOfMemory, MissingTexture, Other}`.

---

## 9. `qf-render`

**Purpose:** Derived render scheduler — viewport cull, dirty tiles, compose into `qf-gpu`.

**Dependencies:** `qf-layout`, `qf-gpu`, `qf-theme`, `qf-core`.

```rust
pub struct RenderScheduler { /* gpu, layout, dirty, stats */ }
impl RenderScheduler {
    pub fn set_layout(&mut self, layout: Arc<LayoutTree>);
    pub fn render_frame(&mut self, viewport: Viewport) -> Result<FrameStats, GpuError>;
}
```

Interactive shell (`qf-app`) uses egui; this crate is the pure composition path for tests/future surfaces.

---

## 10. `qf-input`

**Purpose:** Translate OS input to commands (basic keys/text/pointer). **IME not shipped.**

**Dependencies:** `qf-core`.

```rust
pub struct InputState { /* selection + caret */ }
impl InputState {
    pub fn on_key(&mut self, ev: KeyEvent) -> Vec<Command>;
    pub fn on_text(&mut self, s: &str) -> Vec<Command>;
    pub fn on_pointer(&mut self, ev: PointerEvent) -> Vec<Command>;
}
```

**Invariant:** No kernel mutation.

---

## 11. `qf-search`

**Purpose:** Derived search indexes and queries.

**Dependencies:** `qf-core`, `qf-doc`, `qf-kernel` (events/state read).

```rust
pub trait SearchIngestor { /* ... */ }
pub struct SearchEngine { /* ... */ }
impl SearchEngine {
    pub fn query_token(&self, q: TokenQuery) -> Result<Vec<SearchHit>>;
    pub fn query_graph(&self, q: GraphQuery) -> Result<Vec<GraphHit>>;
    pub fn query_semantic(&self, q: SemanticQuery) -> Result<Vec<SearchHit>>;
}
```

---

## 12. `qf-sync`

**Purpose:** Offline queue, protocol client, merge ingress.

**Dependencies:** `qf-core`, `qf-journal`, `qf-collab`, `qf-security`.

```rust
pub struct SyncClient { /* ... */ }
pub struct SyncQueue { /* ... */ }
pub trait MergeEngine { /* ... */ }
```

---

## 13. `qf-collab`

**Purpose:** Presence broker, suggestion types, RGA text CRDT. Foundation collab — not full multiplayer UX.

**Dependencies:** `qf-core`.

```rust
pub struct PresenceBroker { /* ... */ }
// Suggestion / SuggestionStatus / SuggestionSource live here for wire + AI.
pub struct TextCrdt { /* RGA; merge_crdt; to_replace_ops */ }
// Note: Command expansion for comments/suggestions is session/AI paths, not a CollabHandlers type.
```

---

## 14. `qf-export`

**Purpose:** Deterministic exporters.

**Dependencies:** `qf-core`, `qf-doc`, `qf-layout` (PDF), `qf-blocks`.

```rust
pub trait Exporter: Send + Sync {
    fn export(&self, state: &CanonicalState, req: &ExportRequest) -> Result<ExportOutput>;
}
```

---

## 15. `qf-ai`

**Purpose:** Suggestion-only AI integration.

**Dependencies:** `qf-core`, `qf-collab`, `qf-security`.

```rust
pub trait AiProvider: Send + Sync {
    fn propose(&self, req: AiRequest) -> Result<AiProposal>;
}

pub struct AiRequest {
    pub permission_token: ContentPermissionToken,
    pub selection_text: String,
    pub instruction: String,
}

pub struct AiService {
    pub fn request_suggestion(&self, req: AiRequest) -> Result<SuggestionId>;
    // creates Suggestion entity via command — does not apply content
}
```

**Invariant:** No silent apply; requires `ContentPermissionToken`.

---

## 16. `qf-citation`

**Purpose:** Source refs, citation formatting helpers.

**Dependencies:** `qf-core`, `qf-doc`.

```rust
pub struct CitationFormatter; // inline markers: (url; doi:…; isbn:…) — not full CSL
```

---

## 17. `qf-plugin`

**Purpose:** WASM host, manifests, capabilities, hooks.

**Dependencies:** `qf-core`, `qf-security`.

```rust
pub struct PluginHost { /* ... */ }
pub struct PluginManifest { /* ... */ }
impl PluginHost {
    pub fn load(&mut self, path: &Path) -> Result<PluginId>;
    pub fn unload(&mut self, id: &PluginId) -> Result<()>;
    pub fn dispatch_hook(&mut self, id: &PluginId, hook: Hook) -> Result<HookResult>;
}
```

---

## 18. `qf-security`

**Purpose:** Encryption, signing, ACL, audit, permission tokens.

**Dependencies:** `qf-core`.

```rust
pub struct Encryptor { /* ... */ }
pub struct AclEnforcer { /* ... */ }
pub struct AuditLog { /* ... */ }
pub struct ContentPermissionToken { /* capability + scope + expiry */ }
```

---

## 19. `qf-telemetry`

**Purpose:** Opt-in metrics; never document content.

**Dependencies:** `qf-core`.

```rust
pub struct Telemetry {
    pub enabled: bool, // default false
}
impl Telemetry {
    pub fn counter(&self, name: &str, value: u64);
    // rejects reserved names that imply content
}
```

---

## 20. `qf-theme`

**Purpose:** Theme tokens for UI/render.

**Dependencies:** `qf-core`.

```rust
pub struct Theme {
    pub colors: ColorPalette,
    pub typography: TypeScale,
}
```

---

## 21. `qf-app`

**Purpose:** Desktop application binary; wires subsystems; commit coordinator; windowing.

**Dependencies:** all feature crates as needed.

```rust
pub struct App {
    sessions: HashMap<DocumentId, DocumentSession>,
    bus: CommandBus,
    // ...
}

pub struct DocumentSession {
    pub kernel: Kernel,
    pub journal: Journal,
    pub storage: PackageStoreHandle,
    pub layout: LayoutEngine,
    pub render: RenderScheduler,
    pub undo: UndoService,
}
```

**Commit coordinator** lives here (or thin `qf-core` util used by app).

---

## 22. Ownership boundaries summary

| Resource | Owner | External access |
|----------|-------|-----------------|
| CanonicalState | Kernel | `&` only |
| Journal file | Journal | append/iter APIs |
| Package file | Storage | open/save APIs |
| GPU device | GpuContext | render only |
| Plugin memory | PluginHost | isolated |
| Search index | SearchEngine | rebuildable |

---

## 23. Error behavior summary

- Prefer typed errors with `thiserror`  
- No panics on user input  
- Invariant violations in kernel → `ApplyError`  
- Corruption → `OpenOutcome::Corrupt` not panic wipe  

---

*End of crate API outline.*
