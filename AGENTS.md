# AGENTS — QuillForge project entry

**Status:** Level 4 project law and canonical-source pointer
**Canonical Constitution:** https://github.com/MrWizard94-Compile/Agents-Constitution
**Skill:** `wpai-constitution`

Accepted GitHub `main` is the sole editable source of WPAI quality/process law.
Local packs, generated mirrors, caches, and history are not independent law.
Freeze one accepted commit for each top-level invocation, load its `SKILL.md`,
`references/always-load.md`, and the named applicable law from that same commit,
and verify source identity/integrity. Do not silently substitute another source
when the canonical revision is unavailable. Each invocation owes one meaningful
Constitution improvement through the canonical review/evolution process, not a
local rule fork or cosmetic change. A continuation is not a new invocation.

## QuillForge project law

- Product requirements and research live in `docs/`; architectural decisions
  live in `DECISIONS.md`. Preserve this project-specific law during adoption.
- `docs/28-product-surface.md` sets the Word-class editor priority, with WPAI
  branding and no Microsoft marks in product chrome.
- Preserve `Command → Operations → Journal → Kernel → Derived`: only the
  command/journal/kernel path mutates canonical document state. Layout, render,
  search, sync views, and UI caches remain derived.
- Follow `rust-toolchain.toml` (Rust 1.85.0) and the committed Cargo lockfile.
  Build and test with locked dependencies; no warning suppression or weakened
  behavior assertions to get a green gate.
- Product verification: `cargo fmt --all -- --check`,
  `cargo clippy --locked --workspace --all-targets -- -D warnings`,
  `cargo test --locked --workspace`, and the documented budget/headless demos.
- Governance verification: see `docs/governance-adoption.md` for the accepted,
  pinned checker and exact-history classification. A static audit is not
  runtime/product acceptance and does not authorize publication or note edits.
- Do not bootstrap another editable governance pack. RepoForge's adopted
  canonical-source workflow replaces its former local-pack bootstrap route.
- Amend cross-project law only in the canonical repository; retain legitimate
  project constraints here without weakening that law or overriding safety.
