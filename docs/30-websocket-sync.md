# WebSocket Sync Transport (QFSNC1)

**Document ID:** QF-SYNC-WS-001  
**Status:** Foundation transport  
**Protocol:** QFSNC1 (same as TCP/TLS)  

---

## Summary

Browser-ready path for multi-client sync:

| Layer | Detail |
|-------|--------|
| **URL** | `ws://host:port/qfsnc1` |
| **Handshake** | RFC 6455 HTTP upgrade |
| **Data** | One WebSocket **binary** message = one QFSNC1 frame (`QFSNC1\0\0` + len + CBOR) |
| **Hub** | Same in-process [`SyncHub`] as TCP/TLS |
| **API** | `WsSyncServer` / `WsSyncClient` in `qf-sync` |

Presence remains a separate non-durable stream (hub isolation unchanged).

---

## Verify

```bash
cargo test -p qf-sync ws_ -- --nocapture
cargo run -p qf-app -- ws-sync-demo
```

---

## Out of scope (next)

- `wss://` (TLS + WebSocket)  
- Long-lived multiplexed sessions (current client opens a connection per request, like TCP client)  
- Browser JS reference client  
