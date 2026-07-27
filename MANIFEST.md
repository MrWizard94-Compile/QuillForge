# Delivery MANIFEST — Research Program + M2 Harden

**Title:** Competitive research (10 projects) + package/layout/citation/search hardening  
**Date:** 2026-07-27  
**Constitution:** AGENTS.md v2.0.0  

## One-sentence summary

Completed the full research→design→plan workflow against ten mature systems, then implemented and hardened M2: untrusted `.qfdoc` ZIP budgets, quarantine, deterministic layout hashing, citation validation, and search rebuild equivalence—all zero-warning and fully tested.

## Suggested commit message

```
Complete research program and harden package/layout/citation/search.

Document ten comparable systems, synthesize Quillforge design, ship
engineering plan, implement untrusted ZIP limits, layout content hashes,
citation validation, and search rebuild gates. Clippy -D warnings clean.
```

## How to verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- demo
```

## Key paths

| Path | Purpose |
|------|---------|
| `docs/research/00-index.md` | Program index |
| `docs/research/01-ten-projects.md` | Research + decompose + deep study |
| `docs/research/02-synthesis-design.md` | Vision, trust, threats |
| `docs/research/03-foundational-delta.md` | Spec deltas |
| `docs/research/04-engineering-plan.md` | Milestones M0–M10 |
| `docs/research/05-audit-report.md` | Audit + harden |
| `docs/research/06-final-delivery.md` | Delivery checklist |
| `crates/qf-storage/src/zip_io.rs` | ZIP validation + quarantine |
| `crates/qf-layout/src/lib.rs` | Deterministic layout + hash |
| `crates/qf-citation/src/lib.rs` | Source validation |
| `crates/qf-search/tests/rebuild_equivalence.rs` | Search gate |
| `DECISIONS.md` | ADR-011, ADR-012 |

## Self-audit (Section 0)

All 15 points satisfied for this milestone scope. Full v1 product DoD remains multi-milestone (see engineering plan).

## Next for human

1. Review research synthesis  
2. Run verify commands  
3. Green-light M3 or M4  
