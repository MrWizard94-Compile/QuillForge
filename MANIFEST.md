# Delivery MANIFEST — Lossless inverse operations

**Title:** Preserve complete block state and secure suggestion markers during undo/replay
**Date:** 2026-07-28  
**Current law:** [WPAI Constitution](https://github.com/MrWizard94-Compile/Agents-Constitution), via [AGENTS.md](AGENTS.md) · `CONST-GATE-001` · `TEST-BEHAVIOR-001` · `SEC-INPUT-001`

**Historical scope:** This manifest records the July 28 inverse-operation delivery,
not current build/runtime acceptance. The current governance migration and its
verification limits are recorded in [governance-adoption.md](docs/governance-adoption.md).

## Changed files

1. `crates/qf-core/src/operation.rs` — document the replay-safe contract for `ApplySuggestionOps` and retain explicit `DeleteSuggestion` for reversible first upserts.
2. `crates/qf-kernel/src/lib.rs` — restore block metadata as a separate inverse operation; encode complete rich bodies; validate accepted, non-nested suggestion proposals; add behavior and negative tests.
3. `docs/03-kernel-journal-constitution.md` — synchronize canonical invariants and undo guarantees.
4. `docs/04-command-bus.md` — state the marker acceptance boundary.
5. `docs/22-testing.md` — require inverse fidelity coverage.
6. `CHANGELOG.md` — record the durable behavior change.
7. `crates/qf-storage/src/migration.rs` and `docs/07-migration.md` — remove dead same-version migration scaffolding and test the actual empty v1 path.
8. `.cargo/config.toml` and `docs/19-workspace-layout.md` — provide the documented xtask alias and list only supported automation commands.
9. `crates/qf-search/src/lib.rs`, `crates/qf-export/src/docx_import.rs`, `crates/qf-export/src/fonts.rs`, `crates/qf-sync/src/ws.rs`, and `crates/qf-gpu/src/lib.rs` — correct strict-Clippy findings without suppression.
10. Remaining Rust source changes are the canonical `cargo fmt --all` normalization required by the workspace format gate; they have no intended behavioral effect.

## Verify

```bash
cargo fmt --all -- --check
cargo test -p qf-kernel
cargo test -p qf-app --lib review::tests
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask check
```

## Self-audit

`CONST-GATE-001` is complete only after the commands above pass with zero warnings. The retained legacy plain-text block payloads and temporary `QFBB1` span encoding remain readable; new inverse payloads use complete `QFBB2` typed bodies.

**Result (2026-07-28):** all listed commands passed; `cargo xtask check` completed its format, strict Clippy, workspace build, workspace test, and performance-budget gates.
