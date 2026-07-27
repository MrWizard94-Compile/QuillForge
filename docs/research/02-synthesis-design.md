# Stage 4: Synthesized System Design

**Product:** Quillforge  
**Based on:** Ten-project research (`01-ten-projects.md`) + existing constitution (`docs/02–03`)

---

## Product vision

Quillforge is a **local-first document operating system**: durable canonical documents for writing, research, knowledge, collaboration, and enterprise review—combining:

| Inspiration | Adopted property |
|-------------|------------------|
| Obsidian | User-owned data, offline, extensibility |
| Logseq | Addressable blocks, graph/query readiness |
| Notion | Review UX: comments, suggestions, roles |
| AFFiNE / Yjs | Offline concurrent edit via CRDT; presence split |
| ProseMirror | Transactional mutation discipline |
| Automerge | Append-only history semantics |
| Zotero | Citations & source integrity |
| Typst | Deterministic layout/export |
| Anytype | Encryption & privacy posture (without forcing object-OS UX) |

**Hard architectural center (unchanged):**

```text
User Action → Command → Operations → Journal → Kernel → Derived State
```

---

## Feature priorities (research-weighted)

### P0 — Already required / partially shipped
1. Canonical write path + journal durability  
2. Atomic package + recovery + LKG  
3. Unknown-content preservation  
4. Operation-based undo  
5. Deterministic layout foundations  
6. Export fidelity path  

### P1 — Research-driven next
1. **Layout engine completeness** (Typst lesson: publish quality)  
2. **Session validation & quarantine** (Obsidian/Anytype trust of local files)  
3. **Citation integrity helpers** (Zotero)  
4. **Search event-sourcing completeness** (Logseq query mental model)  
5. **Collab CRDT text positions** (Yjs/AFFiNE) — after P1 layout  

### P2
Plugins WASM host production binding; AI suggestion providers; GPU polish.

---

## User stories (v1 core)

| ID | Story | Acceptance |
|----|-------|------------|
| US-01 | As an author, I write offline and never lose committed text after power loss | Journal fsync + recovery tests |
| US-02 | As a researcher, my citations and sources survive save/load | SourceRef + export |
| US-03 | As a reviewer, I comment and suggest without force-editing | Suggestions via command path |
| US-04 | As an enterprise admin, packages can be encrypted and audited | Security spec + ACL |
| US-05 | As a plugin author, custom blocks survive uninstall | UnknownBlock |
| US-06 | As a publisher, export is deterministic for same inputs | Layout+export goldens |
| US-07 | As a collaborator, offline edits converge when online | CRDT merge + journal |

---

## Trust boundaries

```text
┌─────────────────────────────────────────────────────┐
│ TRUSTED COMPUTE (local process)                     │
│  kernel, journal, storage, layout, render, security │
├─────────────────────────────────────────────────────┤
│ SEMI-TRUSTED (capability-gated)                     │
│  WASM plugins, AI providers (after permission)      │
├─────────────────────────────────────────────────────┤
│ UNTRUSTED INPUT                                     │
│  opened packages, network sync messages, clipboard  │
├─────────────────────────────────────────────────────┤
│ EPHEMERAL NON-CANONICAL                             │
│  presence, UI selection, render caches, telemetry   │
└─────────────────────────────────────────────────────┘
```

| Boundary | Rule |
|----------|------|
| Package open | Integrity verify; fail closed; LKG fallback |
| Sync | Auth + ACL; merge → commands only |
| Plugin | No FS/journal; propose ops only |
| AI | ContentPermissionToken required |
| Telemetry | Default off; never document body |

---

## Data classification

| Class | Examples | Handling |
|-------|----------|----------|
| **Critical** | Journal ops, canonical AST, encryption keys | Integrity + optional at-rest encrypt |
| **Sensitive** | Document text, comments, suggestions | Local; sync only authorized |
| **Internal** | Layout trees, search indexes | Rebuildable; may encrypt when unlocked |
| **Public/Derived** | Export outputs (user-initiated) | User-controlled destination |
| **Telemetry** | Metrics enums only | Opt-in; no content |

---

## Threat model notes

| Threat | Mitigation |
|--------|------------|
| Disk theft | At-rest encryption (spec) |
| Corrupt / malicious package | Manifest hashes, fail closed, LKG, size limits |
| Journal poison | Dry-run before append |
| Plugin escape | WASM + capabilities |
| AI exfiltration | Explicit permission token |
| Sync impersonation | TLS + auth + ACL re-check |
| Silent history rewrite | Append-only journal, hash chain |

---

## Success criteria (program)

1. Research documented and linked to ADRs.  
2. Specs remain consistent with synthesis.  
3. Engineering plan milestones executable.  
4. Implementation slices pass tests + clippy `-D warnings`.  
5. Audit finds no sacred-path violations.  
6. Hardening closes high-severity issues.  
7. Delivery package (MANIFEST) complete.

---

## Competitive positioning statement

Quillforge is **not** “another Markdown vault” (Obsidian) or “another cloud wiki” (Notion). It is a **durable document kernel** with package integrity, enterprise review, and deterministic publishing—local-first first, collaborative second, AI last and never silent.
