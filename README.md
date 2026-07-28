# Quillforge

**WPAI Software** · Wizard Productions AI Studio (**W.P.A.I**)

**Local-first document operating system** for writing, publishing, research, knowledge management, collaboration, and enterprise review workflows.

Quillforge is a durable, offline-first, canonical-document platform with optional synchronization and a capability-based plugin system. The entire codebase is Rust.

**Studio tagline:** Forging the future of creative media.  
**Thesis:** AI is in the name. The wizard is in the work.  
**Badge:** Human-directed · AI-assisted  

Brand kit (forge palette, voice, official marks): **[brand/](brand/)**.

## Hard architectural truth

```
User Action → Command → Operations → Journal → Kernel → Derived State
```

Only the command / journal / kernel path may mutate canonical document state. Layout, render, search, sync views, telemetry, and UI caches are **derived only**.

## Absolute priorities (conflict resolution order)

1. Data integrity  
2. Crash recovery  
3. Determinism  
4. Unknown-content preservation  
5. Migration safety  
6. Canonical state correctness  
7. Security  
8. Performance  
9. Extensibility  
10. AI convenience  

## Package format

Primary document package: **`.qfdoc`**

## Engineering package

This repository contains the production architecture foundation. Start here:

| Doc | Description |
|-----|-------------|
| [docs/00-index.md](docs/00-index.md) | Master index of all specifications |
| [docs/01-prd.md](docs/01-prd.md) | Formal Product Requirements Document |
| [docs/02-architecture.md](docs/02-architecture.md) | Production architecture specification |
| [docs/03-kernel-journal-constitution.md](docs/03-kernel-journal-constitution.md) | Kernel & journal constitution |
| [docs/04-command-bus.md](docs/04-command-bus.md) | Command bus specification |
| [docs/05-document-ast.md](docs/05-document-ast.md) | Document AST, blocks, spans |
| [docs/06-recovery.md](docs/06-recovery.md) | Crash recovery specification |
| [docs/07-migration.md](docs/07-migration.md) | Versioned migration specification |
| [docs/08-unknown-content.md](docs/08-unknown-content.md) | Unknown-content preservation |
| [docs/09-plugin-sandbox.md](docs/09-plugin-sandbox.md) | Plugin sandbox specification |
| [docs/10-security.md](docs/10-security.md) | Security specification |
| [docs/11-search.md](docs/11-search.md) | Search architecture |
| [docs/12-layout.md](docs/12-layout.md) | Deterministic layout engine |
| [docs/13-render.md](docs/13-render.md) | Render scheduler |
| [docs/14-export.md](docs/14-export.md) | Export specification |
| [docs/15-gpu-toolkit.md](docs/15-gpu-toolkit.md) | Native Rust GPU toolkit |
| [docs/16-collaboration.md](docs/16-collaboration.md) | Collaboration architecture |
| [docs/17-sync.md](docs/17-sync.md) | Sync architecture |
| [docs/18-storage-qfdoc.md](docs/18-storage-qfdoc.md) | `.qfdoc` package format |
| [docs/19-workspace-layout.md](docs/19-workspace-layout.md) | Rust workspace layout |
| [docs/20-crate-apis.md](docs/20-crate-apis.md) | Module-by-module API outline |
| [docs/21-roadmap.md](docs/21-roadmap.md) | Engineering roadmap |
| [docs/22-testing.md](docs/22-testing.md) | Testing & acceptance criteria |
| [docs/23-risk-register.md](docs/23-risk-register.md) | Risk register |
| [docs/24-definition-of-done.md](docs/24-definition-of-done.md) | Definition of done for v1 |
| [docs/25-telemetry.md](docs/25-telemetry.md) | Telemetry (default disabled) |
| [docs/26-performance-budgets.md](docs/26-performance-budgets.md) | CI performance budgets |
| [docs/ship/v1-checklist.md](docs/ship/v1-checklist.md) | Foundation ship checklist |
| [CHANGELOG.md](CHANGELOG.md) | Release history |

## Law

Binding AI/engineering law: [AGENTS.md](AGENTS.md) → [AGENTS Constitution](AGENTS%20Constitution/AGENTS.md) **v5.0.0** portable pack + [SOP.md](AGENTS%20Constitution/SOP.md). Verify: `pwsh -File "AGENTS Constitution/tools/verify-pack.ps1"`.

## Workspace

```
QuillForge/
├── Cargo.toml          # virtual workspace manifest
├── crates/             # focused internal crates (version 0.0.0)
├── xtask/              # cargo xtask automation
├── docs/               # engineering specifications
└── AGENTS Constitution/  # quality + process law
```

Build & verify:

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace budget_ -- --nocapture
cargo run -p qf-app -- demo
cargo run -p qf-app -- sync-demo
cargo xtask check
```

## Foundation release

Tag **`v0.1.0-foundation`** marks the local-first core as shippable for further product work. See [CHANGELOG.md](CHANGELOG.md). Package format version remains `1.0.0`.

## Studio

Quillforge is a **WPAI** (Wizard Productions AI Studio) Software product, directed by Rob Bulkley (Mrwizard94).

Made in open collaboration with AI tools; written into shape, directed, and finished by Rob Bulkley. AI is in the name — the wizard is in the work.

- https://wpaistudio.net · rob@wpaistudio.net

## Implementation status

| Phase | Goal | Status |
|-------|------|--------|
| 0 | Workspace + specs | **Done** |
| 1 | Command bus, journal, kernel, package | **Done** (`DocumentSession`) |
| 2 | Recovery, migration, unknown content | **Done** |
| Research program | 10-project study → design → plan | **Done** (`docs/research/`) |
| M2 harden | ZIP budgets, layout hash, citations, search | **Done** |
| M3 | Layout golden fixtures | **Done** |
| M4 | EditorController input↔session | **Done** |
| M5 | Search rebuild equivalence | **Done** |
| M6 | Citation export markers + bibliography | **Done** |
| M7 | GPU/headless render of layout frames | **Done** |
| M8 | Text CRDT + presence/content streams | **Done** |
| M9 | Plugin host (capabilities, proposals, KV) | **Done** |
| M10 | AI suggestions + ship checklist | **Done** |
| Ship | Foundation v1 checklist | [docs/ship/v1-checklist.md](docs/ship/v1-checklist.md) |

Binding law: **[AGENTS Constitution/AGENTS.md](AGENTS%20Constitution/AGENTS.md)** (entry: [AGENTS.md](AGENTS.md)).  
SOP (delivery process): **[AGENTS Constitution/SOP.md](AGENTS%20Constitution/SOP.md)**.  
Research program: **[docs/research/00-index.md](docs/research/00-index.md)**.  
Latest delivery: **[MANIFEST.md](MANIFEST.md)**.  
Decisions: **[DECISIONS.md](DECISIONS.md)**.

## License

Proprietary — all rights reserved unless otherwise stated.
