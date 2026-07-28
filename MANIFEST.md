# Delivery MANIFEST — Word-class product surface priority

**Title:** Prioritize Word-class editor UX without Microsoft branding  
**Date:** 2026-07-28  
**Law:** AGENTS Constitution v5.0.0 · ADR-026 · PRD 1.1.0

## Summary

1. `docs/28-product-surface.md` — binding product priority.  
2. `qf_app::ProductSurface` — page view, command bar tabs/groups, status model.  
3. `Theme::document_workspace_light` — paper chrome + WPAI ember (not Office blue).  
4. PRD/README/status updated; `quillforge surface` demo.

## Verify

```bash
cargo test -p qf-app --lib
cargo test -p qf-theme
cargo run -p qf-app -- surface
cargo clippy --workspace --all-targets -- -D warnings
```

## Self-audit

CONST-GATE-001 pass — complete law + tested model; GUI shell is next P1, not stubbed as fake UI.
