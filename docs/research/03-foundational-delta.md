# Stage 5: Foundational Documentation Delta

Existing binding specs live under `docs/01-prd.md` … `docs/25-telemetry.md`.  
This document records **research-driven additions** that refine (not replace) those specs.

---

## Traceability to core docs

| Foundation need | Existing doc | Research delta |
|-----------------|--------------|----------------|
| PRD | `01-prd.md` | User stories US-01…07 in synthesis |
| Architecture | `02-architecture.md` | Trust boundaries formalized in synthesis |
| Kernel/Journal | `03-…` | Confirmed by ProseMirror/Automerge patterns |
| Command bus | `04-…` | Transaction analogy from ProseMirror |
| AST | `05-…` | Block atom from Logseq/Notion |
| Recovery | `06-…` | Package untrusted-input validation (Anytype/Obsidian) |
| Migration | `07-…` | Logseq DB pivot caution |
| Unknown content | `08-…` | Plugin ecosystem lesson (Obsidian) |
| Plugins | `09-…` | Capability > unrestricted plugins |
| Security | `10-…` | Encryption posture (Anytype) + enterprise ACL (Notion) |
| Search | `11-…` | Derived-only (Logseq queries as mental model) |
| Layout/Export | `12/14` | Typst determinism mandate reinforced |
| Collab/Sync | `16/17` | Yjs presence split + AFFiNE CRDT |
| Citations | `qf-citation` crate | Zotero workflow first-class |
| Testing/DoD | `22/24` | Research gates added below |

---

## Functional specification additions

### FS-R1 Package open validation budget
- Reject packages with uncompressed logical size > configurable max (default 512 MiB).  
- Reject ZIP entry count > 100_000.  
- Reject path traversal (`..`, absolute paths) in ZIP names.

### FS-R2 Layout determinism contract (Typst-aligned)
- Same `(CanonicalState, LayoutConfig, font set)` → identical `LayoutTree` hash.  
- Incremental reflow hash-equals full layout.

### FS-R3 Citation integrity
- `SourceRef.content` locator non-empty when kind is Url/Doi.  
- Detached assets fail closed as `missing`, not silent drop.

### FS-R4 Search rebuild equivalence
- Event-sourced index for document equals full rebuild at same `op_seq`.

---

## UX / workflow maps (condensed)

### Author offline
```
Open/Create .qfdoc → Edit (commands) → Auto journal → Explicit Save → Export
```

### Reviewer
```
Open shared doc → Comment/Suggest → Author Accept/Reject → Audit trail
```

### Researcher
```
Attach Source → Cite block/span → Search graph → Export bibliography
```

### Plugin developer
```
Manifest + WASM → Capabilities grant → Custom block → User removes plugin → Unknown preserved
```

---

## Data model notes (no schema break)

Research confirms current model. Add explicit **document mode** flag for future CRDT text:

```rust
// DocumentMeta.custom or dedicated field later:
// collab_mode: bool  // when true, text uses stable IDs (Phase 7)
```

v1 single-player remains offset text; share migrates (already in collab spec).

---

## API documentation delta

Primary session API (shipped):

```rust
DocumentSession::create / open_package / commit_command / commit_ops / save
```

Layout API target:

```rust
LayoutEngine::full_layout / reflow_dirty / layout_hash
```

---

## Security & access-control policy delta

1. **Opened packages are untrusted** until integrity verification succeeds.  
2. **Least privilege** for plugins (no ambient authority).  
3. **AI** never on critical path for mutation.  
4. **Audit** without bodies by default.

---

## Test & release plan delta

| Gate | Source |
|------|--------|
| Sacred path | existing |
| ZIP bomb / traversal | **new** (FS-R1) |
| Layout hash stable | **new** (FS-R2) |
| Unknown round-trip | existing |
| Citation validation | **new** light unit tests |

Release: tag only when clippy `-D warnings` + `cargo test --workspace` green.
