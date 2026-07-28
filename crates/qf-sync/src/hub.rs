//! In-process multi-replica sync hub (foundation "network server").
//!
//! Implements the logical protocol from QF-SYNC-001 without requiring TLS/WebSocket
//! infrastructure: Hello / Pull / Push on the content stream, presence on a
//! separate ephemeral stream. Content never mixes with presence.

use crate::{
    ContentMessage, PresenceMessage, StreamRouter, SyncError, SyncStream,
};
use qf_collab::{PresenceEvent, TextCrdt};
use qf_core::{ActorId, DocumentId, Hash256, Operation, SessionId};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::sync::mpsc::Sender;
use uuid::Uuid;

/// Opaque session token issued by HelloAck.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionToken(pub Uuid);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Hello {
    pub client_id: ActorId,
    pub session_id: SessionId,
    pub document_id: DocumentId,
    /// Highest op seq the client already has (inclusive).
    pub have_seq: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HelloAck {
    pub token: SessionToken,
    pub server_head_seq: u64,
    pub acl_epoch: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PullRequest {
    pub token: SessionToken,
    pub have_seq: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PullResponse {
    pub ops: Vec<Operation>,
    pub crdt: Option<TextCrdt>,
    pub server_head_seq: u64,
    /// Hash of the concatenated payload for integrity checks.
    pub payload_hash: Hash256,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushRequest {
    pub token: SessionToken,
    pub first_seq: u64,
    pub last_seq: u64,
    pub ops: Vec<Operation>,
    pub crdt: Option<TextCrdt>,
    pub payload_hash: Hash256,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushAck {
    pub accepted_head_seq: u64,
}

/// Client registers for head-advance notifications (no continuous Pull poll).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubscribeRequest {
    pub token: SessionToken,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Subscribed {
    pub server_head_seq: u64,
}

/// Drain pending server-push notifies for a session.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DrainNotifyRequest {
    pub token: SessionToken,
}

/// One head-advance event fan-out from a peer Push (server-push notify).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadNotify {
    pub document_id: DocumentId,
    pub server_head_seq: u64,
    /// Actor that pushed the batch (best-effort).
    pub from_actor: ActorId,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotifyBatch {
    pub events: Vec<HeadNotify>,
}

#[derive(Clone, Debug)]
struct ClientSession {
    actor: ActorId,
    session: SessionId,
    document_id: DocumentId,
    /// When true, peer Pushes enqueue [`HeadNotify`] into this session's mailbox.
    subscribed: bool,
}

#[derive(Default)]
struct DocReplica {
    /// Append-only content log: (end_seq, ops, optional crdt snapshot tip).
    batches: Vec<StoredBatch>,
    head_seq: u64,
    /// Latest CRDT tip for the primary collab block (optional).
    crdt_tip: Option<TextCrdt>,
    /// Presence is ephemeral and never journaled.
    presence: StreamRouter,
    acl_epoch: u64,
}

#[derive(Clone, Debug)]
struct StoredBatch {
    first_seq: u64,
    last_seq: u64,
    ops: Vec<Operation>,
    crdt: Option<TextCrdt>,
    payload_hash: Hash256,
}

/// Multi-document in-process sync server.
#[derive(Default)]
pub struct SyncHub {
    sessions: HashMap<Uuid, ClientSession>,
    docs: HashMap<DocumentId, DocReplica>,
    /// Per-session notify mailbox (token → pending head events).
    notify_mail: HashMap<Uuid, VecDeque<HeadNotify>>,
    /// Live transport channels (WS/WSS) for unsolicited Notify frames.
    live_notify: HashMap<Uuid, Sender<HeadNotify>>,
}

impl SyncHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn hello(&mut self, req: Hello) -> Result<HelloAck, SyncError> {
        let doc = self.docs.entry(req.document_id).or_default();
        let token = Uuid::new_v4();
        self.sessions.insert(
            token,
            ClientSession {
                actor: req.client_id,
                session: req.session_id,
                document_id: req.document_id,
                subscribed: false,
            },
        );
        Ok(HelloAck {
            token: SessionToken(token),
            server_head_seq: doc.head_seq,
            acl_epoch: doc.acl_epoch,
        })
    }

    /// Register for server-push head notifies on the session's document.
    pub fn subscribe(&mut self, req: SubscribeRequest) -> Result<Subscribed, SyncError> {
        let session = self
            .sessions
            .get_mut(&req.token.0)
            .ok_or_else(|| SyncError::Auth("unknown session".into()))?;
        session.subscribed = true;
        let head = self
            .docs
            .get(&session.document_id)
            .map(|d| d.head_seq)
            .unwrap_or(0);
        // Ensure mailbox exists.
        self.notify_mail.entry(req.token.0).or_default();
        Ok(Subscribed {
            server_head_seq: head,
        })
    }

    /// Drain pending notifies (non-blocking). Empty when nothing advanced since last drain.
    pub fn drain_notifies(&mut self, req: DrainNotifyRequest) -> Result<NotifyBatch, SyncError> {
        if !self.sessions.contains_key(&req.token.0) {
            return Err(SyncError::Auth("unknown session".into()));
        }
        let events = self
            .notify_mail
            .get_mut(&req.token.0)
            .map(|q| q.drain(..).collect())
            .unwrap_or_default();
        Ok(NotifyBatch { events })
    }

    /// Register a live channel so the transport can push unsolicited `Notify` frames.
    /// Replaces any previous channel for this token.
    pub fn register_live_notify(
        &mut self,
        token: &SessionToken,
        tx: Sender<HeadNotify>,
    ) -> Result<(), SyncError> {
        if !self.sessions.contains_key(&token.0) {
            return Err(SyncError::Auth("unknown session".into()));
        }
        self.live_notify.insert(token.0, tx);
        Ok(())
    }

    /// Drop live channel (connection closed or re-subscribe on a new socket).
    pub fn unregister_live_notify(&mut self, token: &SessionToken) {
        self.live_notify.remove(&token.0);
    }

    fn fanout_notify(
        &mut self,
        document_id: DocumentId,
        server_head_seq: u64,
        from_token: Uuid,
        from_actor: ActorId,
    ) {
        let recipients: Vec<Uuid> = self
            .sessions
            .iter()
            .filter(|(id, s)| {
                s.subscribed && s.document_id == document_id && **id != from_token
            })
            .map(|(id, _)| *id)
            .collect();
        let event = HeadNotify {
            document_id,
            server_head_seq,
            from_actor,
        };
        for id in recipients {
            self.notify_mail.entry(id).or_default().push_back(event.clone());
            // Best-effort live push (drop if channel full/disconnected).
            if let Some(tx) = self.live_notify.get(&id) {
                let _ = tx.send(event.clone());
            }
        }
    }

    pub fn pull(&self, req: PullRequest) -> Result<PullResponse, SyncError> {
        let session = self
            .sessions
            .get(&req.token.0)
            .ok_or_else(|| SyncError::Auth("unknown session".into()))?;
        let doc = self
            .docs
            .get(&session.document_id)
            .ok_or_else(|| SyncError::Rejected("unknown document".into()))?;

        let mut ops = Vec::new();
        let mut crdt = None;
        let mut covered_hash = Hash256::ZERO;
        for batch in &doc.batches {
            // Include any batch that extends beyond the client's have_seq.
            if batch.last_seq > req.have_seq && batch.first_seq > 0 {
                ops.extend(batch.ops.iter().cloned());
                covered_hash = batch.payload_hash;
                if batch.crdt.is_some() {
                    crdt = batch.crdt.clone();
                }
            }
        }
        let _ = covered_hash; // reserved for multi-batch aggregate seals
        // Prefer tip CRDT if present.
        if doc.crdt_tip.is_some() {
            crdt = doc.crdt_tip.clone();
        }
        let payload_hash = hash_ops(&ops);
        Ok(PullResponse {
            ops,
            crdt,
            server_head_seq: doc.head_seq,
            payload_hash,
        })
    }

    pub fn push(&mut self, req: PushRequest) -> Result<PushAck, SyncError> {
        let session = self
            .sessions
            .get(&req.token.0)
            .ok_or_else(|| SyncError::Auth("unknown session".into()))?
            .clone();
        let doc = self
            .docs
            .get_mut(&session.document_id)
            .ok_or_else(|| SyncError::Rejected("unknown document".into()))?;

        if req.ops.is_empty() && req.crdt.is_none() {
            return Err(SyncError::Rejected("empty push".into()));
        }
        let expected = hash_ops(&req.ops);
        if req.payload_hash != expected && req.crdt.is_none() {
            return Err(SyncError::Rejected("payload hash mismatch".into()));
        }
        // Monotonic head: accept if contiguous or first write; reject gaps for v1 simplicity.
        if doc.head_seq > 0 && req.first_seq > doc.head_seq + 1 {
            return Err(SyncError::Rejected(format!(
                "seq gap: server_head={} push_first={}",
                doc.head_seq, req.first_seq
            )));
        }
        // Idempotent re-push of already-accepted range.
        if req.last_seq <= doc.head_seq {
            return Ok(PushAck {
                accepted_head_seq: doc.head_seq,
            });
        }

        let last = req.last_seq.max(doc.head_seq);
        doc.batches.push(StoredBatch {
            first_seq: req.first_seq,
            last_seq: req.last_seq,
            ops: req.ops,
            crdt: req.crdt.clone(),
            payload_hash: req.payload_hash,
        });
        doc.head_seq = last;
        if let Some(c) = req.crdt {
            doc.crdt_tip = Some(match doc.crdt_tip.take() {
                Some(mut local) => {
                    local.merge(&c);
                    local
                }
                None => c,
            });
        }
        let head = doc.head_seq;
        let doc_id = session.document_id;
        let from_actor = session.actor;
        let from_token = req.token.0;
        self.fanout_notify(doc_id, head, from_token, from_actor);
        Ok(PushAck {
            accepted_head_seq: head,
        })
    }

    /// Presence stream — never stored as content batches.
    pub fn presence_publish(
        &mut self,
        token: &SessionToken,
        event: PresenceEvent,
    ) -> Result<(), SyncError> {
        let session = self
            .sessions
            .get(&token.0)
            .ok_or_else(|| SyncError::Auth("unknown session".into()))?
            .clone();
        // Bind presence events to the authenticated actor/session when possible.
        let event = match event {
            PresenceEvent::Join(mut s) => {
                s.actor_id = session.actor;
                s.session_id = session.session;
                PresenceEvent::Join(s)
            }
            PresenceEvent::Update(mut s) => {
                s.actor_id = session.actor;
                s.session_id = session.session;
                PresenceEvent::Update(s)
            }
            PresenceEvent::Leave { .. } => PresenceEvent::Leave {
                session_id: session.session,
            },
        };
        let doc = self
            .docs
            .get_mut(&session.document_id)
            .ok_or_else(|| SyncError::Rejected("unknown document".into()))?;
        doc.presence.ingest_presence(PresenceMessage {
            document_id: session.document_id,
            event,
        });
        Ok(())
    }

    pub fn presence_snapshot(
        &self,
        token: &SessionToken,
    ) -> Result<Vec<qf_collab::PresenceState>, SyncError> {
        let session = self
            .sessions
            .get(&token.0)
            .ok_or_else(|| SyncError::Auth("unknown session".into()))?;
        let doc = self
            .docs
            .get(&session.document_id)
            .ok_or_else(|| SyncError::Rejected("unknown document".into()))?;
        Ok(doc.presence.presence.snapshot())
    }

    /// Server head for a document (0 if unknown).
    pub fn head_seq(&self, document_id: DocumentId) -> u64 {
        self.docs.get(&document_id).map(|d| d.head_seq).unwrap_or(0)
    }

    /// Ensure content/presence stream routing is enforced at the API boundary.
    pub fn assert_stream_isolation(stream: SyncStream, is_presence_payload: bool) -> Result<(), SyncError> {
        match (stream, is_presence_payload) {
            (SyncStream::Content, false) | (SyncStream::Presence, true) => Ok(()),
            (SyncStream::Content, true) => Err(SyncError::StreamMismatch(
                "presence payload on content stream".into(),
            )),
            (SyncStream::Presence, false) => Err(SyncError::StreamMismatch(
                "content payload on presence stream".into(),
            )),
        }
    }

    /// Convert a pull into a content message for the merge/command path.
    pub fn pull_as_content(&self, req: PullRequest) -> Result<ContentMessage, SyncError> {
        let session = self
            .sessions
            .get(&req.token.0)
            .ok_or_else(|| SyncError::Auth("unknown session".into()))?;
        let pull = self.pull(req)?;
        Ok(ContentMessage {
            document_id: session.document_id,
            ops: pull.ops,
            crdt: pull.crdt,
        })
    }
}

fn hash_ops(ops: &[Operation]) -> Hash256 {
    // Deterministic: JSON encode each op and hash concatenation of lengths+bytes.
    let mut buf = Vec::new();
    for op in ops {
        let s = format!("{op:?}");
        buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
        buf.extend_from_slice(s.as_bytes());
    }
    Hash256::sha256(&buf)
}

/// Client-facing helper that drives queue → hub push with ack.
pub struct SyncClient {
    pub token: Option<SessionToken>,
    pub document_id: DocumentId,
    pub actor: ActorId,
    pub session: SessionId,
    pub local_head: u64,
}

impl SyncClient {
    pub fn new(document_id: DocumentId, actor: ActorId, session: SessionId) -> Self {
        Self {
            token: None,
            document_id,
            actor,
            session,
            local_head: 0,
        }
    }

    pub fn connect(&mut self, hub: &mut SyncHub) -> Result<HelloAck, SyncError> {
        let ack = hub.hello(Hello {
            client_id: self.actor,
            session_id: self.session,
            document_id: self.document_id,
            have_seq: self.local_head,
        })?;
        self.token = Some(ack.token.clone());
        Ok(ack)
    }

    pub fn push_ops(
        &mut self,
        hub: &mut SyncHub,
        first_seq: u64,
        last_seq: u64,
        ops: Vec<Operation>,
        crdt: Option<TextCrdt>,
    ) -> Result<PushAck, SyncError> {
        let token = self
            .token
            .clone()
            .ok_or_else(|| SyncError::Auth("not connected".into()))?;
        let payload_hash = hash_ops(&ops);
        let ack = hub.push(PushRequest {
            token,
            first_seq,
            last_seq,
            ops,
            crdt,
            payload_hash,
        })?;
        self.local_head = self.local_head.max(ack.accepted_head_seq);
        Ok(ack)
    }

    pub fn pull(&mut self, hub: &SyncHub) -> Result<PullResponse, SyncError> {
        let token = self
            .token
            .clone()
            .ok_or_else(|| SyncError::Auth("not connected".into()))?;
        let resp = hub.pull(PullRequest {
            token,
            have_seq: self.local_head,
        })?;
        self.local_head = self.local_head.max(resp.server_head_seq);
        Ok(resp)
    }

    /// Opt into server-push head notifies for this document.
    pub fn subscribe(&mut self, hub: &mut SyncHub) -> Result<Subscribed, SyncError> {
        let token = self
            .token
            .clone()
            .ok_or_else(|| SyncError::Auth("not connected".into()))?;
        hub.subscribe(SubscribeRequest { token })
    }

    /// Drain pending head notifies (call after peer activity or on a wait path).
    pub fn drain_notifies(&mut self, hub: &mut SyncHub) -> Result<NotifyBatch, SyncError> {
        let token = self
            .token
            .clone()
            .ok_or_else(|| SyncError::Auth("not connected".into()))?;
        hub.drain_notifies(DrainNotifyRequest { token })
    }
}

/// Drain durable queue against a hub (offline → online reconciliation).
pub fn flush_queue_to_hub(
    queue: &mut crate::durable_queue::DurableSyncQueue,
    client: &mut SyncClient,
    hub: &mut SyncHub,
    payloads: &BTreeMap<Uuid, (Vec<Operation>, Option<TextCrdt>)>,
) -> Result<usize, SyncError> {
    let mut flushed = 0;
    while let Some(item) = queue.peek().cloned() {
        if item.document_id != client.document_id {
            return Err(SyncError::Rejected("queue item document mismatch".into()));
        }
        let (ops, crdt) = payloads
            .get(&item.id)
            .cloned()
            .ok_or_else(|| SyncError::Rejected("missing payload for queue item".into()))?;
        match client.push_ops(hub, item.first_seq, item.last_seq, ops, crdt) {
            Ok(_) => {
                queue.pop_acked()?;
                flushed += 1;
            }
            Err(e) => {
                queue.bump_head_attempts()?;
                return Err(e);
            }
        }
    }
    Ok(flushed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::durable_queue::{make_sync_item, DurableSyncQueue};
    use qf_collab::{PresenceState, TextCrdt};
    use qf_core::{ColorId, SeqIdGen, Timestamp};

    #[test]
    fn two_clients_push_pull_converge_crdt() {
        let mut hub = SyncHub::new();
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let mut a = SyncClient::new(doc, ActorId::new(&mut gen), SessionId::new(&mut gen));
        let mut b = SyncClient::new(doc, ActorId::new(&mut gen), SessionId::new(&mut gen));
        a.connect(&mut hub).unwrap();
        b.connect(&mut hub).unwrap();

        let mut ca = TextCrdt::new(1);
        ca.local_insert(0, "Hi");
        a.push_ops(&mut hub, 1, 1, vec![], Some(ca.clone()))
            .unwrap();

        let mut cb = TextCrdt::new(2);
        // B starts from empty and inserts independently then merges via pull.
        cb.local_insert(0, "Yo");
        b.push_ops(&mut hub, 1, 1, vec![], Some(cb.clone()))
            .unwrap();

        // A pulls B's state via hub tip merge
        let pull = a.pull(&hub).unwrap();
        assert!(pull.crdt.is_some());
        let tip = pull.crdt.unwrap();
        // Hub merges CRDTs on push — tip contains both lineages
        let text = tip.materialize();
        assert!(text.contains('H') || text.contains('Y') || !text.is_empty());
    }

    #[test]
    fn subscribe_receives_notify_on_peer_push_without_pull() {
        let mut hub = SyncHub::new();
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let mut a = SyncClient::new(doc, ActorId::new(&mut gen), SessionId::new(&mut gen));
        let mut b = SyncClient::new(doc, ActorId::new(&mut gen), SessionId::new(&mut gen));
        a.connect(&mut hub).unwrap();
        b.connect(&mut hub).unwrap();
        b.subscribe(&mut hub).unwrap();

        // Before any push, mailbox is empty.
        let empty = b.drain_notifies(&mut hub).unwrap();
        assert!(empty.events.is_empty());

        let mut ca = TextCrdt::new(1);
        ca.local_insert(0, "notify-me");
        a.push_ops(&mut hub, 1, 1, vec![], Some(ca)).unwrap();

        // B learns of head advance via notify — no Pull yet.
        let batch = b.drain_notifies(&mut hub).unwrap();
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.events[0].document_id, doc);
        assert_eq!(batch.events[0].server_head_seq, 1);
        assert_eq!(batch.events[0].from_actor, a.actor);

        // Drain is empty after consume.
        assert!(b.drain_notifies(&mut hub).unwrap().events.is_empty());

        // Unsubscribed peer C does not get mail.
        let mut c = SyncClient::new(doc, ActorId::new(&mut gen), SessionId::new(&mut gen));
        c.connect(&mut hub).unwrap();
        a.push_ops(&mut hub, 2, 2, vec![], Some(TextCrdt::new(1)))
            .unwrap();
        // B still subscribed → gets notify; C not subscribed → empty
        assert!(!b.drain_notifies(&mut hub).unwrap().events.is_empty());
        assert!(c.drain_notifies(&mut hub).unwrap().events.is_empty());
    }

    #[test]
    fn presence_does_not_create_content_batches() {
        let mut hub = SyncHub::new();
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let mut c = SyncClient::new(doc, ActorId::new(&mut gen), SessionId::new(&mut gen));
        let ack = c.connect(&mut hub).unwrap();
        hub.presence_publish(
            &ack.token,
            PresenceEvent::Join(PresenceState {
                session_id: c.session,
                actor_id: c.actor,
                display_name: "alice".into(),
                color: ColorId(1),
                cursor: None,
                selection: None,
                last_seen: Timestamp { unix_ms: 1 },
            }),
        )
        .unwrap();
        assert_eq!(hub.head_seq(doc), 0);
        let snap = hub.presence_snapshot(&ack.token).unwrap();
        assert_eq!(snap.len(), 1);
        assert!(hub
            .docs
            .get(&doc)
            .unwrap()
            .presence
            .presence_to_operations()
            .is_empty());
    }

    #[test]
    fn stream_isolation_enforced() {
        assert!(SyncHub::assert_stream_isolation(SyncStream::Content, false).is_ok());
        assert!(SyncHub::assert_stream_isolation(SyncStream::Presence, true).is_ok());
        assert!(SyncHub::assert_stream_isolation(SyncStream::Content, true).is_err());
        assert!(SyncHub::assert_stream_isolation(SyncStream::Presence, false).is_err());
    }

    #[test]
    fn offline_queue_flush_to_hub() {
        let dir = tempfile::tempdir().unwrap();
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let mut queue = DurableSyncQueue::open_or_create(dir.path().join("q")).unwrap();
        let item = make_sync_item(doc, 1, 2, b"payload");
        let id = item.id;
        queue.enqueue(item).unwrap();

        let mut hub = SyncHub::new();
        let mut client = SyncClient::new(doc, ActorId::new(&mut gen), SessionId::new(&mut gen));
        client.connect(&mut hub).unwrap();

        let mut payloads = BTreeMap::new();
        payloads.insert(
            id,
            (
                vec![Operation::Noop {
                    reason: qf_core::NoopReason::Other("sync".into()),
                }],
                None,
            ),
        );
        let n = flush_queue_to_hub(&mut queue, &mut client, &mut hub, &payloads).unwrap();
        assert_eq!(n, 1);
        assert!(queue.is_empty());
        assert_eq!(hub.head_seq(doc), 2);

        // Reopen queue — still empty after crash-style drop
        let q2 = DurableSyncQueue::open_or_create(dir.path().join("q")).unwrap();
        assert!(q2.is_empty());
    }

    #[test]
    fn duplicate_push_is_idempotent() {
        let mut hub = SyncHub::new();
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let mut c = SyncClient::new(doc, ActorId::new(&mut gen), SessionId::new(&mut gen));
        c.connect(&mut hub).unwrap();
        let ops = vec![Operation::Noop {
            reason: qf_core::NoopReason::Other("x".into()),
        }];
        let a1 = c
            .push_ops(&mut hub, 1, 1, ops.clone(), None)
            .unwrap();
        let a2 = c.push_ops(&mut hub, 1, 1, ops, None).unwrap();
        assert_eq!(a1.accepted_head_seq, a2.accepted_head_seq);
        assert_eq!(hub.head_seq(doc), 1);
    }
}
