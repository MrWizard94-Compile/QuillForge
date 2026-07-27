//! Collaboration: ephemeral presence, CRDT text, suggestions.
//!
//! # Stream separation (binding)
//!
//! | Stream     | Durable? | Path                          |
//! |------------|----------|-------------------------------|
//! | Content    | Yes      | Command → Journal → Kernel    |
//! | Presence   | No       | PresenceBroker only           |
//! | Suggestions| Yes      | Ops via command path          |
//!
//! Presence MUST NOT be written to the journal.

mod presence;
mod text_crdt;

pub use presence::*;
pub use text_crdt::*;

use qf_core::{ActorId, Operation, SuggestionId};
use serde::{Deserialize, Serialize};

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

/// Compile-time documentation of stream policy for audits.
pub const CONTENT_STREAM_DURABLE: bool = true;
pub const PRESENCE_STREAM_DURABLE: bool = false;
