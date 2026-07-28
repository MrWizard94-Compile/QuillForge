# Delivery MANIFEST — CI performance budgets (D26)

**Title:** Performance budget tests + CI gate + xtask perf  
**Date:** 2026-07-28  
**Law:** AGENTS Constitution v4.1 (`PERF-BUDGET-001`, `CONST-GATE-001`, `ENG-WARN-001`)

## Summary

1. **`docs/26-performance-budgets.md`** — binding budget table (PB-LAYOUT, JOURNAL, EXPORT, SYNC, CRYPTO).
2. **`budget_*` tests** — layout, journal, export, TCP sync, crypto.
3. **CI** — `.github/workflows/ci.yml` runs `cargo test --workspace budget_`.
4. **xtask** — `cargo xtask perf` and `check` includes budgets.
5. **ADR-020** — CI-gated budgets decision.
6. Ship checklist **D26 → Met**.

## Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace budget_ -- --nocapture
cargo xtask perf
```

## Self-audit

| CONST-GATE | Result |
|------------|--------|
| Completeness | Pass |
| Zero warnings | Pass |
| Tests | Pass (all budget_* + workspace) |
| Docs sync | Pass |
| Review packaging | Pass |

## Deferred (marketing / polish)

| Item | Notes |
|------|--------|
| TLS/WebSocket WAN | Protocol + TCP complete |
| Rich PDF/DOCX styles | Minimal writers ship |
| OS process-spawn kill | Synthetic matrix binding |
