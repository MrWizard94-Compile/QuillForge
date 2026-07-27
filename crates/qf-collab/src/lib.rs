//! Collaboration: presence (ephemeral), comments/suggestions (via ops).

use qf_core::{
    ActorId, Operation, SessionId, SuggestionId, TextTarget, Timestamp,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PresenceState {
    pub session_id: SessionId,
    pub actor_id: ActorId,
    pub display_name: String,
    pub cursor: Option<TextTarget>,
    pub last_seen: Timestamp,
}

#[derive(Clone, Debug)]
pub enum PresenceEvent {
    Join(PresenceState),
    Update(PresenceState),
    Leave { session_id: SessionId },
}

#[derive(Default)]
pub struct PresenceBroker {
    peers: HashMap<SessionId, PresenceState>,
}

impl PresenceBroker {
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

    pub fn peers(&self) -> impl Iterator<Item = &PresenceState> {
        self.peers.values()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionStatus {
    Pending,
    Accepted,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionSource {
    Human,
    Ai { provider: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Suggestion {
    pub id: SuggestionId,
    pub author: ActorId,
    pub note: Option<String>,
    pub proposed_ops: Vec<Operation>,
    pub status: SuggestionStatus,
    pub source: SuggestionSource,
}

/// Presence MUST NOT be written to the journal.
pub fn presence_is_ephemeral() -> bool {
    true
}
