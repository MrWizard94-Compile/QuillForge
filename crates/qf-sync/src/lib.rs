//! Offline-first sync queue and merge ingress.
//! Remote ops enter canonical state only via command/journal/kernel.

use qf_core::{DocumentId, Hash256, Operation};
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
}

/// Merge remote operations into candidates for command path.
pub trait MergeEngine: Send + Sync {
    fn integrate(&self, remote_ops: &[Operation]) -> Result<Vec<Operation>, SyncError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SyncError {
    Rejected(String),
    Network(String),
    Auth(String),
}

/// Identity merge (foundation): remote ops already CRDT-resolved upstream.
pub struct IdentityMerge;

impl MergeEngine for IdentityMerge {
    fn integrate(&self, remote_ops: &[Operation]) -> Result<Vec<Operation>, SyncError> {
        Ok(remote_ops.to_vec())
    }
}

#[derive(Clone, Debug)]
pub enum SyncStream {
    Content,
    Presence,
}
