# Stage 6: Engineering Plan

**Horizon:** Research-informed build program  
**Current baseline:** Phase 1–2 complete (`DocumentSession`, `.qfdoc`, recovery basics)  
**Constitution:** AGENTS.md  

---

## Tech stack (confirmed)

| Layer | Choice | Rationale |
|-------|--------|-----------|
| Language | Rust 1.85 | Systems integrity, single language mandate |
| Workspace | Cargo virtual workspace | Modular crates |
| Package | ZIP + CBOR + journal frames | Inspectable + durable |
| Layout | Fixed-point pure engine | Typst-like determinism |
| GPU | wgpu via `qf-gpu` | Cross-platform |
| Collab | CRDT-primary (later) | AFFiNE/Yjs lesson |
| Plugins | WASM | Security |
| Automation | `cargo xtask` | Reproducible checks |

---

## Repository & branching

| Item | Policy |
|------|--------|
| Default branch | `main` |
| Commits | Atomic, green tree, imperative messages |
| Tags | `phase-N-…` when milestone DoD met |
| Secrets | Never in repo |

---

## Milestone plan

| Milestone | Goal | Exit criteria | Depends |
|-----------|------|---------------|---------|
| **M0** | Research program docs | `docs/research/*` complete | — |
| **M1** | Phase 1–2 session | Done (prior commit) | — |
| **M2** | Untrusted package hardening + layout hash | Tests + clippy green | M1 |
| **M3** | Layout block coverage + golden fixtures | Determinism tests | M2 |
| **M4** | Input↔session wiring + edit integration | End-to-end edit tests | M3 |
| **M5** | Search rebuild equivalence | Property test | M4 |
| **M6** | Citations validation + export cite markers | Unit/integration | M5 |
| **M7** | GPU render of layout frames | Smoke + no kernel write | M6 |
| **M8** | Collab CRDT text mode | Convergence tests | M7 |
| **M9** | Plugin WASM host | Capability tests | M8 |
| **M10** | AI suggestions + harden + ship checklist | DoD v1 | M9 |

**This delivery implements M0 + M2 fully, and advances M3 foundations.**

---

## Definition of done (per milestone)

- Builds with zero clippy warnings (`-D warnings`)  
- All tests pass  
- Docs/ADRs updated if behavior changes  
- MANIFEST entry for the delivery  
- No TODOs/stubs  

---

## Test strategy

| Type | Use |
|------|-----|
| Unit | Pure apply, invert, layout hash, validation |
| Integration | Session save/open, package rejection |
| Property | Replay equality, layout incremental==full |
| Security | ZIP traversal, size limits, capability deny |
| Perf | Budgets from `docs/22-testing.md` (later CI benches) |

---

## Dependency & release controls

- Pin via `Cargo.lock`  
- New deps require justification in DECISIONS or MANIFEST  
- Prefer std + small crates (zip, ciborium, sha2 already justified)  

---

## Risk register (research-updated)

| ID | Risk | Impact | Mitigation |
|----|------|--------|------------|
| R-01 | Layout nondeterminism | High | Fixed-point + hash tests |
| R-02 | Malicious ZIP | High | Entry/size/path limits |
| R-03 | Collab divergence | High | CRDT + op_id idempotency |
| R-04 | Plugin content loss | High | UnknownBlock gates |
| R-05 | Schema drift | High | Migrations + fixtures |
| R-06 | Scope creep (Notion parity) | Med | Document OS focus; P0/P1 freeze |

---

## Implementation order for M2 (this slice)

1. Package ZIP validation (path, count, size)  
2. Quarantine helper for corrupt packages  
3. Layout tree content hash + expanded block metrics  
4. Citation locator validation  
5. Search rebuild equivalence test  
6. Audit + harden + final docs  
