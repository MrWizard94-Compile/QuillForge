# Quillforge Engineering Package — Master Index

**Product:** Quillforge  
**Document set version:** 1.0.0  
**Status:** Binding engineering foundation  
**Audience:** Engineering leadership, principal/senior engineers, implementers  
**Language of implementation:** Rust (entire codebase)

---

## How to use this package

1. Read this index and the priority order.
2. Internalize [01-prd.md](01-prd.md) for product scope and non-negotiables.
3. Internalize [02-architecture.md](02-architecture.md) and [03-kernel-journal-constitution.md](03-kernel-journal-constitution.md) before any code.
4. Implement in the order defined in [21-roadmap.md](21-roadmap.md).
5. Gate every phase against [22-testing.md](22-testing.md) and [24-definition-of-done.md](24-definition-of-done.md).

Specs in this package are **normative**. Ambiguity is a bug in the spec; open a design change request rather than inventing local policy.

---

## Absolute priority order

When requirements conflict, resolve strictly in this order:

| Rank | Priority |
|------|----------|
| 1 | Data integrity |
| 2 | Crash recovery |
| 3 | Determinism |
| 4 | Unknown-content preservation |
| 5 | Migration safety |
| 6 | Canonical state correctness |
| 7 | Security |
| 8 | Performance |
| 9 | Extensibility |
| 10 | AI convenience |

---

## Sacred canonical state rules

1. **Canonical mutation path only:** `Command → Operation → Journal → Kernel`
2. **Derived systems never authoritative:** layout, render, search, sync views, telemetry, UI/render caches
3. **Undo/redo is operation-based**, not snapshot-based
4. **Every committed operation survives power loss**; every package write is atomic
5. **Migrations are first-class** from day one
6. **Unknown content survives** all round trips, plugin lifecycle, and migrations
7. **AI proposes only**; never silent mutation; never content without explicit permission
8. **Plugins are sandboxed and capability-based**; no raw FS; no kernel/journal bypass

---

## Document map

| # | File | Contents |
|---|------|----------|
| 01 | [prd.md](01-prd.md) | Formal PRD: personas, goals, constraints, v1 scope |
| 02 | [architecture.md](02-architecture.md) | System architecture, layers, data flow, invariants |
| 03 | [kernel-journal-constitution.md](03-kernel-journal-constitution.md) | Kernel & journal constitution |
| 04 | [command-bus.md](04-command-bus.md) | Command bus, handlers, validation, replay |
| 05 | [document-ast.md](05-document-ast.md) | AST, blocks, spans, marks |
| 06 | [recovery.md](06-recovery.md) | Crash recovery, atomic writes, corruption handling |
| 07 | [migration.md](07-migration.md) | Versioned package migrations |
| 08 | [unknown-content.md](08-unknown-content.md) | Unknown block/payload preservation |
| 09 | [plugin-sandbox.md](09-plugin-sandbox.md) | Plugins, capabilities, hooks |
| 10 | [security.md](10-security.md) | Encryption, signing, permissions, audit |
| 11 | [search.md](11-search.md) | Token, semantic, graph search |
| 12 | [layout.md](12-layout.md) | Deterministic layout & pagination |
| 13 | [render.md](13-render.md) | Render scheduler, dirty tracking, virtualization |
| 14 | [export.md](14-export.md) | Deterministic export pipelines |
| 15 | [gpu-toolkit.md](15-gpu-toolkit.md) | Native Rust GPU abstraction |
| 16 | [collaboration.md](16-collaboration.md) | Presence, comments, suggestions, shared editing |
| 17 | [sync.md](17-sync.md) | Offline-first sync, CRDT primary |
| 18 | [storage-qfdoc.md](18-storage-qfdoc.md) | `.qfdoc` package format |
| 19 | [workspace-layout.md](19-workspace-layout.md) | Workspace, folders, dependency direction |
| 20 | [crate-apis.md](20-crate-apis.md) | Per-crate APIs, traits, ownership |
| 21 | [roadmap.md](21-roadmap.md) | Phased roadmap & first implementation order |
| 22 | [testing.md](22-testing.md) | Test strategy, quality gates, budgets |
| 23 | [risk-register.md](23-risk-register.md) | Risks, mitigations, validation |
| 24 | [definition-of-done.md](24-definition-of-done.md) | v1 DoD checklist |
| 25 | [telemetry.md](25-telemetry.md) | Telemetry policy (default off) |

---

## Recovery guarantees (hard requirements)

- Every committed operation survives power loss.
- Every package write is atomic.
- Recovery reconstructs from **journal + last checkpoint + package metadata**.
- Derived caches are rebuilt after canonical recovery.
- Corruption must **never** silently overwrite last-known-good state.

---

## Suggested first implementation order

1. `qf-core` types, IDs, errors, versioning  
2. `qf-doc` AST + `qf-blocks` native blocks + `UnknownBlock`  
3. `qf-journal` append-only durable journal  
4. `qf-kernel` apply path + snapshots  
5. Command bus (`qf-core` / app wiring)  
6. `qf-storage` `.qfdoc` package + atomic writes  
7. Recovery + migration + unknown-content round-trip tests  
8. `qf-layout` → `qf-gpu` → `qf-render`  
9. Input / undo / editing  
10. Export / search  
11. Collab / sync  
12. Plugins → AI → hardening  

Full phase detail: [21-roadmap.md](21-roadmap.md).

---

## Normative language

| Term | Meaning |
|------|---------|
| **MUST** | Absolute requirement |
| **MUST NOT** | Absolute prohibition |
| **SHOULD** | Strong recommendation; deviation requires recorded design decision |
| **MAY** | Optional; implementer choice within stated bounds |
| **Canonical** | Authoritative document state after journal application |
| **Derived** | Recomputable; never source of truth |
| **Committed** | Journal-persisted and fsync-durable |

---

*End of index.*
