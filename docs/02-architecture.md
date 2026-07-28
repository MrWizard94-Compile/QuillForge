# Quillforge — Production Architecture Specification

**Document ID:** QF-ARCH-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Architectural thesis

Quillforge is a **command-sourced, journaled, kernel-applied document OS**.

```
┌─────────────┐
│ User Action │  (input, UI, collab remote, plugin request, AI accept)
└──────┬──────┘
       ▼
┌─────────────┐
│   Command   │  (intent + targets + actor + capability context)
└──────┬──────┘
       ▼
┌─────────────┐
│ Command Bus │  (validate, authorize, resolve, emit Operations)
└──────┬──────┘
       ▼
┌─────────────┐
│ Operations  │  (pure, serializable, replayable mutations)
└──────┬──────┘
       ▼
┌─────────────┐
│   Journal   │  (append-only, durable, ordered, auditable)
└──────┬──────┘
       ▼
┌─────────────┐
│   Kernel    │  (apply ops → canonical state; enforce invariants)
└──────┬──────┘
       ▼
┌─────────────┐
│Derived State│  (layout, render, search, UI caches, sync views)
└─────────────┘
```

**Invariant A1:** No subsystem MAY mutate canonical document state outside this path.

**Invariant A2:** Derived systems MAY read canonical state and journal events; they MUST NOT become authoritative.

---

## 2. Layer model

| Layer | Crates (primary) | Mutates canonical? | Durability |
|-------|------------------|--------------------|------------|
| Input / UI | `qf-input`, `qf-app`, `qf-theme` | No (emits commands) | Ephemeral |
| Command bus | `qf-core` (+ handlers in domain crates) | No (emits ops) | Ephemeral until journal |
| Journal | `qf-journal` | Appends durable ops | Durable |
| Kernel | `qf-kernel`, `qf-doc`, `qf-blocks` | Yes (apply only) | Via journal |
| Storage | `qf-storage` | Package/checkpoint I/O | Durable |
| Layout | `qf-layout` | No | Cache |
| Render/GPU | `qf-render`, `qf-gpu` | No | Cache |
| Search | `qf-search` | No | Rebuildable index |
| Sync/Collab | `qf-sync`, `qf-collab` | No (emit commands) | Queue durable |
| Export | `qf-export` | No | Output files |
| Plugins | `qf-plugin` | No (request commands) | N/A |
| AI | `qf-ai` | No (suggestions only) | N/A |
| Security | `qf-security` | No (gates) | Keys/audit |
| Telemetry | `qf-telemetry` | No | Opt-in |

---

## 3. Process architecture (v1 desktop)

Single primary process (app) with optional sandboxed plugin workers:

```
┌──────────────────────────────────────────────────────┐
│ qf-app (main process)                                │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐  │
│  │ Input    │→ │ Command  │→ │ Journal + Kernel   │  │
│  │ + UI     │  │ Bus      │  │ + Storage          │  │
│  └──────────┘  └──────────┘  └────────────────────┘  │
│        ↑              ↑                │             │
│        │              │                ▼             │
│  ┌──────────┐  ┌──────────┐  ┌────────────────────┐  │
│  │ Render   │← │ Layout   │← │ Canonical snapshot │  │
│  │ + GPU    │  │          │  │ + event fanout     │  │
│  └──────────┘  └──────────┘  └────────────────────┘  │
└──────────────────────────────────────────────────────┘
          │ IPC (capability-restricted)
          ▼
┌─────────────────────┐
│ Plugin workers      │  (wasm or process isolation; no raw FS)
└─────────────────────┘
```

**Chosen isolation model:** WebAssembly component model for untrusted plugins (v1), with host-provided capability APIs only. Native dynamic libraries are **forbidden** for third-party plugins in v1 (security priority over extensibility convenience).

**Justification:** WASM provides memory isolation and deterministic host gating without raw FS or pointer smuggling. Native plugin DLLs violate the security priority ranking.

---

## 4. Data ownership boundaries

| Data | Owner crate | Writers | Readers |
|------|-------------|---------|---------|
| Operation log | `qf-journal` | journal only | kernel, recovery, audit, sync |
| Canonical Document | `qf-kernel` | kernel apply only | all read models |
| Package bytes | `qf-storage` | storage atomic writer | open/recovery |
| Layout tree | `qf-layout` | layout engine | render |
| GPU resources | `qf-gpu` | render/gpu | render |
| Search index | `qf-search` | index builders from events | query API |
| Sync queue | `qf-sync` | sync layer | sync layer |
| Presence | `qf-collab` | collab ephemeral | UI |
| Suggestions (AI/review) | kernel entities via ops | command path | UI |

---

## 5. Runtime data flow (edit keystroke)

1. `qf-input` produces `Command::InsertText { target, text, ... }`.
2. Command bus validates selection, authorizes actor, resolves block/span IDs.
3. Handler returns `Vec<Operation>` (e.g. `Op::InsertText`, mark preservation ops).
4. Journal appends ops with monotonic `OpSeq`, actor, timestamp, parent hash; **fsync**.
5. Kernel applies ops; updates `Document` AST; emits `KernelEvent`s.
6. Layout marks dirty ranges; reflows affected pages.
7. Render scheduler invalidates tiles; GPU composites viewport.
8. Search indexer consumes events asynchronously (never blocks commit).
9. Sync queue records ops for later push (if enabled).

**Failure policy:** If journal fsync fails, commit aborts; kernel is not advanced; UI reports error. If kernel apply fails after journal write, enter recovery/rebuild path (see recovery spec)—this is a **hard bug** and MUST crash-safe reconstruct from journal.

---

## 6. Read models

| Read model | Source | Freshness |
|------------|--------|-----------|
| Canonical snapshot | Kernel | Immediate after apply |
| Layout frames | Layout from snapshot | Sync or async with generation token |
| Search | Journal events | Eventually consistent (local) |
| Collab presence | Ephemeral stream | Best-effort |
| Export view | Snapshot + export config | On demand |

All read models carry a **`state_gen: u64`** matching last applied `OpSeq` (or a derived generation). Stale generations MUST NOT be written back to canonical storage.

---

## 7. Concurrency model

| Domain | Model |
|--------|-------|
| Command commit | Single-threaded commit pipeline per document (serialized) |
| Kernel apply | Document-local mutex/actor; no parallel apply of conflicting seq |
| Layout | Parallel page layout **allowed** if pure and deterministic |
| Render | Parallel tile rasterization allowed |
| Search index | Background thread/task |
| Sync network | Async I/O; merges produce commands, not kernel writes |
| Plugins | Host-mediated async; no shared mutable kernel |

**Invariant A3:** Document commit is strictly serial per `DocumentId`. Parallelism lives only in pure derived work.

---

## 8. Identity & types (cross-cutting)

Core IDs (defined in `qf-core`):

```rust
pub struct DocumentId(pub Uuid);
pub struct BlockId(pub Uuid);
pub struct SpanId(pub Uuid);
pub struct SectionId(pub Uuid);
pub struct OpId(pub Uuid);
pub struct OpSeq(pub u64); // monotonic per document journal
pub struct ActorId(pub Uuid);
pub struct SessionId(pub Uuid);
pub struct AssetId(pub Uuid);
pub struct CommentId(pub Uuid);
pub struct SuggestionId(pub Uuid);
pub struct PluginId(pub String); // reverse-DNS, e.g. "com.example.math"
pub struct Capability(pub String);
```

**ID generation:** UUIDv7 preferred for time-order; tests use deterministic UUID sources via `IdGen` trait.

---

## 9. Error taxonomy (architecture-level)

| Class | Behavior |
|-------|----------|
| `Validation` | Command rejected; no ops; no journal write |
| `Authorization` | Command rejected; audit event |
| `Conflict` | Command rejected or transformed per collab rules |
| `JournalIo` | Commit abort; user-visible; no kernel advance |
| `ApplyInvariant` | Treated as corruption risk; recovery path |
| `Migration` | Open blocked or migrate with user consent |
| `PluginFault` | Plugin killed; document unaffected |
| `Security` | Hard fail; no degrade-open that weakens crypto |

---

## 10. Cross-cutting policies

### 10.1 Determinism

- Journal replay MUST produce identical canonical state for identical journal bytes.
- Layout MUST be deterministic for identical snapshot + layout config + font set version.
- Export MUST be deterministic for identical snapshot + export config + font set.
- Wall-clock timestamps MAY be stored for audit but MUST NOT affect apply semantics (except as ordered metadata).

### 10.2 Time

- `CommittedAt` is wall clock for audit.
- Logical order is solely `OpSeq` (+ causal metadata for multi-replica).
- Clock skew MUST NOT reorder local commits.

### 10.3 Feature flags

Feature flags MAY gate UI and network, but MUST NOT fork canonical apply semantics mid-journal. Schema evolution uses migrations, not flags.

---

## 11. Component interaction map

```
qf-app
  ├─ qf-input ──────────────► commands
  ├─ qf-theme ──────────────► derived presentation
  ├─ qf-security ───────────► authorize + encrypt
  ├─ command bus
  │    ├─ handlers (editing, collab accept, plugin request)
  │    └─► qf-journal ─► qf-kernel ─► events
  │                          │
  │                          ├─► qf-layout ─► qf-render ─► qf-gpu
  │                          ├─► qf-search
  │                          ├─► qf-sync / qf-collab
  │                          └─► qf-export
  ├─ qf-storage ◄─────────── package I/O, checkpoints
  ├─ qf-plugin ───────────── capability host
  ├─ qf-ai ───────────────── suggestion proposals
  ├─ qf-citation
  └─ qf-telemetry (opt-in)
```

**Dependency direction rule:** Lower layers NEVER depend on `qf-app`. Derived crates NEVER depend on each other cyclically. See [19-workspace-layout.md](19-workspace-layout.md).

---

## 12. Failure domains

| Failure | Containment |
|---------|-------------|
| UI panic | Document session remains; recover UI from kernel snapshot |
| Plugin panic | Terminate plugin instance only |
| GPU device lost | Recreate device; rebuild render caches from layout |
| Disk full on journal | Commit fails; no silent truncation |
| Network partition | Local editing continues; sync queue grows |
| Corrupt package | Refuse clobber; offer LKG / recovery tools |

---

## 13. Architectural decisions (selected & justified)

| Decision | Choice | Why |
|----------|--------|-----|
| Mutation model | Command → Op → Journal → Kernel | Integrity + recovery + audit |
| Undo | Operation-based | Matches journal; avoids snapshot bloat |
| Sync | CRDT-friendly ops primary | Offline concurrent edit |
| Plugins | WASM sandbox | Security over convenience |
| Package | ZIP-like container `.qfdoc` with manifest | Atomic replace + inspectability |
| Checkpoints | Periodic snapshot + op seq | Fast open without full replay |
| GPU | Software framebuffer (`qf-gpu`); wgpu roadmap | Headless complete; native surface later |
| AI | Suggestion entities only | User ownership |

---

## 14. What is forbidden

- Direct `Document` mutation from UI or plugins  
- Search index writing into package as canonical  
- Layout/render writing into journal  
- Silent AI edits  
- “Best effort” dropping of unknown fields  
- In-place overwrite of package without staging  
- Mixing presence into durable content journal  

---

*End of architecture specification.*
