# Delivery MANIFEST — WPAI branding alignment

**Title:** Quillforge branded as WPAI Software product  
**Date:** 2026-07-28  
**Law:** AGENTS Constitution v4.1 + company Brand voice (`brand/BRAND-VOICE.md`)

## Summary

1. `qf_core::brand` — WPAI / W.P.A.I constants, CLI banner, package identity helper.  
2. `Theme::wpai_forge_dark` — coal/ember forge palette as product default.  
3. CLI help/version chrome with studio thesis and disclosure.  
4. `brand/` kit with official marks + voice (from `C:\WPAI\Brand`).  
5. README, CHANGELOG, ADR-022, Cargo authors.

## Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- version
cargo run -p qf-app -- help
```

## Self-audit

CONST-GATE-001: complete branding surfaces, tests pass, no stubs, docs synced.
