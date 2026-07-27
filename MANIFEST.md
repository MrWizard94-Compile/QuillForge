# Delivery MANIFEST — Residual hardening + Wasmtime

**Title:** Encryption, recovery, PDF/DOCX, session encrypt API, Wasmtime guest runtime  
**Date:** 2026-07-27  
**Law:** `AGENTS Constitution/AGENTS.md` + `SOP.md`

## Summary

Closed foundation residuals:

1. Recovery suite (torn journal, staging, LKG, encrypted open)
2. QFENC1 package encryption (Argon2id + XChaCha20-Poly1305)
3. Deterministic PDF 1.4 + OOXML DOCX exporters
4. Session/editor encrypted save/open + demo path
5. **Wasmtime guest runtime** with fuel and capability-gated hostcalls

## Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- demo
```

## Remaining marketing-v1 residuals

| Residual | Status |
|----------|--------|
| OS process-kill matrix | Accepted; synthetic recovery suite covers torn/LKG |
| Network sync server | Residual (local-first core + offline queue complete) |
| Full rich PDF/DOCX styles | Deferred (minimal writers ship) |

## Self-audit

Section 0 pass. Clippy `-D warnings` and workspace tests green for delivered crates.
