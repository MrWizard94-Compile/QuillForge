# Delivery MANIFEST — Sync hub + durability matrix

**Title:** In-process SyncHub, durable offline queue, expanded recovery  
**Date:** 2026-07-27  
**Law:** `AGENTS Constitution/AGENTS.md` + `SOP.md`

## Summary

1. **DurableSyncQueue** — fsync-backed offline queue (`QFSYNCQ1`), reopen + backoff.
2. **SyncHub** — multi-replica logical server: Hello / Pull / Push, presence stream isolation, idempotent push, offline queue flush.
3. **Recovery expansion** — process-kill style handle-forget after commit; partial-frame truncation.
4. ADRs 017–018 + ship checklist residual closures.

## Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- demo
```

## Remaining deferred (non-blocking)

| Item | Notes |
|------|--------|
| TLS/WebSocket transport adapter | Protocol complete in-process |
| OS process-spawn kill harness | Synthetic matrix binding per ADR-018 |
| Rich PDF/DOCX styles | Minimal writers ship |

## Self-audit

Section 0 pass. Clippy `-D warnings` and full workspace tests green.
