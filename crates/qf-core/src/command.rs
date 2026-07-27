//! User/system intent that expands to zero or more operations.

use crate::ids::*;
use crate::operation::{MetaPatch, Operation, ParentRef};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActorContext {
    pub actor_id: ActorId,
    pub session_id: SessionId,
    pub roles: Vec<Role>,
    pub capabilities: Vec<Capability>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Role {
    Owner,
    Editor,
    Commenter,
    Viewer,
    Reviewer,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextTarget {
    pub block_id: BlockId,
    pub span_id: Option<SpanId>,
    pub offset: TextOffset,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocRange {
    pub start: TextTarget,
    pub end: TextTarget,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientMeta {
    /// Non-authoritative UI selection snapshot for diagnostics.
    pub note: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandEnvelope {
    pub command_id: CommandId,
    pub document_id: DocumentId,
    pub actor: ActorContext,
    pub issued_at_unix_ms: i64,
    pub client_meta: ClientMeta,
    pub command: Command,
}

/// Intent enum. Only operations are journaled for replay — not commands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Command {
    InsertText {
        target: TextTarget,
        text: String,
    },
    DeleteRange {
        range: DocRange,
    },
    ReplaceRange {
        range: DocRange,
        text: String,
    },
    InsertBlock {
        parent: ParentRef,
        index: u32,
        body_type: String,
        body_payload: Vec<u8>,
    },
    DeleteBlock {
        block_id: BlockId,
    },
    MoveBlock {
        block_id: BlockId,
        new_parent: ParentRef,
        index: u32,
    },
    SplitBlock {
        block_id: BlockId,
        offset: TextOffset,
    },
    MergeBlocks {
        left: BlockId,
        right: BlockId,
    },
    ApplyMark {
        range: DocRange,
        mark_payload: Vec<u8>,
    },
    RemoveMark {
        range: DocRange,
        mark_kind: String,
    },
    AddComment {
        anchor_payload: Vec<u8>,
        body: String,
    },
    ResolveComment {
        comment_id: CommentId,
    },
    CreateSuggestion {
        ops: Vec<Operation>,
        note: Option<String>,
    },
    AcceptSuggestion {
        suggestion_id: SuggestionId,
    },
    RejectSuggestion {
        suggestion_id: SuggestionId,
    },
    Undo {
        steps: u32,
    },
    Redo {
        steps: u32,
    },
    SetDocumentMeta {
        patch: MetaPatch,
    },
    PluginCommand {
        plugin_id: PluginId,
        name: String,
        payload: Vec<u8>,
    },
    AcceptAiSuggestion {
        suggestion_id: SuggestionId,
    },
    ApplyRemoteBatch {
        remote_actor: ActorId,
        ops: Vec<Operation>,
    },
}
