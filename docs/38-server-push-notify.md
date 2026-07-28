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
| Hub | `SyncHub::subscribe` / `drain_notifies` |
| Client helper | `SyncClient::subscribe` / `drain_notifies` |
| Wire | `WireRequest::Subscribe` / `DrainNotify` |
| TCP | `TcpSyncClient::subscribe` / `drain_notify` |

Notify is a **signal**, not content. After notify, call `Pull` and merge via the normal path.

---

## Verify

```bash
cargo test -p qf-sync subscribe_receives_notify -- --nocapture
cargo test -p qf-sync tcp_subscribe_notify -- --nocapture
```

---

## Out of scope

- Unsolicited multi-connection socket push without DrainNotify  
- Presence fan-out over the same mailbox  
- Browser client subscribe UI  
