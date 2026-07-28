# Delivery MANIFEST — Constitution v4.1 + TCP sync transport

**Title:** Modular AGENTS pack + localhost TCP framed SyncHub transport  
**Date:** 2026-07-28  
**Law:** `AGENTS Constitution/AGENTS.md` v4.1 + `SOP.md` + always-load modules

## Summary

1. **AGENTS Constitution v4.1** modular pack committed; `verify-pack.ps1` PASS (GOV-INT-001).
2. **TcpSyncServer / TcpSyncClient** — real loopback sockets, `QFSNC1` frames, CBOR body, 16 MiB budget.
3. **CLI** — `quillforge sync-demo` exercises multi-client Hello/Push/Pull over TCP.
4. **ADR-019** — wire format decision.

## Verify

```bash
pwsh -File "AGENTS Constitution/tools/verify-pack.ps1"
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- demo
cargo run -p qf-app -- sync-demo
```

## Self-audit (CONST-GATE-001)

| # | Result |
|---|--------|
| Completeness | Pass — no stubs in new paths |
| Zero warnings | Pass — clippy `-D warnings` |
| Tests | Pass — qf-sync TCP + full workspace |
| Docs | Pass — MANIFEST, ADR-019, checklist |
| Security | Pass — max frame, magic check, loopback only in demos |
| Review packaging | Pass — this MANIFEST |

## Deferred

| Item | Notes |
|------|--------|
| TLS/WebSocket WAN adapter | Protocol + TCP proven |
| OS process-spawn kill harness | Synthetic matrix binding (ADR-018) |
| Rich PDF/DOCX styles | Minimal writers ship |
