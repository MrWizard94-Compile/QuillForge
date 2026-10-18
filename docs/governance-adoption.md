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
  failed/ignored tests or compiler warnings. The initial product-code edit
  was the CLI law string. Its new integration test first failed against the
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

## Cross-platform MSRV repair

The first adoption PR run `39077558620` passed canonical validation, the
adoption audit/tests, and formatting. Linux Clippy then exposed pre-existing
target-specific lock drift: `home` 0.5.12 requires Rust 1.88 and
`wayland-protocols` 0.32.13 requires 1.86, above the declared 1.85 baseline.
Windows-only verification had not exercised those Linux dependency paths.

Official release metadata, checked October 17, declares Rust 1.81 for
[`home` 0.5.11](https://crates.io/api/v1/crates/home/0.5.11) and Rust 1.71 for
[`wayland-protocols` 0.32.12](https://crates.io/api/v1/crates/wayland-protocols/0.32.12).
Precise Cargo updates select those compatible patch versions without changing
the Rust pin or disabling a target. Cargo also reconciled existing
Windows-sys/getrandom dependency selections within the locked cohort; no new
direct dependency was added. Windows checks are repeated for that changed
cohort, and Linux/macOS acceptance still depends on fresh CI. The primary
checkout and its own lockfile are unchanged.

After the lock repair, strict Windows all-target Clippy and the full locked
workspace suite pass again, with zero failed/ignored tests or diagnostics.
Linux's resolved 272-package graph declares no Rust requirement above 1.85;
that metadata check is not a substitute for compiling/testing it in CI.

CI also reported Node 20 deprecation in the old checkout action and an imminent
Ubuntu label migration. The workflow now pins official checkout v7.0.1 at
`3d3c42e5aac5ba805825da76410c181273ba90b1`, with credential persistence off,
and explicitly selects Ubuntu 24.04. The official
[checkout contract](https://github.com/actions/checkout) and
[Node 20 deprecation notice](https://github.blog/changelog/2025-09-19-deprecation-of-node-20-on-github-actions-runners/)
were verified October 17. No compatibility-bypass environment variable or
unsafe fork-checkout opt-in is used. Other platform labels are unchanged.

## Approved transport repair — October 18, 2026

Run `39077973186` passed Constitution, Ubuntu lint, and Windows tests; Linux
and macOS failed the existing unsolicited-notify test. OS read timeouts were
identified from English error messages, missing Unix `WouldBlock` wording.
Inspection also found lost frame progress when `read_exact` timed out after
consuming part of a header, mask, or payload. The director explicitly approved
a focused transport repair in this adoption PR, with all existing assertions
and three-OS gates retained.

Two new deterministic server regressions failed before repair: unrecognized
idle status produced a wire error, and a recognized mid-frame timeout led to
an unsupported opcode on the remaining bytes. The shared incremental WS/WSS
reader now retains exact offsets and classifies I/O kinds before converting
to the unchanged public error type. Server upgrade and live polling timeouts
are separated, and client notification waits restore the previous timeout.

Nine new tests cover both idle kinds, every byte split in a real request,
short/16-bit/64-bit lengths, masks, back-to-back frames, interrupted reads,
EOF/fatal errors, oversized/malformed frames, existing control handling,
partial client notifications, slow HTTP upgrade, and actual TLS-backed idle
and partial-frame delivery. The 39 sync tests pass locally, including every
original test, without retries added to existing assertions. Final local
formatting, strict all-target workspace Clippy, and all 206 workspace tests
pass, with zero failed/ignored tests or diagnostics. All seven explicit budget
tests and eight packaged commands (`help`, `version`, `surface`, `search-demo`,
`polish-demo`, `review-demo`, `ws-sync-demo`, `wss-sync-demo`) pass. Six
fail-closed governance tests and the pinned adoption audit pass; only the same
two historical changelog references remain classified non-active. The primary
44-file hash baseline and unpublished HEAD remain unchanged. Fresh
Linux/Windows/macOS CI, merge, and post-merge checks are still separate gates.
See the transport document for precise timeout and foundation-scope limits.
