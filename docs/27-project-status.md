# Quillforge Project Status

**Document ID:** QF-STATUS-001  
**Date:** 2026-07-28  
**Studio:** WPAI Software · Wizard Productions AI Studio  
**Current law:** [WPAI Constitution](https://github.com/MrWizard94-Compile/Agents-Constitution), via the project `AGENTS.md`
**Git tag:** `v0.1.0-foundation`  
**Package format:** `1.0.0`

---

## Completion percentage (honest layers)

The percentages and product claims below are historical July 28 estimates,
not current acceptance evidence. Published `main` failed its format gate
before this governance migration; see `governance-adoption.md` for current
verification. No product-completion percentage is advanced by changing law.

| Layer | Scope | Completion | Notes |
|-------|--------|------------|--------|
| **Foundation DoD** | Ship checklist D1–D28 | **~94%** | Core Met; docs/API honesty remediated; some command expanders still foundation-scoped |
| **Local-first core product** | Offline edit, package, recovery, export, plugins, AI, sync, search | **~96%** | DrawingML DOCX images; open/export matrix |
| **Marketing / “v1 product”** | Polish, multi-OS CI, browser UX | **~95%** | Word-class shell (egui); software GPU path for composition tests |
| **Long-term vision** | Full document OS, multiplayer UX, neural packs, **native wgpu** | **~78%** | QFEMB1 ONNX pack; **wgpu not started**; collab foundation only |

**Headline for directors:**  
**Foundation shippable** (edit · journal · recovery · export · search · sync · plugins · AI accept).  
**Not claimed:** native wgpu GPU, full multiplayer UX, every Command enum expander. Overall vision **~78%**.

---

## What remains (ordered)

1. Complete dead `Command` expanders (split/merge/comment/remote batch) or remove from public enum  
2. Native `wgpu` surface (optional; software GPU already complete for headless)  
3. PDF import (optional; currently export-only)  
4. Vendor ORT/tract full ONNX (optional, native)  
5. Full OT / true range marks / face embedding  
6. Browser e2e / presence fan-out polish  

---

## How to re-verify

```bash
python tools/check-constitution-adoption.py --source <verified-canonical-check-adoption.py>
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- search-demo
```

MSRV: rustc **1.85.0**.

---

*Update this document when a band moves more than ~5% or a residual closes.*
