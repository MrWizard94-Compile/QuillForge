# Delivery MANIFEST — Status + process-kill harness

**Title:** Project status report; OS process-abort durability harness  
**Date:** 2026-07-28  
**Law:** AGENTS Constitution v5.0.0

## Summary

1. `docs/27-project-status.md` — layered completion percentages.  
2. `qf-kill-harness` binary + `process_kill` integration tests (ADR-025).  
3. Ship checklist D6 residual fully closed.

## Completion (from status doc)

| Layer | % |
|-------|---|
| Foundation DoD D1–D28 | ~98% |
| Local-first core | ~92% |
| Marketing v1 polish | ~75% |
| Long-term vision | ~55% |

## Verify

```bash
cargo test -p qf-storage --test process_kill
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Self-audit

CONST-GATE-001 pass.
