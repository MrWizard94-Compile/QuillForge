# WPAI Constitution adoption — QuillForge

## Source and recovery

Accepted GitHub [WPAI Constitution](https://github.com/MrWizard94-Compile/Agents-Constitution)
`main` is the single editable source. Project `AGENTS.md` retains QuillForge's
product priorities, Rust pins, and canonical mutation pipeline; it does not
copy quality/process law. Freeze one accepted revision per invocation and
load its skill and named law together. Evolve the canonical source through its
reviewed process, not through a project-local fork.

The CI audit is pinned to accepted version 6.16.3, commit
`89f8771df5a6fa8627cb9576afca1eefd0486d27`. This is a reproducibility pin for
the checker, not a permanent pin on future invocation law. Its checker SHA-256
is `8abf494f014cf6f2595e9b4b281a06d01a394ebebb8fa5f6ff2aefd5c95323ef`.
The wrapper verifies those exact bytes before executing them. It allows only
individually reviewed historical changelog references, bound to the complete
file digest and exact finding lines. Changed/missing evidence, new paths,
duplicate findings, or changed checker bytes fail closed; it does not filter
the scanner's file types or directory coverage.

The 69 retired tracked v5.0.1 files remain recoverable in QuillForge's published
base [2d3595595200f0b5b3dc93b190a8e1b368487c16](https://github.com/MrWizard94-Compile/QuillForge/tree/2d3595595200f0b5b3dc93b190a8e1b368487c16).
Their exact Git tree object is `3635f0fc87740e23b86dfbb485ad5855896efe9f`.
Removal from this branch does not rewrite history. Do not restore that tree
as live law; historical inspection/recovery is distinct from source authority.
The pre-pack backup and modified audit in the separate primary workstation
checkout are deliberately preserved, not claimed as cleaned by this branch.

## Scope

The migration changes the project loader, active documentation, CLI help, and
CI. The packaged CLI help receives a regression test for its single canonical
law route. The former local-pack integrity step is replaced by independent
canonical validation/tests and project adoption checks. `ci-ok` requires that
job as well as every existing product job; no build/test/security gate is
removed or weakened.

Published base CI run `30326133201` failed `cargo fmt` before migration. Its
Clippy/test steps were skipped. A fresh isolated Windows baseline passes
strict Clippy on Rust 1.85.0 and reproduces the format failure. The declared
formatter is applied separately, without copying the primary checkout's
unpublished commit or 44 unrelated modified files. Historical product-status
percentages are labeled as historical, not reaffirmed as current acceptance.

## Verification

With a separately verified checkout of the pinned canonical revision:

```powershell
python tools/check-constitution-adoption.py --source <canonical-root>/tools/check-adoption.py
python -m unittest discover -s tools -p 'test_*.py' -v
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo test --locked --workspace budget_ -- --nocapture
cargo run --locked -p qf-app -- help
```

The source audit covers tracked/unignored project files and the canonical
checker's bounded ignored agent-config locations. It does not prove every
PC copy is retired or establish native editor/runtime/release acceptance.
Keep the dirty primary checkout intact until its independent product changes
can be reconciled. Do not reset it to synchronize this migration.

## Local evidence — October 17, 2026

- Rust 1.85.0 full workspace tests passed after baseline formatting; zero
  failed/ignored tests or compiler warnings. The only later product-code edit
  is the CLI law string. Its new integration test first failed against the
  retired route, then passed against the fixed packaged executable.
- Final `cargo fmt --all -- --check` and strict workspace/all-target Clippy
  pass. All seven explicit performance-budget tests pass. Packaged `help`,
  `version`, `surface`, `search-demo`, `polish-demo`, and `review-demo` exit 0;
  generated demo outputs stay under the ignored task-owned `target` tree.
- All six fail-closed adoption-classification tests pass. The pinned canonical
  checker reports exactly two passive changelog references; their complete
  file digest and lines are reviewed, with no active route classified away.
- The 44 modified primary-checkout files were independently hashed before
  work and rechecked afterward: all remained byte-for-byte unchanged. The
  primary unpublished commit, untracked config, and pre-pack backup were not
  copied into this branch, staged, reset, or published.
- The 69 retired source files remain in the published base and exact tree
  identified above. Empty directory shells left locally contain zero files
  and are not part of the delivered Git tree. The separate primary checkout
  still needs a preservation-aware local reconciliation.

Section 0: the scoped adoption has complete source/dependency wiring, clean
owned diagnostics, behavior tests, synchronized docs, reviewed path/input and
publication boundaries, pinned stack, bounded audit work, and recoverable
history. No new invention claim or multi-agent implementation. Cross-platform
CI and merge are separate evidence gates; no native editor, full product,
full-PC cleanup, or new release acceptance is claimed by these local checks.
