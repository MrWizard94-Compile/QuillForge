# Browser QFSNC1 Reference Client

**Document ID:** QF-SYNC-BROWSER-001  
**Status:** Reference client  
**Brand:** WPAI  

---

## What it is

A zero-build **HTML + JS** client that speaks the same **QFSNC1** protocol as Rust:

| Item | Detail |
|------|--------|
| **Path** | `web/qfsnc1-client/` (`index.html`, `qfsnc1.js`) |
| **Transport** | WebSocket binary frames to `ws://host:port/qfsnc1` |
| **Framing** | `QFSNC1\0\0` + u32 LE length + CBOR (serde/ciborium compatible) |
| **Demo** | `cargo run -p qf-app -- browser-sync-demo` |

The demo starts a `WsSyncServer` and a tiny HTTP server that embeds the client page with the correct `?ws=` query parameter.

---

## Verify

```bash
# CBOR golden shared with Rust
# (Node optional)
node -e "require('fs'); /* or open page and click Self-test CBOR */"

cargo test -p qf-sync hello_fixed -- --nocapture
cargo test -p qf-app embedded_assets -- --nocapture
cargo run -p qf-app -- browser-sync-demo
# Open printed http://127.0.0.1:…/?ws=ws://…/qfsnc1
# Click “Hello · Push · Pull”
```

---

## Compatibility notes

- UUID fields are **16-byte CBOR byte strings** (not JSON UUID strings on the wire).
- `Operation::Noop` uses serde enum map form: `{ "Noop": { "reason": { "Other": "…" } } }`.
- `payload_hash` is a 32-byte CBOR byte string.
- Client opens **one WebSocket per request** (matches current Rust `WsSyncClient`).

---

## Out of scope

- `wss://` from file:// pages (needs trusted TLS + HTTPS page origin)  
- Long-lived multiplexed sessions  
- Full CRDT / all Operation variants in JS  
