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
| **Transport** | WebSocket binary frames to `ws://` or `wss://…/qfsnc1` |
| **Framing** | `QFSNC1\0\0` + u32 LE length + CBOR (serde/ciborium compatible) |
| **HTTP demo** | `cargo run -p qf-app -- browser-sync-demo` |
| **HTTPS+WSS demo** | `cargo run -p qf-app -- browser-wss-demo` |

### Demos

| Command | Page | Hub |
|---------|------|-----|
| `browser-sync-demo` | `http://127.0.0.1:…/` | `ws://…/qfsnc1` |
| `browser-wss-demo` | `https://localhost:…/` | `wss://localhost:…/qfsnc1` |

`browser-wss-demo` uses one self-signed `TlsIdentity` for both HTTPS and WSS, writes PEM to `target/quill-browser-wss/dev-localhost.pem`, and prints the URL. Accept the browser certificate warning for local demos.

Duration defaults to 120s; override with `QUILL_BROWSER_DEMO_SECS`.

---

## Verify

```bash
cargo test -p qf-sync hello_fixed -- --nocapture
cargo test -p qf-app https_serves -- --nocapture
$env:QUILL_BROWSER_DEMO_SECS=5; cargo run -p qf-app -- browser-wss-demo
# Open printed https://localhost:…/?ws=wss://…/qfsnc1
# Accept self-signed warning → “Hello · Push · Pull”
```

---

## Compatibility notes

- UUID fields are **16-byte CBOR byte strings** (not JSON UUID strings on the wire).
- `Operation::Noop` uses serde enum map form: `{ "Noop": { "reason": { "Other": "…" } } }`.
- `payload_hash` is a 32-byte CBOR byte string.
- Client opens **one WebSocket per request** (matches current Rust `WsSyncClient` / `WssSyncClient`).

---

## Out of scope

- Production CA / public trust  
- Long-lived multiplexed sessions  
- Full CRDT / all Operation variants in JS  
