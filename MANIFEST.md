# Delivery MANIFEST — M10 AI + foundation ship

**Title:** AI suggestion-only flow, hardening, CI, v1 foundation checklist  
**Date:** 2026-07-27  
**Law:** `AGENTS Constitution/AGENTS.md` + `SOP.md`

## Summary

Completed M10: permissioned AI proposals, accept/reject via journal only, audit without bodies, rate/selection limits, security unit tests, GitHub CI, and ship checklist mapping DoD criteria to evidence.

## Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- demo
```

## Commits this turn

1. AI suggestion accept path  
2. Ship checklist + CI + docs  

## Self-audit

Section 0 pass for M10. Full marketing v1 still has residuals (see checklist). Foundation product brick is complete and shippable as `v0.1.0-foundation`.

## Residual (accepted)

Process-kill matrix, binary PDF/DOCX, Wasmtime guest, network sync server, Argon2 package encrypt UX.
