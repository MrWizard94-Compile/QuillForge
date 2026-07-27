# Definition of Done — Quillforge v1

**Document ID:** QF-DOD-001  
**Version:** 1.0.0  
**Status:** Binding  

v1 is **done only when all items below are true**. Partial feature demos do not constitute v1.

---

## 1. Canonical core

| # | Criterion | Evidence |
|---|-----------|----------|
| D1 | Canonical write path exists: Command → Ops → Journal → Kernel | Architecture tests + code structure review |
| D2 | No subsystem mutates canonical state outside path | API surface review; no public interior mutators |
| D3 | Journal replay works and is deterministic | Replay suite green multi-OS |
| D4 | Operation-based undo/redo works | Undo round-trip tests |
| D5 | Dry-run before journal prevents poison | Fault test |

---

## 2. Durability & recovery

| # | Criterion | Evidence |
|---|-----------|----------|
| D6 | Every committed operation survives power loss | REC-01 |
| D7 | Every package write is atomic | REC-04 |
| D8 | Recovery from journal + checkpoint + metadata | Open path tests |
| D9 | Derived caches rebuilt after recovery | REC-08 |
| D10 | Corruption never silently overwrites LKG | REC-06, REC-07 |

---

## 3. Evolution & unknown content

| # | Criterion | Evidence |
|---|-----------|----------|
| D11 | Migrations exist and run safely | Migration fixtures |
| D12 | Unknown blocks survive round trips | U1 tests |
| D13 | Unknown survives plugin remove/reinstall | Plugin suite |
| D14 | Unknown survives migration | Migration+unknown fixtures |

---

## 4. Product features

| # | Criterion | Evidence |
|---|-----------|----------|
| D15 | Editing works offline (text, structure, marks, core blocks) | Integration + manual script |
| D16 | Layout works (deterministic pagination) | Layout goldens |
| D17 | Rendering works (GPU path, virtualization) | Render smoke + perf |
| D18 | Export works (MD, HTML, TXT, PDF, DOCX) | Export goldens |
| D19 | Search works (token + graph; semantic optional) | Search tests |
| D20 | Collaboration works (presence, comments, suggestions, shared edit) | Collab tests |
| D21 | Sync works (offline queue, eventual consistency) | Sync convergence |
| D22 | Plugins work (sandbox, custom block, hooks) | Plugin suite |
| D23 | AI suggestion flow works (permissioned, accept-only apply) | AI security tests |
| D24 | Security works (at-rest option, TLS sync, ACL, audit defaults) | Security suite |

---

## 5. Quality

| # | Criterion | Evidence |
|---|-----------|----------|
| D25 | All required automated tests pass on CI matrix | CI green |
| D26 | Performance budgets met | Bench gates |
| D27 | Risk register H-items mitigated or accepted with sign-off | Review notes |
| D28 | Docs match implementation for package format & APIs | Doc audit |

---

## 6. Explicit non-completion signals

v1 is **not** done if:

- Unknown content can be lost on save  
- Journal commits can disappear on power loss  
- AI can edit without accept  
- Plugins can touch raw FS  
- Layout goldens differ across CI OS with pinned fonts  
- Sync can write kernel without journal  

---

## 7. Sign-off

| Role | Signs |
|------|-------|
| Systems architect | D1–D5 |
| Storage/recovery lead | D6–D14 |
| Product eng lead | D15–D24 |
| QA lead | D25–D26 |
| Security lead | D23–D24, RK-07 |

---

*End of definition of done.*
