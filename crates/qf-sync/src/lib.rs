//! Offline-first sync queue, merge ingress, and network transports.
//!
//! Remote content enters canonical state only via command/journal/kernel.
//! Presence uses a separate non-durable stream and never becomes Operations.
//!
//! # Transports
//!
//! - [`SyncHub`] — in-process multi-replica logical server
//! - [`TcpSyncServer`] / [`TcpSyncClient`] — plain TCP QFSNC1 frames
//! - [`TlsSyncServer`] / [`TlsSyncClient`] — same protocol over rustls TLS 1.3
//! - [`WsSyncServer`] / [`WsSyncClient`] — QFSNC1 binary frames over WebSocket (`ws://…/qfsnc1`)
//! - [`DurableSyncQueue`] — fsync offline queue

mod durable_queue;
mod hub;
mod tcp;
mod tls;
mod wire;
mod ws;

pub use durable_queue::{make_sync_item, DurableSyncQueue};
pub use hub::{
    flush_queue_to_hub, Hello, HelloAck, PullRequest, PullResponse, PushAck, PushRequest,
    SessionToken, SyncClient, SyncHub,
};
pub use tcp::{TcpSyncClient, TcpSyncServer};
pub use tls::{TlsIdentity, TlsSyncClient, TlsSyncServer};
pub use wire::{WireRequest, WireResponse, PROTOCOL_ID};
pub use ws::{WsSyncClient, WsSyncServer};

use qf_collab::{merge_crdt, PresenceBroker, PresenceEvent, TextCrdt};
use qf_core::{BlockId, DocumentId, Hash256, Operation};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SyncItem {
    pub id: Uuid,
    pub document_id: DocumentId,
    pub first_seq: u64,
    pub last_seq: u64,
    pub payload_hash: Hash256,
    pub attempts: u32,
}

#[derive(Default)]
pub struct SyncQueue {
    pub pending: VecDeque<SyncItem>,
}

impl SyncQueue {
    pub fn enqueue(&mut self, item: SyncItem) {
        self.pending.push_back(item);
    }

    pub fn peek(&self) -> Option<&SyncItem> {
        self.pending.front()
    }

    pub fn pop_acked(&mut self) -> Option<SyncItem> {
        self.pending.pop_front()
    }

    pub fn len(&self) -> usize {
        self.pending.len()
    }

    pub fn is_empty(&self) -> bool {
        self.pending.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    Rejected(String),
    Network(String),
    Auth(String),
    StreamMismatch(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Rejected(s) => write!(f, "rejected: {s}"),
            SyncError::Network(s) => write!(f, "network: {s}"),
            SyncError::Auth(s) => write!(f, "auth: {s}"),
            SyncError::StreamMismatch(s) => write!(f, "stream mismatch: {s}"),
        }
    }
}

impl std::error::Error for SyncError {}

/// Logical stream multiplex — content vs presence must not be mixed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SyncStream {
    Content,
    Presence,
}

/// Content envelope (durable after local commit).
#[derive(Clone, Debug)]
pub struct ContentMessage {
    pub document_id: DocumentId,
    pub ops: Vec<Operation>,
    pub crdt: Option<TextCrdt>,
}

/// Presence envelope (ephemeral only).
#[derive(Clone, Debug)]
pub struct PresenceMessage {
    pub document_id: DocumentId,
    pub event: PresenceEvent,
}

/// Router that refuses to treat presence as content.
#[derive(Debug, Default)]
pub struct StreamRouter {
    pub presence: PresenceBroker,
}

impl StreamRouter {
    pub fn ingest_presence(&mut self, msg: PresenceMessage) {
        self.presence.apply(msg.event);
    }

    /// Content messages produce ops for the command path; never auto-journaled here.
    pub fn integrate_content(
        &self,
        local_crdt: Option<&TextCrdt>,
        msg: &ContentMessage,
        block_id: BlockId,
        current_text: &str,
    ) -> Result<Vec<Operation>, SyncError> {
        if let Some(remote) = &msg.crdt {
            let merged = match local_crdt {
                Some(local) => merge_crdt(local, remote),
                None => remote.clone(),
            };
            return Ok(merged.to_replace_ops(block_id, current_text));
        }
        Ok(msg.ops.clone())
    }

    /// Hard guarantee for audits.
    pub fn presence_to_operations(&self) -> Vec<Operation> {
        self.presence.to_operations()
    }
}

/// Merge remote operations into candidates for command path.
pub trait MergeEngine: Send + Sync {
    fn integrate(&self, remote_ops: &[Operation]) -> Result<Vec<Operation>, SyncError>;
}

/// Identity merge: remote ops already resolved (or single-writer).
pub struct IdentityMerge;

impl MergeEngine for IdentityMerge {
    fn integrate(&self, remote_ops: &[Operation]) -> Result<Vec<Operation>, SyncError> {
        Ok(remote_ops.to_vec())
    }
}

/// CRDT-aware merge engine for a single text block.
pub struct TextCrdtMerge {
    pub block_id: BlockId,
    pub local: TextCrdt,
    pub current_text: String,
}

impl TextCrdtMerge {
    pub fn integrate_remote_crdt(
        &mut self,
        remote: &TextCrdt,
    ) -> Result<Vec<Operation>, SyncError> {
        self.local.merge(remote);
        let ops = self.local.to_replace_ops(self.block_id, &self.current_text);
        self.current_text = self.local.materialize();
        Ok(ops)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_collab::PresenceState;
    use qf_core::{ActorId, ColorId, SeqIdGen, SessionId, Timestamp};

    #[test]
    fn presence_stream_never_becomes_ops() {
        let mut router = StreamRouter::default();
        let mut gen = SeqIdGen::default();
        router.ingest_presence(PresenceMessage {
            document_id: DocumentId::new(&mut gen),
            event: PresenceEvent::Join(PresenceState {
                session_id: SessionId::new(&mut gen),
                actor_id: ActorId::new(&mut gen),
                display_name: "x".into(),
                color: ColorId(0),
                cursor: None,
                selection: None,
                last_seen: Timestamp { unix_ms: 1 },
            }),
        });
        assert_eq!(router.presence.len(), 1);
        assert!(router.presence_to_operations().is_empty());
    }

    #[test]
    fn two_replica_crdt_content_merge_converges() {
        let mut a = TextCrdt::new(1);
        a.local_insert(0, "base");
        let mut b = a.fork_replica(2);
        a.local_insert(4, "A");
        b.local_insert(4, "B");

        let mut ma = TextCrdtMerge {
            block_id: BlockId(uuid::Uuid::from_u128(9)),
            local: a.clone(),
            current_text: a.materialize(),
        };
        ma.current_text = a.materialize();
        let ops = ma.integrate_remote_crdt(&b).unwrap();
        let final_text = ma.local.materialize();

        let mut mb = TextCrdtMerge {
            block_id: BlockId(uuid::Uuid::from_u128(9)),
            local: b.clone(),
            current_text: b.materialize(),
        };
        mb.integrate_remote_crdt(&a).unwrap();
        assert_eq!(mb.local.materialize(), final_text);
        assert!(!ops.is_empty() || final_text == "baseA" || final_text == "baseB");
        assert_eq!(final_text.chars().count(), 6);
    }
}
