//! Ephemeral presence — never journaled.

use qf_core::{ActorId, ColorId, DocRange, SessionId, TextTarget, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresenceState {
    pub session_id: SessionId,
    pub actor_id: ActorId,
    pub display_name: String,
    pub color: ColorId,
    pub cursor: Option<TextTarget>,
    pub selection: Option<DocRange>,
    pub last_seen: Timestamp,
}

#[derive(Clone, Debug)]
pub enum PresenceEvent {
    Join(PresenceState),
    Update(PresenceState),
    Leave { session_id: SessionId },
}

/// In-memory presence broker. Not durable. Not a source of Operations.
#[derive(Debug, Default)]
pub struct PresenceBroker {
    peers: HashMap<SessionId, PresenceState>,
}

impl PresenceBroker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply(&mut self, ev: PresenceEvent) {
        match ev {
            PresenceEvent::Join(s) | PresenceEvent::Update(s) => {
                self.peers.insert(s.session_id, s);
            }
            PresenceEvent::Leave { session_id } => {
                self.peers.remove(&session_id);
            }
        }
    }

    pub fn get(&self, session_id: SessionId) -> Option<&PresenceState> {
        self.peers.get(&session_id)
    }

    pub fn peers(&self) -> impl Iterator<Item = &PresenceState> {
        self.peers.values()
    }

    /// Snapshot of all peer presence states (ephemeral; not durable).
    pub fn snapshot(&self) -> Vec<PresenceState> {
        self.peers.values().cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.peers.len()
    }

    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    /// Explicit API: presence never yields document operations.
    pub fn to_operations(&self) -> Vec<qf_core::Operation> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::SeqIdGen;

    #[test]
    fn presence_does_not_emit_ops() {
        let mut gen = SeqIdGen::default();
        let mut broker = PresenceBroker::new();
        broker.apply(PresenceEvent::Join(PresenceState {
            session_id: SessionId::new(&mut gen),
            actor_id: ActorId::new(&mut gen),
            display_name: "A".into(),
            color: ColorId(1),
            cursor: None,
            selection: None,
            last_seen: Timestamp { unix_ms: 0 },
        }));
        assert_eq!(broker.len(), 1);
        assert!(broker.to_operations().is_empty());
        // Presence durability policy is a named constant for auditors.
        const _: bool = crate::PRESENCE_STREAM_DURABLE;
        let _ = crate::presence_is_ephemeral();
    }
}
