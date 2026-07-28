# Product Surface — Word-class Document Editor (WPAI)

**Document ID:** QF-UX-001  
**Version:** 1.0.0  
**Status:** Binding product priority  
**Authority:** Director instruction + PRD  
**Brand:** WPAI — never Microsoft / Word trademarks in product chrome  

---

## 1. Product priority (non-negotiable)

Quillforge’s **primary product surface** MUST read to end users as:

> **An upgraded Microsoft Word–class desktop document editor** — familiar page-centric writing, formatting, review, and export — **without Microsoft branding or trademarks.**

Internal identity remains “local-first document OS.” **User-facing priority is the Word-class editor experience**, powered by Quillforge’s durable core.

| Layer | Message |
|-------|---------|
| **User-facing** | Familiar document editor: pages, print layout, formatting, review, export |
| **Differentiation** | Local-first `.qfdoc`, crash-proof journal, encryption, plugins, AI suggestions only, WPAI brand |
| **Forbidden** | Microsoft, Word, Office, OneDrive, ribbon copycats of trademarked names, Office blue skin as brand |

---

## 2. Familiar patterns to match (behavior, not branding)

Users should feel “I already know this” within seconds:

| Pattern | Quillforge name | Notes |
|---------|-----------------|--------|
| Page canvas / print layout | **Page view** | Default authoring view |
| Continuous draft view | **Draft view** | Optional |
| Top command strip | **Command bar** (not “Ribbon”) | Tabs: Home, Insert, Layout, Review, View, Export |
| File menu | **File** | New, Open, Save, Save As, Export, Properties |
| Status bar | **Status** | Page x of y, words, language, zoom, sync/offline |
| Track changes / comments | **Suggestions** + **Comments** | Map to existing collab model |
| Styles gallery | **Styles** | Paragraph/heading styles from theme |
| Export to PDF/DOCX | **Export** | Already foundation-complete |

---

## 3. Upgrade axes (why not “just Word”)

Quillforge must feel **better** for ambitious authors, not only familiar:

1. **Never lose work** — journal + LKG + process-kill proven  
2. **Own the file** — local-first `.qfdoc`, optional encryption  
3. **Honest AI** — suggestions only; WPAI thesis  
4. **Extensible blocks** — sandboxed plugins; unknown content survives  
5. **Deterministic publish** — same inputs → same PDF/DOCX/layout hashes  
6. **Offline collab path** — CRDT + TLS sync without cloud lock-in  

---

## 4. Visual brand (Word-class ≠ Office blue)

- Default chrome may be **light document-workspace** for familiarity (paper, gray rails).  
- **Accent and identity** remain WPAI forge (ember/coal) in logos, about, installers, splash.  
- Document page surface: **white / off-white paper**, black body text, clear margins — industry-standard reading comfort.  
- Do **not** clone Office Fluent blue as brand color.

---

## 5. Roadmap priority (sequencing)

After foundation, **productization order** is:

| Priority | Work | Exit |
|----------|------|------|
| P0 | Product surface law + chrome model (this doc + `ProductSurface`) | **Done** (model tested) |
| P1 | Native window shell: page canvas + command bar + status | **Done** (`quillforge edit`) |
| P2 | Print-layout fidelity (WYSIWYG vs layout tree) | **Done** — layout pages drive GUI + PDF `/Count` |
| P3 | Review UI (suggestions accept/reject) | Word-class review loop |
| P4 | DOCX round-trip import | Open Word files into `.qfdoc` |
| P5 | Polish: rulers, headers/footers, images | Parity set documented |

Kernel law is unchanged: all edits still Command → Journal → Kernel.

---

## 6. Naming rules

| Do | Don’t |
|----|--------|
| Quillforge, Page view, Command bar, Suggestions | Word, Microsoft, Office, OneDrive, SharePoint |
| “Document”, “Page layout”, “Export PDF” | “Word document” as product name |
| WPAI / Wizard Productions AI Studio | Office 365, Microsoft 365 |

External copy may say **“Word-class document editor”** once for category education; chrome and logos never use Microsoft marks.

---

## 7. Implementation hooks

| Artifact | Role |
|----------|------|
| `qf_app::product_surface` | Binding chrome model (tabs, views, status fields) |
| `Theme::document_workspace_light` | Paper-first chrome tokens |
| `Theme::wpai_forge_dark` | Brand / about / developer surfaces |
| Layout engine | Page geometry for print layout |
| Export DOCX/PDF | Interop with Word-class world |

---

*End of product surface law.*
