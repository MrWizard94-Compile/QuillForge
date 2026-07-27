# Quillforge — Formal Product Requirements Document (PRD)

**Document ID:** QF-PRD-001  
**Version:** 1.0.0  
**Status:** Binding  
**Product:** Quillforge  

---

## 1. Product definition

Quillforge is a **local-first document operating system** for:

- Writing  
- Publishing  
- Research  
- Knowledge management  
- Collaboration  
- Enterprise review workflows  

It MUST function as a durable, offline-first, **canonical-document platform** with optional synchronization and extensibility.

Quillforge is **not** a loose collection of UI components. It is a systems product centered on a durable mutation pipeline and package integrity.

---

## 2. Vision

A single local package (`.qfdoc`) is the user's durable unit of truth. The application can fully author, edit, review, export, and search without network access. When connectivity exists, changes synchronize under explicit security and permission models. Custom and future content is never silently discarded.

---

## 3. Target users & personas

| Persona | Needs | Constraints |
|---------|-------|-------------|
| **Author** | Fast offline writing, undo, layout fidelity | No data loss on crash |
| **Researcher** | Citations, sources, search, knowledge graph | Asset + reference integrity |
| **Editor / Reviewer** | Comments, suggestions, audit trail | Permission-scoped edits |
| **Enterprise admin** | Encryption, signing, audit, SSO-ready identity hooks | Compliance, offline laptop policy |
| **Plugin developer** | Custom blocks, export hooks, search providers | Sandbox, capability grants only |
| **Publisher** | Deterministic export (PDF, HTML, Markdown, DOCX) | Bit-stable exports for same inputs |

---

## 4. Product goals

### 4.1 Primary goals (v1)

| ID | Goal | Success metric |
|----|------|----------------|
| G1 | Offline-complete core editing | Full edit/save/open cycle with zero network |
| G2 | Crash-safe durability | Power-loss test: zero committed op loss |
| G3 | Canonical mutation integrity | 100% mutations via command→journal→kernel |
| G4 | Unknown-content preservation | Round-trip tests pass for unknown blocks |
| G5 | Migration path | vN→vN+1 migrations with fixtures |
| G6 | Deterministic layout & export | Same inputs → same layout/export bytes |
| G7 | Operation-based undo/redo | Undo stack is ops, not full snapshots |
| G8 | Sandboxed plugins | Capability denial tests pass |
| G9 | AI as suggestion-only | No silent content mutation path exists |
| G10 | Collaboration + sync | Convergence under offline concurrent edit |

### 4.2 Non-goals (v1)

- Multi-tenant cloud-hosted document service as primary store (local-first is primary)
- Browser-only WASM product as sole runtime
- Real-time multiplayer gaming-style latency guarantees
- Automatic AI rewriting of user content
- Unrestricted native plugin process with filesystem access
- Supporting non-Rust application cores

---

## 5. Functional requirements

### 5.1 Document lifecycle

| ID | Requirement |
|----|-------------|
| FR-DL-01 | Create, open, save, close `.qfdoc` packages |
| FR-DL-02 | Atomic save; no half-written package as active state |
| FR-DL-03 | Crash recovery from journal + checkpoint + metadata |
| FR-DL-04 | Package version migration on open when safe |
| FR-DL-05 | Integrity verification via checksums on open |
| FR-DL-06 | Last-known-good retention on corruption |

### 5.2 Editing

| ID | Requirement |
|----|-------------|
| FR-ED-01 | Insert/delete/replace text within spans |
| FR-ED-02 | Insert/move/delete blocks |
| FR-ED-03 | Apply/remove marks (bold, italic, link, etc.) |
| FR-ED-04 | Native block types listed in AST spec |
| FR-ED-05 | Operation-based undo/redo |
| FR-ED-06 | Selection, caret, multi-cursor local support |
| FR-ED-07 | Clipboard paste produces commands, not raw kernel writes |

### 5.3 Structure & content model

| ID | Requirement |
|----|-------------|
| FR-CM-01 | Document → Section → Block → Span hierarchy |
| FR-CM-02 | Comments, suggestions, revisions as first-class entities |
| FR-CM-03 | Asset references and source references |
| FR-CM-04 | UnknownBlock preserves original type + payload |

### 5.4 Layout & render

| ID | Requirement |
|----|-------------|
| FR-LR-01 | Deterministic pagination and reflow |
| FR-LR-02 | Dirty-region render scheduling |
| FR-LR-03 | Virtualized viewport rendering |
| FR-LR-04 | Custom native Rust GPU toolkit |

### 5.5 Search

| ID | Requirement |
|----|-------------|
| FR-SE-01 | Token search over document text |
| FR-SE-02 | Semantic search (optional provider; offline embedding path if enabled) |
| FR-SE-03 | Graph search over citations/links/references |
| FR-SE-04 | Indexes rebuilt only from journal/canonical events |

### 5.6 Export

| ID | Requirement |
|----|-------------|
| FR-EX-01 | Export to PDF, HTML, Markdown, plain text, DOCX |
| FR-EX-02 | Deterministic export for fixed config + content |
| FR-EX-03 | Unknown blocks export as preserved opaque or annotated fallback |
| FR-EX-04 | Plugin export hooks under capability grants |

### 5.7 Collaboration

| ID | Requirement |
|----|-------------|
| FR-CO-01 | Presence and cursor tracking (ephemeral stream) |
| FR-CO-02 | Comments and threaded discussion |
| FR-CO-03 | Suggestions (proposed ops) requiring accept/reject |
| FR-CO-04 | Shared editing with conflict resolution |
| FR-CO-05 | Audit-friendly attribution on committed ops |

### 5.8 Sync

| ID | Requirement |
|----|-------------|
| FR-SY-01 | Offline-first queue of pending ops |
| FR-SY-02 | Eventual consistency; CRDT preferred, OT fallback allowed |
| FR-SY-03 | Separate content and presence streams |
| FR-SY-04 | Sync never bypasses journal/kernel |

### 5.9 Plugins

| ID | Requirement |
|----|-------------|
| FR-PL-01 | Capability-based sandbox |
| FR-PL-02 | Manifest-declared capabilities only |
| FR-PL-03 | Custom blocks, export hooks, search/AI/theme providers |
| FR-PL-04 | No raw filesystem access |
| FR-PL-05 | No direct journal or kernel mutation |

### 5.10 AI

| ID | Requirement |
|----|-------------|
| FR-AI-01 | AI may propose suggestions only |
| FR-AI-02 | AI MUST NOT silently modify user content |
| FR-AI-03 | AI MUST NOT receive document content without explicit permission |
| FR-AI-04 | Suggestions apply only via user accept → command path |

### 5.11 Security

| ID | Requirement |
|----|-------------|
| FR-SC-01 | Encryption at rest for package contents when enabled |
| FR-SC-02 | Encryption in transit for sync |
| FR-SC-03 | Package/signing support for integrity |
| FR-SC-04 | Permission model for collab roles |
| FR-SC-05 | Audit logs without document content by default |
| FR-SC-06 | Telemetry default disabled; never collect document content |

---

## 6. Non-functional requirements

| ID | Category | Requirement |
|----|----------|-------------|
| NFR-01 | Durability | Committed ops survive power loss (fsync barriers) |
| NFR-02 | Atomicity | Package write uses temp + fsync + atomic rename |
| NFR-03 | Determinism | Replay of same journal yields identical canonical state |
| NFR-04 | Performance | Interactive edit p95 < 16ms for single-op apply (typical laptop) |
| NFR-05 | Performance | Open 100MB package (cold) < 3s to editable |
| NFR-06 | Performance | Layout dirty reflow of 50 pages p95 < 50ms |
| NFR-07 | Reliability | Corruption never silently clobbers LKG |
| NFR-08 | Portability | Windows, macOS, Linux first-class |
| NFR-09 | Language | 100% Rust application core and GPU toolkit |
| NFR-10 | Maintainability | Modular workspace; acyclic crate graph |

---

## 7. Constraints

1. **Rust only** for the product codebase (no C++/C# app core).  
2. **Local-first:** full function offline.  
3. **User owns data:** packages readable via documented format; no forced cloud lock-in.  
4. **Canonical pipeline is inviolable.**  
5. **Unknown content preservation is non-negotiable.**  
6. **Migrations designed before v1 ships.**  

---

## 8. Success criteria for v1 product

v1 ships when all items in [24-definition-of-done.md](24-definition-of-done.md) are true, including:

- Canonical write path  
- Journal replay  
- Recovery + atomic writes  
- Migrations  
- Unknown-block round trips  
- Editing + undo/redo  
- Layout + GPU render  
- Export + search  
- Collaboration + sync  
- Plugins + AI suggestion flow  
- Security baselines  
- Tests and performance budgets met  

---

## 9. Out-of-scope integrations (v1)

| Item | Status |
|------|--------|
| Third-party SaaS AI without user consent UI | Forbidden |
| Direct Google Docs protocol compatibility | Out of scope |
| Mobile native apps | Post-v1 |
| Browser extension as primary editor | Post-v1 |

---

## 10. Acceptance of this PRD

Engineering work that violates absolute priorities or sacred canonical rules is rejected regardless of feature completeness.

---

*End of PRD.*
