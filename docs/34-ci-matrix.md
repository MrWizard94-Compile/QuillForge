# Multi-OS CI Matrix

**Document ID:** QF-CI-001  
**Status:** Binding quality gate  
**MSRV:** rustc **1.85.0**  

---

## Jobs

| Job | Runner(s) | Steps |
|-----|-----------|--------|
| **lint** | `ubuntu-latest` | `cargo fmt --check`, clippy `-D warnings`, constitution `verify-pack.ps1` |
| **test** | `ubuntu-latest`, `windows-latest`, `macos-latest` | clippy, `cargo test --workspace`, `budget_*`, headless demos |
| **ci-ok** | `ubuntu-latest` | Aggregates lint + test (branch protection friendly) |

Workflow: [`.github/workflows/ci.yml`](../.github/workflows/ci.yml)

---

## Guarantees

- Same MSRV pin on every OS (`dtolnay/rust-toolchain@1.85.0`).
- Zero-warning law (`RUSTFLAGS=-D warnings`, clippy `-D warnings`) on all matrix legs.
- Performance budgets (`docs/26-performance-budgets.md`) on all matrix legs.
- Constitution pack integrity (GOV-INT-001) on Linux lint job.
- `fail-fast: false` so one OS failure still reports the others.

---

## Local parity

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace budget_ -- --nocapture
pwsh -File "AGENTS Constitution/tools/verify-pack.ps1"
cargo run -p qf-app -- search-demo
cargo run -p qf-app -- polish-demo
```

Or: `cargo xtask check` / `cargo xtask perf` when using xtask.

---

## Out of scope (next)

- Cross-compile targets (e.g. `aarch64-pc-windows-msvc` from Linux)  
- Browser WebDriver e2e  
- Nightly / MSRV-1 dual lane  
