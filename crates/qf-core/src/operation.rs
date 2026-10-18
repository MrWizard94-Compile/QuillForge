//! Pure, serializable, replayable mutation units.

use crate::ids::*;
use crate::preserved::PreservedMap;
use serde::{Deserialize, Serialize};

/// Parent reference for structural ops.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParentRef {
    Section(SectionId),
    Block(BlockId),
    DocumentRoot,
}

/// Opaque block body payload as carried on the wire before typed decode in qf-blocks.
/// Core keeps this as bytes + type tag to avoid circular deps; qf-doc/blocks refine it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockBodyBytes {
    pub type_name: String,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MetaPatch {
    pub title: Option<String>,
    pub language: Option<String>,
    pub custom: Option<PreservedMap>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NoopReason {
    IdempotentRemote,
    TargetMissing,
    Other(String),
}

/// Canonical operation set. All new entity IDs are pre-assigned by command handlers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    InsertText {
        block_id: BlockId,
        at: TextOffset,
        text: String,
    },
    DeleteText {
        block_id: BlockId,
        range: OffsetRange,
    },
    InsertBlock {
        block_id: BlockId,
        parent: ParentRef,
        index: u32,
        body: BlockBodyBytes,
    },
    RemoveBlock {
        block_id: BlockId,
    },
    MoveBlock {
        block_id: BlockId,
        new_parent: ParentRef,
        index: u32,
    },
    SetMarks {
        span_id: SpanId,
        /// Serialized mark list (typed in qf-blocks).
        marks_payload: Vec<u8>,
    },
    UpdateBlockMeta {
        block_id: BlockId,
        /// Serialized BlockMetadata attrs patch.
        meta_payload: Vec<u8>,
    },
    ReplaceUnknownPayload {
        block_id: BlockId,
        original_type: String,
        original_payload: Vec<u8>,
    },
    UpsertComment {
        comment_id: CommentId,
        payload: Vec<u8>,
    },
    DeleteComment {
        comment_id: CommentId,
    },
    UpsertSuggestion {
        suggestion_id: SuggestionId,
        payload: Vec<u8>,
    },
    /// Remove a suggestion entity (invert of first upsert; explicit delete).
    DeleteSuggestion {
        suggestion_id: SuggestionId,
    },
    /// Apply the stored operations of an already accepted suggestion.
    ///
    /// The kernel resolves the recorded payload deterministically during replay
    /// and rejects pending, malformed, or recursively nested suggestions.
    ApplySuggestionOps {
        suggestion_id: SuggestionId,
    },
    SetDocumentMeta {
        patch: MetaPatch,
    },
    Noop {
        reason: NoopReason,
    },
}

/// Batch of operations committed together (undo unit).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpBatch {
    pub ops: Vec<Operation>,
}
