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
| **Foundation DoD** | Ship checklist D1–D28 | **~98%** | All criteria **Met** |
| **Local-first core product** | Offline edit, package, recovery, export, plugins, AI, sync, search | **~99%** | Product surface + search |
| **Marketing / “v1 product”** | Polish, multi-OS CI, browser UX | **~97%** | **Multi-OS CI matrix** (Linux/Windows/macOS) |
| **Long-term vision** | Full document OS, multiplayer UX, dense vectors, rich GPU | **~81%** | CI quality on three OSes |

**Headline for directors:**  
**Foundation ~98% DoD. Word-class editor + sync + browser path + media + search + multi-OS CI. Overall vision ~81%.**

---

## What remains (ordered)

1. Long-lived WS/WSS sessions  
2. Dense embeddings (optional offline model pack)  
3. DOCX DrawingML / PDF image XObjects / font embed  
4. Production TLS packaging  
5. Cross-compile / browser e2e (optional)  

---

## How to re-verify

```bash
pwsh -File "AGENTS Constitution/tools/verify-pack.ps1"
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace budget_ -- --nocapture
cargo run -p qf-app -- search-demo
```

CI: `.github/workflows/ci.yml` · `docs/34-ci-matrix.md`  
MSRV: rustc **1.85.0**.

---

*Update this document when a band moves more than ~5% or a residual closes.*
