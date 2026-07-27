# Collaboration Architecture Specification

**Document ID:** QF-COL-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Scope

- Presence  
- Cursor tracking  
- Comments  
- Suggestions  
- Shared editing  
- Conflict handling  

Collaboration **never** writes canonical state except through the command/journal/kernel path.

---

## 2. Streams separation

| Stream | Durable? | Contents |
|--------|----------|----------|
| Content | Yes (journal) | Ops that mutate document |
| Presence | No | Cursors, selections, user online status |
| Comments | Yes (entities via ops) | Comment threads |
| Suggestions | Yes (entities via ops) | Proposed op batches |

---

## 3. Presence & cursors

```rust
pub struct PresenceState {
    pub session_id: SessionId,
    pub actor_id: ActorId,
    pub display_name: String,
    pub color: ColorId,
    pub cursor: Option<TextTarget>,
    pub selection: Option<DocRangeStable>,
    pub last_seen: Timestamp,
}

pub enum PresenceEvent {
    Join(PresenceState),
    Update(PresenceState),
    Leave { session_id: SessionId },
}
```

- Broadcast at low rate (e.g. 10–20 Hz max, coalesced).  
- Not journaled.  
- On reconnect, presence rebuilds from peers.

---

## 4. Comments

Created via `Command::AddComment` → ops → kernel entity table.

- Anchors stored as stable block/offset  
- On text edit, anchors transform via op application rules (sticky offsets)  
- Resolve/unresolve are ops  
- Threading via `parent: Option<CommentId>`  

---

## 5. Suggestions

```rust
pub struct Suggestion {
    pub id: SuggestionId,
    pub author: ActorId,
    pub note: Option<String>,
    pub proposed_ops: Vec<Operation>,
    pub status: SuggestionStatus,
    pub source: SuggestionSource,
}
```

- **Accept** runs proposed ops through dry-run + commit as the accepting actor (with attribution to original author in metadata).  
- **Reject** marks status; no content change.  
- AI suggestions use same structure (`SuggestionSource::Ai`).  

---

## 6. Shared editing model

**Decision:** CRDT-friendly operation model with sequential per-replica journal, merged via sync layer (see sync spec).

Local UX:

1. User edits → local command commit immediately (offline-first).  
2. Ops enqueue for sync.  
3. Remote ops arrive → transform/merge → commit as `ActorId` of remote via command ingress `ApplyRemoteOps` (authorized).  

```rust
Command::ApplyRemoteBatch {
    remote_actor: ActorId,
    ops: Vec<Operation>,
    remote_parents: Vec<OpId>,
}
```

Handler validates merge result, emits possibly transformed ops.

---

## 7. Conflict handling

| Conflict | Resolution |
|----------|------------|
| Concurrent text insert same offset | CRDT position IDs / fractional indexing — **decision: fuse with explicit position ids in text model for collab mode** |
| Concurrent delete/insert | Delete wins for deleted chars; inserts adjacent survive |
| Concurrent mark changes | Union or last-writer by op timestamp + actor tie-break |
| Structural move vs delete | Delete wins if block gone; move becomes noop op |
| ACL conflict | Reject unauthorized remote batch |

### Text position model (v1 collab)

For collaborative documents, text uses **stable character IDs** (or Fugue/RGA-style) under the hood; single-player local docs MAY use simple offsets with conversion on first share.

**Safer path chosen:** Implement rich-text CRDT (RGA/Fugue) in `qf-collab` mapping to operations the kernel understands (`InsertText` with position ids). Kernel stores text as CRDT-aware structure when `document.collab_mode = true`.

Simpler non-collab documents keep offset model; enabling share migrates text to CRDT representation via explicit migration op.

---

## 8. Attribution

Every content op carries `actor_id`. UI can display author colors from journal. Suggestions preserve proposer id on accept.

---

## 9. Enterprise review workflow

States: `Draft → InReview → Approved → Published` as document meta via ops.

Reviewers use suggestions/comments; force-edit optional by role.

Audit events for accept/reject/publish.

---

## 10. Testing

- Two-replica concurrent edit convergence  
- Presence isolation from journal  
- Suggestion accept dry-run failure path  
- Anchor stability under edits  

---

*End of collaboration specification.*
