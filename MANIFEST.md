# Delivery MANIFEST — TLS QFSNC1 transport

**Title:** rustls TLS 1.3 adapter for SyncHub / QFSNC1  
**Date:** 2026-07-28  
**Law:** AGENTS Constitution v5.0.0 · `specialist/NETWORKING.md` · SEC-INPUT-001

## Summary

1. Shared `wire.rs` frame codec (QFSNC1 CBOR).  
2. `TlsIdentity` + `TlsSyncServer` / `TlsSyncClient` (rustls, ring provider).  
3. Dev self-signed certs; tests + `quillforge tls-sync-demo`.  
4. ADR-023; ship checklist D21 TLS closed.

## Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- tls-sync-demo
```

## Self-audit

CONST-GATE-001 pass: complete, tested, documented, zero warnings.
