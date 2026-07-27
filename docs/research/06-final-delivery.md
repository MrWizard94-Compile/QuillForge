# Stage 10: Final Audit & Delivery Package

## Re-verification commands

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- demo
```

**Expected:** all succeed; demo prints unknown-block preservation and reopen ok.

## Documentation currency

| Doc set | Status |
|---------|--------|
| `docs/01–25` product/architecture specs | Binding, prior |
| `docs/research/*` research program | Complete this delivery |
| `DECISIONS.md` | ADR-011, ADR-012 added |
| `MANIFEST.md` | Updated for research+M2 |
| `AGENTS.md` | Law |

## Release / rollback

- **Release unit:** git tag optional `research-m2-hardened`  
- **Rollback:** prior commit on `main`; packages remain forward-compatible within format 1.0.0  
- **No migrations required** for existing demo packages from Phase 1–2 if format unchanged  

## Monitoring / alerting

Offline-first app: no cloud monitors required for this slice.  
Operational signals: journal fsync errors surface as `CommandError::Journal`; corrupt open as `OpenOutcome::Corrupt`.

## Handoff materials

1. [00-index.md](00-index.md) — program map  
2. [01-ten-projects.md](01-ten-projects.md) — research  
3. [02-synthesis-design.md](02-synthesis-design.md) — design  
4. [03-foundational-delta.md](03-foundational-delta.md) — doc deltas  
5. [04-engineering-plan.md](04-engineering-plan.md) — milestones  
6. [05-audit-report.md](05-audit-report.md) — audit/harden  
7. This file — delivery checklist  

## Product delivery status

| Layer | Status |
|-------|--------|
| Research (10 projects) | **Delivered** |
| Design synthesis | **Delivered** |
| Foundational docs | **Delivered** (delta + existing) |
| Engineering plan | **Delivered** |
| Implementation M2 | **Delivered** |
| Audit + harden | **Delivered** |
| Full v1 DoD (`docs/24`) | **In progress** (M3–M10 remain) |

## Human next actions

1. Read `docs/research/00-index.md` then synthesis.  
2. Run re-verification commands.  
3. Approve milestone M3 (layout goldens) or M4 (input wiring).  
