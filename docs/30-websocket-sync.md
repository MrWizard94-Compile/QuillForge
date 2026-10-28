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
| **API** | `WsSyncServer`/`WsSyncClient`/`WsSyncSession` · `WssSyncServer`/`WssSyncClient`/`WssSyncSession` |
| **Sessions** | One TCP/TLS+WS handshake; many Hello/Push/Pull pairs until peer close |

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
cargo run -p qf-app -- browser-sync-demo      # http + ws
cargo run -p qf-app -- browser-wss-demo       # https + wss (dev cert)
```

## Long-lived sessions

```rust
let mut session = WsSyncClient::connect(addr).open_session()?;
let ack = session.hello(hello)?;
session.push(push_req)?;
session.pull(pull_req)?;
// drop(session) closes the socket; server ends the accept loop for that peer
```

Same pattern for `WssSyncClient::open_session`. One-shot `hello`/`push`/`pull` on the client types still open a short session per call.

### Receive lifecycle and timeout contract

Each connection owns one incremental reader, shared by the WS and WSS paths.
It retains header, extended-length, mask, and payload offsets across idle
reads. `WouldBlock` and `TimedOut` are classified by `std::io::ErrorKind`, not
localized OS error text; `Interrupted` retries the same read. EOF, close frames,
fatal I/O, malformed QFSNC1, and oversized frames are not idle retries.
The existing 16 MiB WebSocket payload cap is checked before allocation.

Server TLS/HTTP upgrade uses a 30-second socket read timeout. Only after the
upgrade does the server use the 50 ms live-notify poll. These are per-read
socket timeouts, not whole-handshake or whole-message deadlines.
`wait_notify(timeout)` returns an empty batch on an idle read, retains any
partial notification for the next call, and restores the previous socket read
timeout on success or failure. A timed-out request/response call is still an
error; callers must not assume a timed-out mutation was uncommitted or retry it
blindly. Public `SyncError` variants, message shapes, and method signatures
remain unchanged.

Research checked October 18, 2026 against the pinned Rust 1.85.0 source:
[`TcpStream` timeout kinds](https://github.com/rust-lang/rust/blob/1.85.0/library/std/src/net/tcp.rs)
and [`Read::read_exact` partial-progress contract](https://github.com/rust-lang/rust/blob/1.85.0/library/std/src/io/mod.rs).
Frame layout follows the existing
[RFC 6455 section 5.2](https://www.rfc-editor.org/rfc/rfc6455.html#section-5.2)
foundation framing. This focused repair does not add fragmented-message
assembly, full control-frame handling, or a broader RFC-compliance claim.

## Live head notify

Subscribed sessions receive unsolicited `Notify` frames when a peer Push advances the hub head (concurrent WS workers + live channels). See `docs/38-server-push-notify.md`.

```rust
session.subscribe(SubscribeRequest { token })?;
// peer push elsewhere…
let batch = session.wait_notify(Duration::from_secs(2))?;
// then Pull and merge via command path
```

## Out of scope (next)

- Production CA / public trust  
- Multiplexed documents on one socket with framing channels  
- Browser client subscribe UI  
