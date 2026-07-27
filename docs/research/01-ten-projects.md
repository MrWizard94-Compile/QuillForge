# Stages 1–3: Research, Decomposition, Deep Study

**Scope:** Ten mature systems closest to Quillforge’s problem space: local-first documents, knowledge management, collaborative editing, research tooling, and deterministic publishing.  
**Method:** Public product materials, architecture docs, community reports, and established engineering literature. Citations mark web sources.

---

## Comparative component matrix

| Component | Obsidian | Logseq | Notion | AFFiNE | ProseMirror | Yjs | Automerge | Zotero | Typst | Anytype |
|-----------|----------|--------|--------|--------|-------------|-----|-----------|-------|-------|---------|
| **UI / UX** | Vault editor, graph, panes | Outliner UI | Block canvas, DB views | Docs + whiteboard + DB | Embeddable view | N/A (lib) | N/A (lib) | Library UI | Compiler/preview | Object graph UI |
| **Domain** | Notes, links, embeds | Blocks, pages, queries | Blocks, DBs, pages | Pages, edgeless, collections | Schema nodes/marks | Shared types | Automerge docs | Items, collections, cites | Markup → layout | Objects, relations |
| **API / Integrations** | Plugin API, sync | Plugins, sync options | REST/public API, embed | Cloud optional, OSS | Editor plugins | Providers | Network adapters | Connectors, CSL | Packages | Sync protocol |
| **Persistence** | Markdown vault files | MD/Org → SQLite DB pivot | Cloud proprietary | Local + CRDT (OctoBase) | In-memory; host stores | Updates / snapshots | Change history | Local SQLite + files | Source files | Local encrypted store |
| **Security / Auth** | Local; E2EE sync product | Local-first privacy | Account RBAC | Local-first + cloud ACL | Host concern | Host concern | Host concern | Local + sync account | Local | Local crypto keys |
| **Automation** | Plugins, Dataview | Queries, flashcards | Automations, API | Limited OSS automation | Transactions | N/A | N/A | Translators, feeds | Build pipeline | Relations automation |
| **Observability / Admin** | Minimal core | Minimal | Workspace admin | Workspace modes | Dev tools | Debug dumps | History inspect | Library stats | Compile errors | Space management |

---

## Project 1: Obsidian

### Profile
| Field | Detail |
|-------|--------|
| **Purpose** | Local-first knowledge base over Markdown vaults |
| **Users** | Researchers, writers, PKM practitioners, power users |
| **Core workflows** | Create/link notes, graph explore, search, plugin-extend, optional sync |
| **Differentiators** | User-owned plain files; plugin ecosystem; graph + backlinks |
| **Architecture** | Desktop app; vault = folder of Markdown; plugins as extensions |
| **Strengths** | Data ownership; longevity of plain text; community plugins; offline |
| **Weaknesses** | Rich layout/print weaker than word processors; collab secondary |
| **Gaps for Quillforge** | No first-class enterprise review/suggestion pipeline; not a full doc OS |

### Deep study
- **Journey:** Open vault → write Markdown → `[[wiki-links]]` → backlinks/graph → plugins.  
- **Entry points:** Vault open, file tree, graph, search, command palette.  
- **Patterns:** Local-first as product principle; separate data from app; extensibility over monolithic features.  
- **Scale clues:** Vaults with tens of thousands of notes common; performance depends on plugins.  
- **Security:** Local by default; official Sync uses E2EE.  
- **Pain:** Plugin quality variance; proprietary binary app (not fully OSS core).

**Takeaways for Quillforge:** Preserve user ownership (package format + export). Plugin capability model must not require cloud. Graph/search as **derived** indexes only.

---

## Project 2: Logseq

### Profile
| Field | Detail |
|-------|--------|
| **Purpose** | Privacy-first block outliner / PKM |
| **Users** | Outliner fans, researchers, daily notes practitioners |
| **Core workflows** | Daily journal, nested blocks, bi-directional links, queries, PDF annotate |
| **Differentiators** | Everything is a block; local MD/Org; Datalog queries |
| **Architecture** | Local files; major pivot to SQLite-backed DB version for scale |
| **Strengths** | Block atomicity; offline; open source ethos; query power |
| **Weaknesses** | Page vs block friction; performance on pure file graph; DB migration anxiety |
| **Gaps** | Not enterprise publishing; collab/review not core strength |

### Deep study
- **Journey:** Open graph → daily page → type bullets → link/tag → query.  
- **Patterns:** Block-as-first-class-object; hierarchy + free association tension.  
- **Scale:** File-based model hit limits → DB version.  
- **Pain:** Ontology overhead; restructuring cost; version transition risk.

**Takeaways:** Treat blocks as durable addressable units (Quillforge already does). Design **migrations** before format changes. Queries/search rebuild from journal events.

---

## Project 3: Notion

### Profile
| Field | Detail |
|-------|--------|
| **Purpose** | Cloud collaborative workspace (docs, DBs, wikis) |
| **Users** | Teams, PMs, startups, enterprise knowledge |
| **Core workflows** | Block edit, databases, permissions, comments, share |
| **Differentiators** | Unified blocks+databases UX; multiplayer; templates |
| **Architecture** | Cloud-primary; proprietary storage; real-time multiplayer |
| **Strengths** | Collaboration UX; structure+flexibility; enterprise admin |
| **Weaknesses** | Offline secondary; data lock-in; performance on huge workspaces |
| **Gaps** | Not local-first; limited deterministic export/print fidelity |

### Deep study
- **Journey:** Workspace → page → blocks/DB → share/comment.  
- **Patterns:** Block-type registry; permissions at page/workspace; presence.  
- **Security:** Account RBAC, SSO enterprise.  
- **Pain:** Offline gaps; export fidelity; vendor dependency.

**Takeaways:** Enterprise review (comments, suggestions, roles) is table stakes. Quillforge must match **permissioned collab** without abandoning local-first. Block type registry informs plugin custom blocks.

---

## Project 4: AFFiNE

### Profile
| Field | Detail |
|-------|--------|
| **Purpose** | Open-source local-first Notion/Miro alternative |
| **Users** | Devs, teams wanting local-first collab + canvas |
| **Core workflows** | Docs, edgeless whiteboard, databases, optional cloud |
| **Differentiators** | Local-first + CRDT collab; multi-modality |
| **Architecture** | Local store; CRDT (y-octo/OctoBase lineage); cloud optional |
| **Strengths** | Offline + sync; OSS; multi-surface workspace |
| **Weaknesses** | Product surface complexity; maturity vs Notion |
| **Gaps** | Enterprise review depth; publishing determinism |

### Deep study
- **Journey:** Local workspace → edit offline → sync/collab when online.  
- **Patterns:** CRDT for eventual consistency; workspace as container.  
- **Security:** Local ownership; cloud ACL when enabled.  
- **Pain:** Multi-modality can dilute focus.

**Takeaways:** Separate **content stream** (durable) from **presence** (ephemeral). CRDT preferred for offline concurrent edit—already in Quillforge sync spec.

---

## Project 5: ProseMirror

### Profile
| Field | Detail |
|-------|--------|
| **Purpose** | Toolkit for building rich-text editors |
| **Users** | Editor implementers (Atlassian, NYT, many products) |
| **Core workflows** | Schema → transactions → state → view; collab steps |
| **Differentiators** | Immutable doc model; schema; collab steps; modular |
| **Architecture** | Functional document tree; plugins; transform pipeline |
| **Strengths** | Proven collab model; schema control; modularity |
| **Weaknesses** | Steep learning curve; not a product UI |
| **Gaps** | No storage/recovery product; host must supply durability |

### Deep study
- **Journey:** Define schema → dispatch transactions → map positions.  
- **Patterns:** **Transaction** as atomic change unit; position maps; steps.  
- **Scale:** Production editors at large orgs.  
- **Pain:** Custom schema + collab binding complexity.

**Takeaways:** Quillforge **Command → Operation → Journal** is the systems analogue of ProseMirror transactions—keep it. Schema/versioned block registry remains essential. Position mapping needed for collab anchors.

---

## Project 6: Yjs

### Profile
| Field | Detail |
|-------|--------|
| **Purpose** | Production CRDT shared types for collab software |
| **Users** | Real-time collab app builders |
| **Core workflows** | Shared types → providers → awareness; snapshots |
| **Differentiators** | Network-agnostic; rich-text bindings; performance |
| **Architecture** | CRDT updates; optional GC; checkpoints |
| **Strengths** | Speed/memory vs Automerge for large text; ecosystem |
| **Weaknesses** | Full history not default (unlike Automerge); single-author risk historically |
| **Gaps** | Not a document OS; no package integrity model |

### Deep study
- **Patterns:** Separate awareness (presence) from document CRDT.  
- **Scale:** Large docs better than Automerge in community benchmarks.  
- **Pain:** GC vs history tradeoff; binding quality varies.

**Takeaways:** Keep **presence out of journal**. Prefer CRDT performance characteristics for text; store **op journal** as Quillforge audit truth even if CRDT is merge substrate.

---

## Project 7: Automerge

### Profile
| Field | Detail |
|-------|--------|
| **Purpose** | CRDT library with rich change history |
| **Users** | Local-first apps needing history |
| **Core workflows** | Concurrent changes → merge → inspect history |
| **Differentiators** | Git-like change history; research pedigree |
| **Architecture** | Full history retention; multi-language (Rust/JS) |
| **Strengths** | History semantics; offline merge correctness |
| **Weaknesses** | Heavier memory/CPU for large text vs Yjs (community reports) |
| **Gaps** | Not a UI product |

### Deep study
- **Patterns:** History as first-class; merge without central authority.  
- **Pain:** Cost of forever-history on huge docs.

**Takeaways:** Quillforge journal already is append-only history—closer to Automerge philosophy. Compaction/checkpoints must preserve recovery without unbounded growth forever in hot path.

---

## Project 8: Zotero

### Profile
| Field | Detail |
|-------|--------|
| **Purpose** | Research reference manager + citations |
| **Users** | Academics, students, researchers |
| **Core workflows** | Capture sources, organize collections, cite, bibliographies |
| **Differentiators** | Citation styles (CSL), connectors, library model |
| **Architecture** | Local SQLite + attachment files; optional sync |
| **Strengths** | Citation fidelity; research workflow depth; local store |
| **Weaknesses** | Not a full writing OS; doc editing external |
| **Gaps** | Layout/export of long-form docs secondary |

### Deep study
- **Journey:** Capture → tag/collect → cite in word processor → bibliography.  
- **Patterns:** Source as first-class entity; style engines; attachment integrity.  
- **Security:** Local library; account for sync.

**Takeaways:** `SourceRef` / citation blocks and deterministic bibliography export are strategic. Attachment hashing already matches Quillforge assets.

---

## Project 9: Typst

### Profile
| Field | Detail |
|-------|--------|
| **Purpose** | Modern typesetting system (LaTeX alternative) |
| **Users** | Academics, technical writers, publishers |
| **Core workflows** | Write markup → compile → PDF; packages |
| **Differentiators** | Fast compile; clean language; incremental compilation |
| **Architecture** | Compiler pipeline; layout engine; deterministic outputs |
| **Strengths** | Deterministic publish; performance; authoring UX vs LaTeX |
| **Weaknesses** | Not collaborative multiplayer core; not KM graph |
| **Gaps** | Not offline collab doc OS |

### Deep study
- **Patterns:** **Deterministic layout/export** as product promise; pure pipeline.  
- **Scale:** Incremental compilation for large docs.  
- **Pain:** Ecosystem younger than LaTeX.

**Takeaways:** Quillforge layout must be pure + fixed-point/deterministic (already specified). Export fidelity is a first-class quality gate, not a plugin afterthought.

---

## Project 10: Anytype

### Profile
| Field | Detail |
|-------|--------|
| **Purpose** | Local-first object-based knowledge OS |
| **Users** | Privacy-focused personal OS users |
| **Core workflows** | Create typed objects, relations, graphs, offline sync |
| **Differentiators** | Object model + local encryption; P2P-ish sync vision |
| **Architecture** | Local encrypted store; typed objects/relations |
| **Strengths** | Privacy; flexible object ontology; offline |
| **Weaknesses** | Learning curve; collaboration maturity |
| **Gaps** | Traditional long-form publishing weaker |

### Deep study
- **Patterns:** Typed objects + relations (graph KM); local crypto keys.  
- **Security:** Encryption-centric local-first.  
- **Pain:** Abstraction overhead for simple writing.

**Takeaways:** Quillforge should keep **document-centric** UX (not force object ontology) but support graph search via citations/links. Encryption-at-rest is enterprise-critical.

---

## Cross-project pattern synthesis (input to Stage 4)

| Pattern | Seen in | Quillforge mapping |
|---------|---------|-------------------|
| Local-first ownership | Obsidian, Logseq, AFFiNE, Anytype | `.qfdoc` + offline kernel |
| Block as atom | Logseq, Notion, ProseMirror | `Block` + ops |
| Transactional edits | ProseMirror | Command/Op/Journal |
| CRDT offline collab | Yjs, Automerge, AFFiNE | `qf-sync` / `qf-collab` |
| Presence ≠ content | Yjs awareness | Separate streams |
| Plugin extensibility | Obsidian, Logseq | WASM capability sandbox |
| Citations/research | Zotero | `qf-citation` |
| Deterministic publish | Typst | `qf-layout` + `qf-export` |
| Schema evolution pain | Logseq DB pivot | Migrations first-class |
| Enterprise roles | Notion | ACL + audit |

---

## Explicit non-goals drawn from competitors

- Cloud-only storage (Notion trap).  
- Unrestricted native plugins (security).  
- Forever-growing unreclaimable history without compaction plan.  
- Silent AI mutation of user content.  
- Dropping unknown plugin content on load.
