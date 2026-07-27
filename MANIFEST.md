# Delivery MANIFEST — Constitution review + M3/M4

**Title:** AGENTS Constitution adoption; layout goldens; editor primary workflow  
**Date:** 2026-07-27  
**Law:** `AGENTS Constitution/AGENTS.md` + `WORKFLOW.md`

## One-sentence summary

Reviewed and wired the AGENTS Constitution folder as law, cleaned WORKFLOW.md, then delivered complete M3 layout golden fixtures and M4 `EditorController` (input → session → derived layout) with zero warnings and full tests.

## Constitution review notes

| Item | Finding |
|------|---------|
| Location | `AGENTS Constitution/AGENTS.md`, `WORKFLOW.md` |
| Root pointer | `AGENTS.md` → constitution folder (tool discoverability) |
| Tweaks observed | Rule of Three added; WORKFLOW process stages |
| WORKFLOW cleanup | Removed broken citation junk; made stages binding and Quillforge-aware |
| Priority | Constitution > product roadmap when process conflicts |

## Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- demo
```

## Suggested commit

```
Adopt AGENTS Constitution and deliver M3/M4 editor workflow.

Point root AGENTS.md at Constitution folder, clean WORKFLOW.md, add
layout golden fixtures, and ship EditorController wiring input through
DocumentSession with E2E tests. Clippy -D warnings clean.
```

## Key files

| Path | Purpose |
|------|---------|
| `AGENTS.md` | Root pointer |
| `AGENTS Constitution/*` | Binding law + workflow |
| `fixtures/layout/*` | Golden layout hashes |
| `crates/qf-layout/tests/golden.rs` | M3 golden tests |
| `crates/qf-app/src/editor.rs` | M4 editor controller |
| `crates/qf-app/tests/edit_e2e.rs` | Primary workflow E2E |
| `crates/qf-input/src/lib.rs` | Input events + selection |

## Self-audit

Section 0: complete M3/M4 scope, zero warnings, tests pass, docs updated, no TODOs.

## Next

M6 citation export markers / M7 GPU render of layout frames (M5 already green).
