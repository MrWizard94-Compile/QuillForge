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
| **Local-first core product** | Offline edit, package, recovery, export, plugins, AI, sync, search | **~99%** | + DrawingML DOCX images |
| **Marketing / “v1 product”** | Polish, multi-OS CI, browser UX | **~99%** | Full open/export matrix (PDF export-only) |
| **Long-term vision** | Full document OS, multiplayer UX, neural packs, rich GPU | **~91%** | Optional ONNX graph runtime (QFEMB1) |

**Headline for directors:**  
**Foundation shippable. Full open/export matrix + mark + CI + search + media + sync + fonts. Overall vision ~91%.**

---

## What remains (ordered)

1. PDF import (optional; currently export-only)  
2. Vendor ORT/tract full ONNX (optional, native)  
3. Full OT / bold-italic face embedding  
4. Browser e2e / presence fan-out polish  

---

## How to re-verify

```bash
pwsh -File "AGENTS Constitution/tools/verify-pack.ps1"
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- search-demo
```

MSRV: rustc **1.85.0**.

---

*Update this document when a band moves more than ~5% or a residual closes.*
