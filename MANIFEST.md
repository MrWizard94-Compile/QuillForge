# Delivery MANIFEST — Residual hardening (encrypt, recovery, PDF, DOCX)

**Title:** At-rest encryption, recovery suite, deterministic PDF + OOXML DOCX  
**Date:** 2026-07-27  
**Law:** `AGENTS Constitution/AGENTS.md` + `SOP.md`

## Summary

Closed foundation residuals accepted at M10:

1. **Recovery suite** — torn journal frames, incomplete staging, LKG open, encrypted open (`crates/qf-storage/tests/recovery.rs`).
2. **Package encryption** — Argon2id KDF + XChaCha20-Poly1305 AEAD (`QFENC1` wire format) with `OpenOptions::with_passphrase` and encrypted save APIs.
3. **Minimal PDF** — deterministic PDF 1.4 Helvetica multi-line writer (`qf-export::pdf`).
4. **Minimal DOCX** — deterministic OOXML package (`qf-export::docx`).

## Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- demo
```

## Commits this delivery

1. Argon2id + XChaCha20-Poly1305 package encryption  
2. Recovery power-loss style tests  
3. Minimal deterministic PDF export  
4. Minimal deterministic OOXML DOCX export  
5. Docs / checklist / ADR updates  

## Self-audit

Section 0 pass for residual slice. Clippy `-D warnings` and full workspace tests green.  
Still deferred for marketing v1: Wasmtime guest runtime, network sync server, OS process-kill harness.

## Residual (updated)

| Residual | Status |
|----------|--------|
| Process-kill matrix on all OS | Accepted; synthetic recovery suite covers torn/LKG paths |
| Binary PDF | **Met** |
| OOXML DOCX | **Met** (minimal) |
| Wasmtime guest | Residual |
| Network sync server | Residual (local-first core complete) |
| Argon2 package encrypt | **Met** |
