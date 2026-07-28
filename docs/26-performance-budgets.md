# Performance Budgets (Foundation)

**Document ID:** QF-PERF-001  
**Status:** Binding for CI  
**Constitution:** `PERF-BUDGET-001`, `ENG-WARN-001`

These budgets gate **debug/CI** builds. They are intentionally generous so shared runners do not flake; they still catch algorithmic regressions (quadratic layout, unbounded journal I/O).

## Budgets

| ID | Operation | Limit (debug CI) | Evidence test |
|----|-----------|------------------|---------------|
| PB-LAYOUT-001 | Full layout of 200-block mixed doc | ≤ 5 s | `qf-layout` `budget_layout_mixed_200_under_limit` |
| PB-LAYOUT-002 | Incremental layout equals full hash (50 blocks × 20 edits) | ≤ 8 s | `qf-layout` `budget_incremental_reflow_under_limit` |
| PB-JOURNAL-001 | Append + fsync 500 journal records | ≤ 10 s | `qf-journal` `budget_append_500_under_limit` |
| PB-EXPORT-001 | Markdown + PDF + DOCX export of 100 paragraphs | ≤ 5 s | `qf-export` `budget_export_triple_under_limit` |
| PB-SYNC-001 | 50 TCP Hello/Push/Pull round-trips on loopback | ≤ 15 s | `qf-sync` `budget_tcp_50_roundtrips_under_limit` |
| PB-CRYPTO-001 | Argon2id encrypt/decrypt package (test KDF params m=8) | ≤ 3 s | `qf-security` `budget_encrypt_decrypt_under_limit` |

## Running

```bash
cargo test --workspace budget_
cargo xtask perf
```

CI runs the same filter after the full suite.

## Changing budgets

1. Measure on reference hardware (document mean/p95).  
2. Update this table and matching `assert!(elapsed < …)` constants.  
3. ADR note if the limit moves by more than 2×.

---

*End of performance budgets.*
