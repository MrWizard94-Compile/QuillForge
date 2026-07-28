# Quillforge Project Status

**Document ID:** QF-STATUS-001  
**Date:** 2026-07-28  
**Studio:** WPAI Software · Wizard Productions AI Studio  
**Law:** AGENTS Constitution pack v5.0.1  
**Git tag:** `v0.1.0-foundation`  
**Package format:** `1.0.0`

---

## Completion percentage (honest layers)

| Layer | Scope | Completion | Notes |
|-------|--------|------------|--------|
| **Foundation DoD** | Ship checklist D1–D28 (`docs/ship/v1-checklist.md`) | **~98%** | All criteria **Met**; residual polish items are optional/depth |
| **Local-first core product** | Offline edit, package, recovery, export, plugins, AI path, sync+TLS | **~94%** | Foundation + Word-class P1 native shell ships |
| **Marketing / “v1 product”** | Polish exports, multi-OS kill CI, browser sync, UX | **~80%** | GUI shell open; WYSIWYG/print fidelity still P2+ |
| **Long-term vision** | Full document OS, multiplayer product UX, semantic search, rich GPU | **~62%** | Word-class shell + durable core; rich media / DOCX import remain |

**Headline for directors:**  
**Foundation is shippable (~98% of binding DoD). Word-class P1 native shell is live (`quillforge edit`). Overall vision ~60–65%.**

### Product surface priority (director)

| Priority | Target | Status |
|----------|--------|--------|
| **User appearance** | Upgraded Word-class document editor | P0 model + P1 GUI shell |
| **Brand** | WPAI only — never Microsoft/Word/Office marks in chrome | Enforced in tests |
| **Engine** | Unchanged: Command → Journal → Kernel, `.qfdoc` | Intact |
| **Law** | `docs/28-product-surface.md` · ADR-026 | Binding |

---

## DoD scoreboard (D1–D28)

| Band | IDs | Status |
|------|-----|--------|
| Canonical core | D1–D5 | **Met** |
| Durability & recovery | D6–D10 | **Met** (process-kill harness) |
| Evolution & unknown | D11–D14 | **Met** |
| Product features | D15–D24 | **Met** (foundation depth) + **P1 GUI shell** |
| Quality | D25–D28 | **Met** |

Evidence: automated workspace tests, recovery suite, budget CI, Wasmtime, TLS sync, structured DOCX/PDF, encryption, WPAI branding, egui shell under MSRV 1.85.

---

## What is done

- Command → Ops → Journal → Kernel + dry-run  
- `.qfdoc` ZIP+CBOR, atomic LKG, QFENC1 encryption  
- Recovery (synthetic + process-kill harness)  
- Layout goldens, headless render  
- Export: MD/HTML/TXT, multi-page PDF, structured DOCX  
- Collab CRDT + presence; SyncHub; TCP + TLS QFSNC1  
- Plugins (capabilities + Wasmtime); AI suggestion-only  
- CI budgets; Constitution v5.0.1; WPAI brand  
- **Word-class product surface model** (`ProductSurface`, `docs/28-product-surface.md`)  
- **P1 native GUI shell** (`eframe`/`egui` 0.27 glow-only): command bar, page canvas, status, type→journal→save  

---

## What remains (ordered)

1. **P2 print-layout fidelity** (WYSIWYG page ≈ PDF pagination)  
2. **P3 Review UI** (suggestions accept/reject)  
3. **P4 DOCX round-trip import**  
4. **WebSocket or browser sync** (TLS TCP already ships)  
5. **Rich media in DOCX/PDF** (images, real tables, fonts)  
6. **Semantic search** (token search ships)  
7. **Multi-OS CI matrix** (Linux CI primary; Windows local)  

---

## How to re-verify

```bash
pwsh -File "AGENTS Constitution/tools/verify-pack.ps1"
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace budget_ -- --nocapture
cargo run -p qf-app -- demo
cargo run -p qf-app -- surface
cargo run -p qf-app -- edit
cargo run -p qf-app -- tls-sync-demo
```

MSRV: rustc **1.85.0**. GUI stack pinned: `eframe`/`egui` 0.27 glow-only; lock `url` **2.5.2** (avoids icu 2.2 / rustc 1.86).

---

*Update this document when a band moves more than ~5% or a residual closes.*
