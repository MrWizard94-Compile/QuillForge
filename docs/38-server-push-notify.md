# Server-Push Head Notify

**Document ID:** QF-SYNC-NOTIFY-001  
**Status:** Foundation  
**Law:** Content still enters only via Pull → command/journal/kernel  

---

## Problem

Clients should not spin continuous `Pull` requests when the hub head is idle.
Peer writes should surface as **notifies**; the client then Pulls once.

---

## Protocol

| Message | Role |
|---------|------|
| `Subscribe { token }` | Opt-in for head-advance events on the session document |
| `Subscribed { server_head_seq }` | Ack + current head |
| Peer `Push` advances head | Hub enqueues `HeadNotify` for other **subscribed** sessions |
| `DrainNotify { token }` | Non-blocking drain of mailbox |
| `Notify { events[] }` | Zero or more `HeadNotify` |

`HeadNotify` fields: `document_id`, `server_head_seq`, `from_actor`.

Publisher does not receive its own notify. Unsubscribed sessions get an empty drain.

---

## Surfaces

| Surface | Entry |
|---------|--------|
| Hub | `SyncHub::subscribe` / `drain_notifies` / `register_live_notify` |
| Client helper | `SyncClient::subscribe` / `drain_notifies` |
| Wire | `WireRequest::Subscribe` / `DrainNotify` |
| TCP | `TcpSyncClient::subscribe` / `drain_notify` (mailbox drain) |
| WS / WSS | Concurrent workers + live channel → **unsolicited** `Notify` frames |
| WS client | `WsSyncSession::subscribe` / `wait_notify` |

Notify is a **signal**, not content. After notify, call `Pull` and merge via the normal path.

### Live (unsolicited) path

1. WS accept spawns **one thread per connection**.  
2. On `Subscribe`, the serve loop registers an `mpsc` sender on the hub.  
3. Peer `Push` fan-out fills the mailbox **and** `send`s on live channels.  
4. The idle serve loop (short read timeout) flushes pending events as `WireResponse::Notify` without a client request.  
5. Connection close unregisters the live channel.

---

## Verify

```bash
cargo test -p qf-sync subscribe_receives_notify -- --nocapture
cargo test -p qf-sync tcp_subscribe_notify -- --nocapture
cargo test -p qf-sync ws_unsolicited_notify -- --nocapture
```

---

## Out of scope

- Presence fan-out over the same mailbox  
- Browser client subscribe UI  
- Backpressure / notify coalescing beyond batch-on-flush  
