# Engineering Roadmap

**Document ID:** QF-RM-001  
**Version:** 1.0.0  
**Status:** Binding for sequencing  

---

## Phase overview

| Phase | Goal | Exit criteria (summary) |
|-------|------|-------------------------|
| 0 | Workspace bootstrap | Builds, CI skeleton, docs merged |
| 1 | Command bus, journal, kernel, package | Edit ops durable; open/save |
| 2 | Recovery, migration, unknown content | Power-loss + round-trip gates green |
| 3 | Layout engine | Deterministic golden layouts |
| 4 | GPU renderer | Viewport render + tiles |
| 5 | Editing/input/undo | Interactive editor MVP |
| 6 | Storage polish/export/search | Export + search usable |
| 7 | Collaboration/sync | Two-client convergence |
| 8 | Plugins | WASM custom block |
| 9 | AI | Suggestion flow with permissions |
| 10 | Hardening/polish | v1 DoD complete |

---

## Phase 0 — Bootstrap

**Goal:** Empty product skeleton that builds.

**Deliverables:**

- Virtual workspace, all crate stubs with real module skeletons matching APIs  
- `xtask check`  
- rust-toolchain, CI workflow  
- This docs package  

**Dependencies:** None  

**Risks:** Over-scaffolding without discipline — mitigate by implementing Phase 1 types fully first  

**Exit criteria:** `cargo build --workspace` succeeds; docs complete  

---

## Phase 1 — Command bus, journal, kernel, package format

**Goal:** Canonical write path exists end-to-end.

**Deliverables:**

- `qf-core` commands/ops  
- `qf-doc` / `qf-blocks` AST  
- `qf-journal` durable append  
- `qf-kernel` apply + invariants  
- `qf-storage` create/open/save `.qfdoc`  
- Commit coordinator with dry-run  
- CLI or headless harness for insert text / save / reopen  

**Dependencies:** Phase 0  

**Risks:** Journal durability bugs — mitigate with fault-injection tests early  

**Exit criteria:**

- Replay equality tests pass  
- Package round-trip preserves state hash  
- No path mutates kernel without journal  

---

## Phase 2 — Recovery, migration, unknown-content

**Goal:** Survive failure and evolution.

**Deliverables:**

- Atomic save + LKG  
- Recovery service + corruption quarantine  
- Migration registry + sample migration harness  
- `UnknownBlock` encode/decode identity  
- Power-loss simulation tests  

**Dependencies:** Phase 1  

**Risks:** Windows rename/fsync semantics — test on Windows CI  

**Exit criteria:** All recovery acceptance tests in QF-REC-001; unknown round-trips; migration fixtures  

---

## Phase 3 — Layout engine

**Goal:** Deterministic pagination.

**Deliverables:**

- Fixed-point layout  
- Paragraph/heading/list/table/image/unknown placeholders  
- Dirty reflow  
- Golden fixtures + pinned fonts  

**Dependencies:** Phase 1 (canonical state)  

**Risks:** Nondeterminism across OS — pinned fonts in repo  

**Exit criteria:** Full layout hash stable; incremental == full  

---

## Phase 4 — GPU renderer

**Goal:** On-screen document.

**Deliverables:**

- `qf-gpu` wgpu context  
- Atlas + text + rects + images  
- `qf-render` scheduler + virtualization  
- Device-loss recovery  

**Dependencies:** Phase 3  

**Risks:** GPU variance — software/headless test path  

**Exit criteria:** Scroll 60fps budget on reference hardware for 50-page doc; no kernel coupling  

---

## Phase 5 — Editing / input / undo

**Goal:** Real interactive writing.

**Deliverables:**

- `qf-input`  
- Selection, IME basics  
- Operation-based undo/redo  
- Marks, lists, headings UI bindings  
- Caret rendering  

**Dependencies:** Phases 1–4  

**Risks:** IME complexity — isolate platform backends  

**Exit criteria:** Author can write multi-page doc offline with undo; crash-safe  

---

## Phase 6 — Export & search

**Goal:** Output and findability.

**Deliverables:**

- Markdown/HTML/TXT/PDF/DOCX exporters  
- Token + graph search  
- Index rebuild from events  
- Deterministic export goldens  

**Dependencies:** Phases 1–3 (PDF needs layout)  

**Risks:** Export fidelity drift — golden CI  

**Exit criteria:** Export gates green; search correctness vs rebuild  

---

## Phase 7 — Collaboration & sync

**Goal:** Multi-user eventual consistency.

**Deliverables:**

- Presence stream  
- Comments + suggestions  
- CRDT text mode  
- Sync queue + merge engine  
- Two-client tests  

**Dependencies:** Phase 1–2, security basics  

**Risks:** Merge divergence — convergence property tests  

**Exit criteria:** Concurrent offline edits converge; presence not journaled  

---

## Phase 8 — Plugins

**Goal:** Sandboxed extensibility.

**Deliverables:**

- WASM host + manifest  
- Capability enforcement  
- Custom block register + unknown fallback  
- Export hook sample  

**Dependencies:** Phase 2 unknown content, security  

**Risks:** Hostcall escapes — capability tests + fuzz  

**Exit criteria:** Plugin removal preserves payload; denied caps fail  

---

## Phase 9 — AI

**Goal:** Suggestion-only assistant.

**Deliverables:**

- Permission token flow  
- Provider trait  
- Suggestions via collab entities  
- Accept → command path  

**Dependencies:** Phase 7 suggestions, security  

**Risks:** Accidental silent apply — type system + tests forbid apply API  

**Exit criteria:** No content sent without grant; accept path only mutates via journal  

---

## Phase 10 — Hardening & polish

**Goal:** v1 ship quality.

**Deliverables:**

- Performance budgets met  
- Security review  
- Encryption at rest UX  
- Audit logs  
- Telemetry opt-in  
- Full DoD checklist  

**Dependencies:** All prior  

**Exit criteria:** [24-definition-of-done.md](24-definition-of-done.md) complete  

---

## Suggested first implementation order (tactical)

1. `qf-core` IDs, Version, errors, OpSeq, Hash256, PreservedMap  
2. `Operation` + `Command` enums  
3. `qf-blocks` + `UnknownBlock`  
4. `qf-doc` Document AST  
5. `qf-kernel` apply InsertText/InsertBlock  
6. `qf-journal` append/iter  
7. Commit coordinator  
8. `qf-storage` package write/read  
9. Headless integration test: type → save → kill → open → equal  
10. Unknown block fixture tests  
11. Layout → GPU → input → …  

---

*End of roadmap.*
