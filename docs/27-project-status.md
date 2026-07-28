# Quillforge Project Status

**Document ID:** QF-STATUS-001  
**Date:** 2026-07-28  
**Studio:** WPAI Software · Wizard Productions AI Studio  
**Law:** AGENTS Constitution pack v5.0.0  
**Git tag:** `v0.1.0-foundation`  
**Package format:** `1.0.0`

---

## Completion percentage (honest layers)

| Layer | Scope | Completion | Notes |
|-------|--------|------------|--------|
| **Foundation DoD** | Ship checklist D1–D28 (`docs/ship/v1-checklist.md`) | **~98%** | All criteria **Met**; residual polish items are optional/depth |
| **Local-first core product** | Offline edit, package, recovery, export, plugins, AI path, sync+TLS | **~92%** | Tagged foundation; no GUI shell |
| **Marketing / “v1 product”** | Polish exports, multi-OS kill CI, browser sync, UX | **~75%** | Full tables/media fonts, WebSocket, native GUI deferred |
| **Long-term vision** | Full document OS, multiplayer product UX, semantic search, rich GPU | **~55%** | Architecture in place; product surface still CLI/headless |

**Headline for directors:**  
**Foundation is shippable (~98% of binding DoD). Product priority is now a Word-class editor shell (no Microsoft branding) on that core. Overall vision ~55–60% until GUI ships.**

### Product surface priority (director)

| Priority | Target |
|----------|--------|
| **User appearance** | Upgraded Word-class document editor |
| **Brand** | WPAI only — never Microsoft/Word/Office marks in chrome |
| **Engine** | Unchanged: Command → Journal → Kernel, `.qfdoc` |
| **Law** | `docs/28-product-surface.md` · ADR-026 |

---

## DoD scoreboard (D1–D28)

| Band | IDs | Status |
|------|-----|--------|
| Canonical core | D1–D5 | **Met** |
| Durability & recovery | D6–D10 | **Met** (process-kill harness: see residual close) |
| Evolution & unknown | D11–D14 | **Met** |
| Product features | D15–D24 | **Met** (foundation depth) |
| Quality | D25–D28 | **Met** |

Evidence: automated workspace tests, recovery suite, budget CI, Wasmtime, TLS sync, structured DOCX/PDF, encryption, WPAI branding.

---

## What is done

- Command → Ops → Journal → Kernel + dry-run  
- `.qfdoc` ZIP+CBOR, atomic LKG, QFENC1 encryption  
- Recovery (synthetic + process-kill harness)  
- Layout goldens, headless render  
- Export: MD/HTML/TXT, multi-page PDF, structured DOCX  
- Collab CRDT + presence; SyncHub; TCP + TLS QFSNC1  
- Plugins (capabilities + Wasmtime); AI suggestion-only  
- CI budgets; Constitution v5.0.0; WPAI brand  

## What remains (ordered)

1. **Native / GUI editor shell** (largest product gap)  
2. **WebSocket or browser sync** (TLS TCP already ships)  
3. **Rich media in DOCX/PDF** (images, real tables, fonts)  
4. **Semantic search** (token search ships)  
5. **Non-empty migration matrix** beyond format 1.0.0 identity path  
6. **Multi-OS CI matrix** (Linux CI primary; Windows local)  

---

## How to re-verify

```bash
pwsh -File "AGENTS Constitution/tools/verify-pack.ps1"
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace budget_ -- --nocapture
cargo run -p qf-app -- demo
cargo run -p qf-app -- tls-sync-demo
```

---

*Update this document when a band moves more than ~5% or a residual closes.*
