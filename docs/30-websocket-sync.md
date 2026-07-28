# WebSocket Sync Transport (QFSNC1)

**Document ID:** QF-SYNC-WS-001  
**Status:** Foundation transport  
**Protocol:** QFSNC1 (same as TCP/TLS)  

---

## Summary

Browser-ready path for multi-client sync:

| Layer | Detail |
|-------|--------|
| **`ws://`** | `ws://host:port/qfsnc1` — plain TCP + RFC 6455 |
| **`wss://`** | `wss://host:port/qfsnc1` — rustls TLS 1.3 + RFC 6455 |
| **Handshake** | HTTP upgrade on `/qfsnc1` (after TLS when using WSS) |
| **Data** | One WebSocket **binary** message = one QFSNC1 frame (`QFSNC1\0\0` + len + CBOR) |
| **Hub** | Same in-process `SyncHub` as TCP/TLS |
| **API** | `WsSyncServer`/`WsSyncClient` · `WssSyncServer`/`WssSyncClient` |

Presence remains a separate non-durable stream (hub isolation unchanged).

### Stack (WSS)

```text
TCP → rustls TLS 1.3 → WebSocket upgrade → binary QFSNC1
```

Dev/test identities: `TlsIdentity::generate_dev()` (SAN localhost / 127.0.0.1).

---

## Verify

```bash
cargo test -p qf-sync ws_ -- --nocapture
cargo test -p qf-sync wss_ -- --nocapture
cargo run -p qf-app -- ws-sync-demo
cargo run -p qf-app -- wss-sync-demo
```

---

## Browser reference client

See **`docs/31-browser-sync-client.md`** and `web/qfsnc1-client/`.

```bash
cargo run -p qf-app -- browser-sync-demo
```

## Out of scope (next)

- Long-lived multiplexed sessions (current client opens a connection per request)  
- Production CA / ALPN packaging  
- Hosted HTTPS + `wss://` browser demo  
