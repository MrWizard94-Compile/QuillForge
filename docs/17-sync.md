# Sync Architecture Specification

**Document ID:** QF-SYNC-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Principles

- Offline-first queue  
- Eventual consistency  
- **CRDT preferred**; OT fallback allowed for specific subsystems if needed  
- Separate content and presence streams  
- Sync NEVER bypasses journal/kernel  

---

## 2. Offline queue

```rust
pub struct SyncQueue {
    pub pending: VecDeque<SyncItem>,
    pub durable_path: PathBuf, // survives restart
}

pub struct SyncItem {
    pub id: Uuid,
    pub document_id: DocumentId,
    pub op_seq_range: RangeInclusive<u64>,
    pub payload_hash: Hash256,
    pub created_at: Timestamp,
    pub attempts: u32,
}
```

- Enqueue after successful local commit.  
- Durable queue file with fsync.  
- Exponential backoff on failure.  

---

## 3. Protocol (logical)

```
Hello { client, doc_id, vector_clock/op_heads }
→ HelloAck { server_heads, acl_epoch }

Pull { have: Heads } → Ops { ops }
Push { ops } → PushAck { accepted_heads } | Reject

Subscribe { token } → Subscribed { server_head_seq }
  (peer Push) → HeadNotify enqueued for other subscribers
DrainNotify { token } → Notify { events }

PresenceSubscribe → PresenceStream
```

Transport: HTTPS/WebSocket over TLS 1.3.  
Head notify: `docs/38-server-push-notify.md`.

---

## 4. Consistency model

- Each replica has linear journal.  
- Merge function integrates remote ops into local journal as new commits with causal metadata.  
- Convergence: same set of ops (by op_id) → same canonical state (CRDT property).  

```rust
pub trait MergeEngine {
    fn integrate(
        &self,
        local: &CanonicalState,
        remote_ops: &[Operation],
    ) -> Result<Vec<Operation>, SyncError>;
}
```

Integrated ops still pass command/commit path for durability.

---

## 5. CRDT vs OT

| Approach | Use |
|----------|-----|
| CRDT (primary) | Text positions, concurrent inserts/deletes |
| OT (fallback) | Only if a subsystem cannot be CRDTized; must be documented |

v1 text collab: CRDT. Structural tree: concurrent move resolution with deterministic tie-break (op_id order).

---

## 6. Content vs presence

| | Content | Presence |
|--|---------|----------|
| Transport channel | `/sync/content` | `/sync/presence` |
| Durability | Yes | No |
| Backpressure | Queue | Drop old updates |
| Authz | ACL write | ACL read |

---

## 7. Security

- Auth tokens / mTLS  
- Server enforces ACL  
- Client re-validates  
- Encrypted packages: sync ciphertext or unlock locally before merge (enterprise policy)  

---

## 8. Conflict UX

- Auto-merge when CRDT clean  
- Surface unresolvable structural conflicts as suggestions for user  
- Never duplicate-apply same `op_id`  

---

## 9. Testing

- Partition for N minutes; heal; converge  
- Duplicate delivery idempotency  
- Queue crash durability  
- Presence does not create journal records  

---

*End of sync specification.*
