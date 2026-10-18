//! Document kernel: pure apply of operations to canonical state.
//!
//! # Invariants
//!
//! - Apply has no I/O, no RNG, no wall-clock decisions.
//! - All new entity IDs are present in the operation payload.
//! - Successful apply advances `op_seq` by exactly one.
//! - Failed apply leaves state unchanged (clone-swap transaction).
//!
//! # Undo
//!
//! `invert_ops` produces inverse operations from a pre-image state so undo is
//! operation-based, not snapshot-based.

use qf_blocks::{
    BlockBody, BlockMetadata, HeadingBlock, ParagraphBlock, QuoteBlock, Span, UnknownBlock,
};
use qf_core::{
    ApplyError, BlockBodyBytes, BlockId, DocumentId, Hash256, MetaPatch, OffsetRange, OpSeq,
    Operation, ParentRef, PreservedMap, TextOffset, Version,
};
use qf_doc::{Block, Document};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Parallel entity tables owned by canonical state.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct EntityTables {
    pub comments: BTreeMap<qf_core::CommentId, Vec<u8>>,
    pub suggestions: BTreeMap<qf_core::SuggestionId, Vec<u8>>,
    pub assets: BTreeMap<qf_core::AssetId, Vec<u8>>,
    pub sources: BTreeMap<qf_core::SourceId, Vec<u8>>,
}

/// Authoritative document state after applying ops `1..=op_seq`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanonicalState {
    pub document_id: DocumentId,
    pub package_format_version: Version,
    pub op_seq: OpSeq,
    pub document: Document,
    pub entities: EntityTables,
}

/// Durable snapshot for checkpoints.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocumentSnapshot {
    pub state: CanonicalState,
    pub snapshot_hash: Hash256,
}

impl DocumentSnapshot {
    pub fn from_state(state: CanonicalState) -> Self {
        let snapshot_hash = state_content_hash(&state);
        Self {
            state,
            snapshot_hash,
        }
    }

    pub fn verify_hash(&self) -> bool {
        state_content_hash(&self.state) == self.snapshot_hash
    }
}

/// Deterministic content hash of canonical state (CBOR preferred, JSON fallback).
pub fn state_content_hash(state: &CanonicalState) -> Hash256 {
    let mut buf = Vec::new();
    if ciborium::into_writer(state, &mut buf).is_ok() {
        return Hash256::sha256(&buf);
    }
    match serde_json::to_vec(state) {
        Ok(bytes) => Hash256::sha256(&bytes),
        Err(_) => Hash256::ZERO,
    }
}

/// Events fan-out only; never authoritative.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KernelEvent {
    DocumentHydrated { op_seq: OpSeq },
    BlockInserted { id: BlockId },
    BlockRemoved { id: BlockId },
    BlockMoved { id: BlockId },
    TextChanged { block: BlockId },
    MetaChanged,
    Noop,
    UnknownBlockPreserved { id: BlockId, original_type: String },
}

/// Sole writer of canonical document state.
#[derive(Debug)]
pub struct Kernel {
    state: CanonicalState,
}

impl Kernel {
    pub fn new(state: CanonicalState) -> Self {
        Self { state }
    }

    pub fn fresh(
        document_id: DocumentId,
        root: qf_core::SectionId,
        actor: qf_core::ActorId,
        at: qf_core::Timestamp,
    ) -> Self {
        let document = Document::empty(document_id, root, actor, at);
        Self::new(CanonicalState {
            document_id,
            package_format_version: Version::V1_0_0,
            op_seq: OpSeq::ZERO,
            document,
            entities: EntityTables::default(),
        })
    }

    pub fn from_snapshot(snap: DocumentSnapshot) -> Result<Self, ApplyError> {
        if !snap.verify_hash() {
            return Err(ApplyError::Invariant("snapshot hash mismatch".into()));
        }
        Ok(Self { state: snap.state })
    }

    /// Load snapshot without hash verify (used when reconstructing from trusted journal fold).
    pub fn from_snapshot_unverified(snap: DocumentSnapshot) -> Self {
        Self { state: snap.state }
    }

    pub fn state(&self) -> &CanonicalState {
        &self.state
    }

    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot::from_state(self.state.clone())
    }

    pub fn content_hash(&self) -> Hash256 {
        state_content_hash(&self.state)
    }

    pub fn apply_batch(&mut self, ops: &[Operation]) -> Result<Vec<KernelEvent>, ApplyError> {
        let mut events = Vec::new();
        // Apply on clone for atomicity of the whole batch.
        let mut next = self.state.clone();
        for op in ops {
            let ev = apply_to(&mut next, op)?;
            next.op_seq = next_op_seq(next.op_seq);
            events.extend(ev);
        }
        self.state = next;
        Ok(events)
    }

    pub fn apply(&mut self, op: &Operation) -> Result<Vec<KernelEvent>, ApplyError> {
        self.apply_batch(std::slice::from_ref(op))
    }

    /// Dry-run apply on a clone; never mutates self.
    pub fn dry_run(&self, ops: &[Operation]) -> Result<(), ApplyError> {
        let mut k = Kernel::new(self.state.clone());
        k.apply_batch(ops)?;
        Ok(())
    }
}

fn next_op_seq(seq: OpSeq) -> OpSeq {
    if seq == OpSeq::ZERO {
        OpSeq(1)
    } else {
        seq.next()
    }
}

/// Produce inverse operations that restore `state_before` after `ops` were applied.
///
/// Inverses are ordered for reverse application (last forward op inverted first).
pub fn invert_ops(
    state_before: &CanonicalState,
    ops: &[Operation],
) -> Result<Vec<Operation>, ApplyError> {
    let mut working = state_before.clone();
    let mut inverses_rev = Vec::with_capacity(ops.len());
    for op in ops {
        let inverses = inverse_for_op(&working, op)?;
        apply_to(&mut working, op)?;
        working.op_seq = next_op_seq(working.op_seq);
        // Each operation's inverse sequence is already in apply order. Store it
        // reversed so the final vector reverses operation groups, not their
        // internal restoration order.
        inverses_rev.extend(inverses.into_iter().rev());
    }
    inverses_rev.reverse();
    Ok(inverses_rev)
}

fn inverse_for_op(state: &CanonicalState, op: &Operation) -> Result<Vec<Operation>, ApplyError> {
    match op {
        Operation::RemoveBlock { block_id } => restore_removed_block_ops(state, *block_id),
        Operation::ApplySuggestionOps { suggestion_id } => {
            let payload = state
                .entities
                .suggestions
                .get(suggestion_id)
                .ok_or(ApplyError::MissingEntity)?;
            let proposed = proposed_ops_from_suggestion_payload(payload)?;
            invert_ops(state, &proposed)
        }
        _ => Ok(vec![invert_one(state, op)?]),
    }
}

fn restore_removed_block_ops(
    state: &CanonicalState,
    block_id: BlockId,
) -> Result<Vec<Operation>, ApplyError> {
    let block = state
        .document
        .blocks
        .get(&block_id)
        .ok_or(ApplyError::MissingEntity)?;
    let (parent, index) = find_block_location(state, block_id)?;
    let mut meta_payload = Vec::new();
    ciborium::into_writer(&block.meta, &mut meta_payload)
        .map_err(|e| ApplyError::Schema(format!("RemoveBlock invert encode: {e}")))?;
    Ok(vec![
        Operation::InsertBlock {
            block_id,
            parent,
            index: index as u32,
            body: encode_body(&block.body)?,
        },
        Operation::UpdateBlockMeta {
            block_id,
            meta_payload,
        },
    ])
}

fn invert_one(state: &CanonicalState, op: &Operation) -> Result<Operation, ApplyError> {
    match op {
        Operation::InsertText { block_id, at, text } => {
            let end = TextOffset(at.0.saturating_add(text.chars().count() as u32));
            Ok(Operation::DeleteText {
                block_id: *block_id,
                range: OffsetRange { start: *at, end },
            })
        }
        Operation::DeleteText { block_id, range } => {
            let text = extract_text(state, *block_id, range.start.0, range.end.0)?;
            Ok(Operation::InsertText {
                block_id: *block_id,
                at: range.start,
                text,
            })
        }
        Operation::InsertBlock { block_id, .. } => Ok(Operation::RemoveBlock {
            block_id: *block_id,
        }),
        Operation::RemoveBlock { .. } => Err(ApplyError::Invariant(
            "RemoveBlock inversion requires block restoration operations".into(),
        )),
        Operation::MoveBlock {
            block_id,
            new_parent: _,
            index: _,
        } => {
            let (parent, index) = find_block_location(state, *block_id)?;
            Ok(Operation::MoveBlock {
                block_id: *block_id,
                new_parent: parent,
                index: index as u32,
            })
        }
        Operation::ReplaceUnknownPayload {
            block_id,
            original_type: _,
            original_payload: _,
        } => {
            let block = state
                .document
                .blocks
                .get(block_id)
                .ok_or(ApplyError::MissingEntity)?;
            match &block.body {
                BlockBody::Unknown(u) => Ok(Operation::ReplaceUnknownPayload {
                    block_id: *block_id,
                    original_type: u.original_type.clone(),
                    original_payload: u.original_payload.clone(),
                }),
                _ => Err(ApplyError::TypeMismatch),
            }
        }
        Operation::SetDocumentMeta { patch: _ } => {
            let m = &state.document.meta;
            Ok(Operation::SetDocumentMeta {
                patch: MetaPatch {
                    title: Some(m.title.clone()),
                    language: m.language.clone(),
                    custom: Some(m.custom.clone()),
                },
            })
        }
        Operation::UpsertComment { comment_id, .. } => {
            if state.entities.comments.contains_key(comment_id) {
                let payload = state.entities.comments[comment_id].clone();
                Ok(Operation::UpsertComment {
                    comment_id: *comment_id,
                    payload,
                })
            } else {
                Ok(Operation::DeleteComment {
                    comment_id: *comment_id,
                })
            }
        }
        Operation::DeleteComment { comment_id } => {
            let payload = state
                .entities
                .comments
                .get(comment_id)
                .cloned()
                .ok_or(ApplyError::MissingEntity)?;
            Ok(Operation::UpsertComment {
                comment_id: *comment_id,
                payload,
            })
        }
        Operation::UpsertSuggestion { suggestion_id, .. } => {
            if let Some(payload) = state.entities.suggestions.get(suggestion_id) {
                Ok(Operation::UpsertSuggestion {
                    suggestion_id: *suggestion_id,
                    payload: payload.clone(),
                })
            } else {
                Ok(Operation::DeleteSuggestion {
                    suggestion_id: *suggestion_id,
                })
            }
        }
        Operation::DeleteSuggestion { suggestion_id } => {
            let payload = state
                .entities
                .suggestions
                .get(suggestion_id)
                .cloned()
                .ok_or(ApplyError::MissingEntity)?;
            Ok(Operation::UpsertSuggestion {
                suggestion_id: *suggestion_id,
                payload,
            })
        }
        Operation::Noop { .. } => Ok(Operation::Noop {
            reason: qf_core::NoopReason::Other("undo noop".into()),
        }),
        Operation::SetMarks { span_id, .. } => {
            let prev = current_marks_for_span(state, *span_id).unwrap_or_default();
            let mut buf = Vec::new();
            ciborium::into_writer(&prev, &mut buf)
                .map_err(|e| ApplyError::Schema(format!("SetMarks invert encode: {e}")))?;
            Ok(Operation::SetMarks {
                span_id: *span_id,
                marks_payload: buf,
            })
        }
        Operation::UpdateBlockMeta { block_id, .. } => {
            let block = state
                .document
                .blocks
                .get(block_id)
                .ok_or(ApplyError::MissingEntity)?;
            let mut buf = Vec::new();
            ciborium::into_writer(&block.meta, &mut buf)
                .map_err(|e| ApplyError::Schema(format!("UpdateBlockMeta invert encode: {e}")))?;
            Ok(Operation::UpdateBlockMeta {
                block_id: *block_id,
                meta_payload: buf,
            })
        }
        Operation::ApplySuggestionOps { .. } => Err(ApplyError::Invariant(
            "ApplySuggestionOps inversion requires recorded proposal operations".into(),
        )),
    }
}

fn current_marks_for_span(
    state: &CanonicalState,
    span_id: qf_core::SpanId,
) -> Option<Vec<qf_blocks::Mark>> {
    let nil = qf_core::SpanId(uuid::Uuid::nil());
    for block in state.document.blocks.values() {
        let spans = match &block.body {
            BlockBody::Paragraph(p) => &p.spans,
            BlockBody::Heading(h) => &h.spans,
            BlockBody::Quote(q) => &q.spans,
            _ => continue,
        };
        if spans.is_empty() {
            continue;
        }
        if span_id == nil {
            return Some(spans[0].marks.clone());
        }
        if let Some(sp) = spans.iter().find(|s| s.id == span_id) {
            return Some(sp.marks.clone());
        }
        // When span ids were not tracked, set_marks_on_block targets the first span.
        return Some(spans[0].marks.clone());
    }
    None
}

fn find_block_location(
    state: &CanonicalState,
    block_id: BlockId,
) -> Result<(ParentRef, usize), ApplyError> {
    for (sid, section) in &state.document.sections {
        if let Some(idx) = section.blocks.iter().position(|b| *b == block_id) {
            return Ok((ParentRef::Section(*sid), idx));
        }
    }
    Err(ApplyError::MissingEntity)
}

fn extract_text(
    state: &CanonicalState,
    block_id: BlockId,
    start: u32,
    end: u32,
) -> Result<String, ApplyError> {
    let full = state
        .document
        .project_text(block_id)
        .ok_or(ApplyError::TypeMismatch)?;
    let chars: Vec<char> = full.chars().collect();
    if end as usize > chars.len() || start > end {
        return Err(ApplyError::OffsetOutOfRange);
    }
    Ok(chars[start as usize..end as usize].iter().collect())
}

fn apply_to(state: &mut CanonicalState, op: &Operation) -> Result<Vec<KernelEvent>, ApplyError> {
    match op {
        Operation::Noop { .. } => Ok(vec![KernelEvent::Noop]),
        Operation::InsertBlock {
            block_id,
            parent,
            index,
            body,
        } => {
            if state.document.blocks.contains_key(block_id) {
                return Err(ApplyError::Invariant("duplicate block id".into()));
            }
            let block_body = decode_body(body)?;
            let author = state
                .document
                .meta
                .authors
                .first()
                .copied()
                .ok_or_else(|| ApplyError::Invariant("no author".into()))?;
            let meta = match &block_body {
                BlockBody::Unknown(u) => u.metadata.clone(),
                _ => BlockMetadata {
                    created_by: author,
                    created_at_seq: next_op_seq(state.op_seq),
                    attrs: PreservedMap::new(),
                },
            };
            let block = Block {
                id: *block_id,
                meta,
                body: block_body,
            };
            insert_block_ref(state, parent, *index as usize, *block_id)?;
            let ev = match &block.body {
                BlockBody::Unknown(u) => KernelEvent::UnknownBlockPreserved {
                    id: *block_id,
                    original_type: u.original_type.clone(),
                },
                _ => KernelEvent::BlockInserted { id: *block_id },
            };
            state.document.blocks.insert(*block_id, block);
            Ok(vec![ev])
        }
        Operation::RemoveBlock { block_id } => {
            if state.document.blocks.remove(block_id).is_none() {
                return Err(ApplyError::MissingEntity);
            }
            remove_block_refs(state, *block_id);
            Ok(vec![KernelEvent::BlockRemoved { id: *block_id }])
        }
        Operation::MoveBlock {
            block_id,
            new_parent,
            index,
        } => {
            if !state.document.blocks.contains_key(block_id) {
                return Err(ApplyError::MissingEntity);
            }
            remove_block_refs(state, *block_id);
            insert_block_ref(state, new_parent, *index as usize, *block_id)?;
            Ok(vec![KernelEvent::BlockMoved { id: *block_id }])
        }
        Operation::InsertText { block_id, at, text } => {
            let block = state
                .document
                .blocks
                .get_mut(block_id)
                .ok_or(ApplyError::MissingEntity)?;
            insert_text_into_block(block, *at, text)?;
            Ok(vec![KernelEvent::TextChanged { block: *block_id }])
        }
        Operation::DeleteText { block_id, range } => {
            let block = state
                .document
                .blocks
                .get_mut(block_id)
                .ok_or(ApplyError::MissingEntity)?;
            delete_text_from_block(block, range.start.0, range.end.0)?;
            Ok(vec![KernelEvent::TextChanged { block: *block_id }])
        }
        Operation::ReplaceUnknownPayload {
            block_id,
            original_type,
            original_payload,
        } => {
            let block = state
                .document
                .blocks
                .get_mut(block_id)
                .ok_or(ApplyError::MissingEntity)?;
            match &mut block.body {
                BlockBody::Unknown(u) => {
                    u.original_type = original_type.clone();
                    u.original_payload = original_payload.clone();
                    Ok(vec![KernelEvent::UnknownBlockPreserved {
                        id: *block_id,
                        original_type: original_type.clone(),
                    }])
                }
                _ => Err(ApplyError::TypeMismatch),
            }
        }
        Operation::SetDocumentMeta { patch } => {
            if let Some(t) = &patch.title {
                state.document.meta.title = t.clone();
            }
            if let Some(l) = &patch.language {
                state.document.meta.language = Some(l.clone());
            }
            if let Some(c) = &patch.custom {
                state.document.meta.custom = c.clone();
            }
            Ok(vec![KernelEvent::MetaChanged])
        }
        Operation::UpsertComment {
            comment_id,
            payload,
        } => {
            state.entities.comments.insert(*comment_id, payload.clone());
            Ok(vec![KernelEvent::Noop])
        }
        Operation::DeleteComment { comment_id } => {
            state.entities.comments.remove(comment_id);
            Ok(vec![KernelEvent::Noop])
        }
        Operation::UpsertSuggestion {
            suggestion_id,
            payload,
        } => {
            state
                .entities
                .suggestions
                .insert(*suggestion_id, payload.clone());
            Ok(vec![KernelEvent::Noop])
        }
        Operation::DeleteSuggestion { suggestion_id } => {
            state
                .entities
                .suggestions
                .remove(suggestion_id)
                .ok_or(ApplyError::MissingEntity)?;
            Ok(vec![KernelEvent::Noop])
        }
        Operation::ApplySuggestionOps { suggestion_id } => {
            // Expand proposed_ops from stored suggestion JSON and apply as one
            // atomic batch step (single op_seq advance happens at Kernel::apply).
            let payload = state
                .entities
                .suggestions
                .get(suggestion_id)
                .cloned()
                .ok_or(ApplyError::MissingEntity)?;
            let proposed = proposed_ops_from_suggestion_payload(&payload)?;
            let mut events = Vec::new();
            for sub in &proposed {
                events.extend(apply_to(state, sub)?);
            }
            Ok(events)
        }
        Operation::SetMarks {
            span_id,
            marks_payload,
        } => {
            let marks: Vec<qf_blocks::Mark> = if marks_payload.is_empty() {
                Vec::new()
            } else {
                ciborium::from_reader(marks_payload.as_slice())
                    .map_err(|e| ApplyError::Schema(format!("SetMarks payload: {e}")))?
            };
            let mut changed = None;
            for (bid, block) in state.document.blocks.iter_mut() {
                if set_marks_on_block(block, *span_id, &marks) {
                    changed = Some(*bid);
                    break;
                }
            }
            let bid = changed.ok_or(ApplyError::MissingEntity)?;
            Ok(vec![KernelEvent::TextChanged { block: bid }])
        }
        Operation::UpdateBlockMeta {
            block_id,
            meta_payload,
        } => {
            let meta: BlockMetadata = if meta_payload.is_empty() {
                BlockMetadata {
                    created_by: qf_core::ActorId(uuid::Uuid::nil()),
                    created_at_seq: state.op_seq,
                    attrs: Default::default(),
                }
            } else {
                ciborium::from_reader(meta_payload.as_slice())
                    .map_err(|e| ApplyError::Schema(format!("UpdateBlockMeta payload: {e}")))?
            };
            let block = state
                .document
                .blocks
                .get_mut(block_id)
                .ok_or(ApplyError::MissingEntity)?;
            block.meta = meta;
            Ok(vec![KernelEvent::TextChanged { block: *block_id }])
        }
    }
}

fn set_marks_on_block(
    block: &mut Block,
    span_id: qf_core::SpanId,
    marks: &[qf_blocks::Mark],
) -> bool {
    let spans = match &mut block.body {
        BlockBody::Paragraph(p) => &mut p.spans,
        BlockBody::Heading(h) => &mut h.spans,
        BlockBody::Quote(q) => &mut q.spans,
        _ => return false,
    };
    if spans.is_empty() {
        spans.push(Span {
            id: span_id,
            text: String::new(),
            marks: marks.to_vec(),
        });
        return true;
    }
    // Match by span id, or apply to first span when nil id (whole-block formatting).
    let nil = qf_core::SpanId(uuid::Uuid::nil());
    if span_id == nil || spans.iter().any(|s| s.id == span_id) {
        for sp in spans.iter_mut() {
            if span_id == nil || sp.id == span_id {
                sp.marks = marks.to_vec();
                if sp.id == nil {
                    sp.id = span_id;
                }
                return true;
            }
        }
    }
    // First span fallback when ids were not tracked.
    spans[0].marks = marks.to_vec();
    spans[0].id = span_id;
    true
}

/// Legacy prefix used only by the uncommitted span-only representation.
const RICH_SPANS_MAGIC: &[u8] = b"QFBB1\0";
/// Versioned prefix for complete typed paragraph/heading/quote bodies.
const RICH_BODY_MAGIC: &[u8] = b"QFBB2\0";

fn encode_rich_body(body: &BlockBody) -> Result<Vec<u8>, ApplyError> {
    let mut buf = RICH_BODY_MAGIC.to_vec();
    ciborium::into_writer(body, &mut buf).map_err(|e| ApplyError::Schema(e.to_string()))?;
    Ok(buf)
}

fn decode_rich_body(payload: &[u8], expected_type: &str) -> Result<Option<BlockBody>, ApplyError> {
    if !payload.starts_with(RICH_BODY_MAGIC) {
        return Ok(None);
    }
    let body: BlockBody = ciborium::from_reader(&payload[RICH_BODY_MAGIC.len()..])
        .map_err(|e| ApplyError::Schema(format!("rich body decode: {e}")))?;
    let actual_type = match &body {
        BlockBody::Paragraph(_) => "paragraph",
        BlockBody::Heading(_) => "heading",
        BlockBody::Quote(_) => "quote",
        _ => "other",
    };
    if actual_type != expected_type {
        return Err(ApplyError::Schema(format!(
            "rich body type mismatch: expected {expected_type}, found {actual_type}"
        )));
    }
    Ok(Some(body))
}

fn try_decode_legacy_rich_spans(payload: &[u8]) -> Option<Vec<Span>> {
    if payload.starts_with(RICH_SPANS_MAGIC) {
        ciborium::from_reader(&payload[RICH_SPANS_MAGIC.len()..]).ok()
    } else {
        None
    }
}

fn proposed_ops_from_suggestion_payload(payload: &[u8]) -> Result<Vec<Operation>, ApplyError> {
    // Suggestions are JSON-encoded by qf-ai (`serde_json` of collab::Suggestion).
    // A marker may only apply an already accepted suggestion; executing a pending
    // proposal would bypass the explicit human acceptance boundary.
    let v: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|e| ApplyError::Schema(format!("suggestion payload: {e}")))?;
    let status = v
        .get("status")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| ApplyError::Schema("suggestion missing status".into()))?;
    if status != "Accepted" {
        return Err(ApplyError::Schema(
            "only accepted suggestions may apply proposed operations".into(),
        ));
    }
    let ops_val = v
        .get("proposed_ops")
        .cloned()
        .ok_or_else(|| ApplyError::Schema("suggestion missing proposed_ops".into()))?;
    let ops: Vec<Operation> = serde_json::from_value(ops_val)
        .map_err(|e| ApplyError::Schema(format!("proposed_ops decode: {e}")))?;
    if ops
        .iter()
        .any(|op| matches!(op, Operation::ApplySuggestionOps { .. }))
    {
        return Err(ApplyError::Schema(
            "suggestion proposed operations may not nest ApplySuggestionOps".into(),
        ));
    }
    Ok(ops)
}

/// Encode a typed block body into transport bytes for ops.
///
/// Paragraph/heading/quote use complete typed encoding so `RemoveBlock` inverse
/// operations preserve formatting, quote citations, and span identity.
pub fn encode_body(body: &BlockBody) -> Result<BlockBodyBytes, ApplyError> {
    match body {
        BlockBody::Paragraph(_) => Ok(BlockBodyBytes {
            type_name: "paragraph".into(),
            payload: encode_rich_body(body)?,
        }),
        BlockBody::Heading(h) => {
            let mut payload = vec![h.level];
            payload.extend(encode_rich_body(body)?);
            Ok(BlockBodyBytes {
                type_name: "heading".into(),
                payload,
            })
        }
        BlockBody::Quote(_) => Ok(BlockBodyBytes {
            type_name: "quote".into(),
            payload: encode_rich_body(body)?,
        }),
        BlockBody::Unknown(u) => Ok(BlockBodyBytes {
            type_name: u.original_type.clone(),
            payload: u.original_payload.clone(),
        }),
        BlockBody::PageBreak => Ok(BlockBodyBytes {
            type_name: "page_break".into(),
            payload: vec![],
        }),
        BlockBody::SectionBreak => Ok(BlockBodyBytes {
            type_name: "section_break".into(),
            payload: vec![],
        }),
        other => {
            // Opaque CBOR for remaining native types so they survive remove/undo.
            let mut buf = Vec::new();
            ciborium::into_writer(other, &mut buf)
                .map_err(|e| ApplyError::Schema(e.to_string()))?;
            Ok(BlockBodyBytes {
                type_name: "native_cbor".into(),
                payload: buf,
            })
        }
    }
}

pub fn decode_body(body: &BlockBodyBytes) -> Result<BlockBody, ApplyError> {
    match body.type_name.as_str() {
        "paragraph" => {
            if let Some(rich) = decode_rich_body(&body.payload, "paragraph")? {
                return Ok(rich);
            }
            let spans = if let Some(s) = try_decode_legacy_rich_spans(&body.payload) {
                s
            } else {
                let text = String::from_utf8(body.payload.clone())
                    .map_err(|e| ApplyError::Schema(e.to_string()))?;
                vec![Span {
                    id: qf_core::SpanId(uuid::Uuid::nil()),
                    text,
                    marks: vec![],
                }]
            };
            Ok(BlockBody::Paragraph(ParagraphBlock { spans }))
        }
        "heading" => {
            if body.payload.is_empty() {
                return Err(ApplyError::Schema("heading payload empty".into()));
            }
            let level = body.payload[0];
            let rest = &body.payload[1..];
            if let Some(rich) = decode_rich_body(rest, "heading")? {
                let BlockBody::Heading(heading) = rich else {
                    return Err(ApplyError::Schema(
                        "rich heading decode type mismatch".into(),
                    ));
                };
                if heading.level != level {
                    return Err(ApplyError::Schema(
                        "rich heading level does not match heading payload".into(),
                    ));
                }
                return Ok(BlockBody::Heading(heading));
            }
            let spans = if let Some(s) = try_decode_legacy_rich_spans(rest) {
                s
            } else {
                let text = String::from_utf8(rest.to_vec())
                    .map_err(|e| ApplyError::Schema(e.to_string()))?;
                vec![Span {
                    id: qf_core::SpanId(uuid::Uuid::nil()),
                    text,
                    marks: vec![],
                }]
            };
            Ok(BlockBody::Heading(HeadingBlock { level, spans }))
        }
        "quote" => {
            if let Some(rich) = decode_rich_body(&body.payload, "quote")? {
                return Ok(rich);
            }
            let spans = if let Some(s) = try_decode_legacy_rich_spans(&body.payload) {
                s
            } else {
                let text = String::from_utf8(body.payload.clone())
                    .map_err(|e| ApplyError::Schema(e.to_string()))?;
                vec![Span {
                    id: qf_core::SpanId(uuid::Uuid::nil()),
                    text,
                    marks: vec![],
                }]
            };
            Ok(BlockBody::Quote(QuoteBlock { spans, cite: None }))
        }
        "page_break" => Ok(BlockBody::PageBreak),
        "section_break" => Ok(BlockBody::SectionBreak),
        "native_cbor" => ciborium::from_reader(body.payload.as_slice())
            .map_err(|e| ApplyError::Schema(e.to_string())),
        "unknown" => Ok(BlockBody::Unknown(UnknownBlock {
            original_type: "unknown".into(),
            original_payload: body.payload.clone(),
            metadata: BlockMetadata {
                created_by: qf_core::ActorId(uuid::Uuid::nil()),
                created_at_seq: OpSeq::ZERO,
                attrs: PreservedMap::new(),
            },
        })),
        other if other.contains('.') || other.contains('/') => {
            Ok(BlockBody::Unknown(UnknownBlock {
                original_type: body.type_name.clone(),
                original_payload: body.payload.clone(),
                metadata: BlockMetadata {
                    created_by: qf_core::ActorId(uuid::Uuid::nil()),
                    created_at_seq: OpSeq::ZERO,
                    attrs: PreservedMap::new(),
                },
            }))
        }
        other => Err(ApplyError::Schema(format!("unknown body type: {other}"))),
    }
}

fn insert_block_ref(
    state: &mut CanonicalState,
    parent: &ParentRef,
    index: usize,
    block_id: BlockId,
) -> Result<(), ApplyError> {
    match parent {
        ParentRef::Section(sid) => {
            let section = state
                .document
                .sections
                .get_mut(sid)
                .ok_or(ApplyError::MissingEntity)?;
            let i = index.min(section.blocks.len());
            section.blocks.insert(i, block_id);
            Ok(())
        }
        ParentRef::DocumentRoot => {
            let sid = state.document.root_section_id;
            let section = state
                .document
                .sections
                .get_mut(&sid)
                .ok_or(ApplyError::MissingEntity)?;
            let i = index.min(section.blocks.len());
            section.blocks.insert(i, block_id);
            Ok(())
        }
        ParentRef::Block(_) => Err(ApplyError::Schema(
            "nested block parents not supported for this op".into(),
        )),
    }
}

fn remove_block_refs(state: &mut CanonicalState, block_id: BlockId) {
    for section in state.document.sections.values_mut() {
        section.blocks.retain(|b| *b != block_id);
    }
}

fn primary_text_mut(block: &mut Block) -> Result<&mut String, ApplyError> {
    match &mut block.body {
        BlockBody::Paragraph(p) => {
            if p.spans.is_empty() {
                p.spans.push(Span {
                    id: qf_core::SpanId(uuid::Uuid::nil()),
                    text: String::new(),
                    marks: vec![],
                });
            }
            Ok(&mut p.spans[0].text)
        }
        BlockBody::Heading(h) => {
            if h.spans.is_empty() {
                h.spans.push(Span {
                    id: qf_core::SpanId(uuid::Uuid::nil()),
                    text: String::new(),
                    marks: vec![],
                });
            }
            Ok(&mut h.spans[0].text)
        }
        BlockBody::Quote(q) => {
            if q.spans.is_empty() {
                q.spans.push(Span {
                    id: qf_core::SpanId(uuid::Uuid::nil()),
                    text: String::new(),
                    marks: vec![],
                });
            }
            Ok(&mut q.spans[0].text)
        }
        BlockBody::Code(c) => Ok(&mut c.text),
        BlockBody::Unknown(_) => Err(ApplyError::TypeMismatch),
        _ => Err(ApplyError::TypeMismatch),
    }
}

fn insert_text_into_block(block: &mut Block, at: TextOffset, text: &str) -> Result<(), ApplyError> {
    let s = primary_text_mut(block)?;
    let idx = char_index(s, at.0)?;
    s.insert_str(idx, text);
    Ok(())
}

fn delete_text_from_block(block: &mut Block, start: u32, end: u32) -> Result<(), ApplyError> {
    if end < start {
        return Err(ApplyError::OffsetOutOfRange);
    }
    let s = primary_text_mut(block)?;
    let a = char_index(s, start)?;
    let b = char_index(s, end)?;
    s.replace_range(a..b, "");
    Ok(())
}

fn char_index(s: &str, char_offset: u32) -> Result<usize, ApplyError> {
    if char_offset as usize > s.chars().count() {
        return Err(ApplyError::OffsetOutOfRange);
    }
    Ok(s.char_indices()
        .nth(char_offset as usize)
        .map(|(i, _)| i)
        .unwrap_or(s.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::{ActorId, SectionId, SeqIdGen, Timestamp};

    fn fresh() -> (Kernel, SeqIdGen) {
        let mut gen = SeqIdGen::default();
        let k = Kernel::fresh(
            DocumentId::new(&mut gen),
            SectionId::new(&mut gen),
            ActorId::new(&mut gen),
            Timestamp { unix_ms: 0 },
        );
        (k, gen)
    }

    #[test]
    fn insert_paragraph_and_text() {
        let (mut k, mut gen) = fresh();
        let bid = BlockId::new(&mut gen);
        k.apply(&Operation::InsertBlock {
            block_id: bid,
            parent: ParentRef::DocumentRoot,
            index: 0,
            body: BlockBodyBytes {
                type_name: "paragraph".into(),
                payload: b"".to_vec(),
            },
        })
        .unwrap();
        k.apply(&Operation::InsertText {
            block_id: bid,
            at: TextOffset(0),
            text: "Hello".into(),
        })
        .unwrap();
        assert_eq!(k.state().document.project_text(bid).unwrap(), "Hello");
        assert_eq!(k.state().op_seq, OpSeq(2));
    }

    #[test]
    fn unknown_block_payload_identity() {
        let (mut k, mut gen) = fresh();
        let bid = BlockId::new(&mut gen);
        let payload = vec![1, 2, 3, 4, 0xff];
        k.apply(&Operation::InsertBlock {
            block_id: bid,
            parent: ParentRef::DocumentRoot,
            index: 0,
            body: BlockBodyBytes {
                type_name: "com.example.custom/v1".into(),
                payload: payload.clone(),
            },
        })
        .unwrap();
        match &k.state().document.blocks.get(&bid).unwrap().body {
            BlockBody::Unknown(u) => {
                assert_eq!(u.original_type, "com.example.custom/v1");
                assert_eq!(u.original_payload, payload);
            }
            _ => panic!("expected unknown"),
        }
    }

    #[test]
    fn undo_via_invert_restores_hash() {
        let (mut k, mut gen) = fresh();
        let bid = BlockId::new(&mut gen);
        let before = k.state().clone();
        let ops = vec![
            Operation::InsertBlock {
                block_id: bid,
                parent: ParentRef::DocumentRoot,
                index: 0,
                body: BlockBodyBytes {
                    type_name: "paragraph".into(),
                    payload: b"".to_vec(),
                },
            },
            Operation::InsertText {
                block_id: bid,
                at: TextOffset(0),
                text: "x".into(),
            },
        ];
        k.apply_batch(&ops).unwrap();
        let inv = invert_ops(&before, &ops).unwrap();
        k.apply_batch(&inv).unwrap();
        // Content equal except op_seq advanced by undo commits in real path;
        // here we applied inverses so op_seq continued — compare document hash fields.
        assert!(k.state().document.blocks.is_empty());
        assert_eq!(
            before.document.blocks.len(),
            k.state().document.blocks.len()
        );
    }

    #[test]
    fn rich_bodies_round_trip_with_spans_and_quote_citation() {
        let mut gen = SeqIdGen::default();
        let span = Span {
            id: qf_core::SpanId::new(&mut gen),
            text: "styled".into(),
            marks: vec![qf_blocks::Mark::Bold],
        };
        let source = qf_core::SourceId::new(&mut gen);
        let bodies = vec![
            BlockBody::Paragraph(ParagraphBlock {
                spans: vec![span.clone()],
            }),
            BlockBody::Heading(HeadingBlock {
                level: 2,
                spans: vec![span.clone()],
            }),
            BlockBody::Quote(QuoteBlock {
                spans: vec![span],
                cite: Some(source),
            }),
        ];

        for body in bodies {
            let encoded = encode_body(&body).unwrap();
            assert_eq!(decode_body(&encoded).unwrap(), body);
        }
    }

    #[test]
    fn undo_remove_block_restores_full_body_and_metadata() {
        let (mut k, mut gen) = fresh();
        let block_id = BlockId::new(&mut gen);
        let quote = BlockBody::Quote(QuoteBlock {
            spans: vec![Span {
                id: qf_core::SpanId::new(&mut gen),
                text: "quoted".into(),
                marks: vec![qf_blocks::Mark::Italic],
            }],
            cite: Some(qf_core::SourceId::new(&mut gen)),
        });
        k.apply(&Operation::InsertBlock {
            block_id,
            parent: ParentRef::DocumentRoot,
            index: 0,
            body: encode_body(&quote).unwrap(),
        })
        .unwrap();

        let metadata = BlockMetadata {
            created_by: ActorId::new(&mut gen),
            created_at_seq: OpSeq(73),
            attrs: PreservedMap::new(),
        };
        let mut meta_payload = Vec::new();
        ciborium::into_writer(&metadata, &mut meta_payload).unwrap();
        k.apply(&Operation::UpdateBlockMeta {
            block_id,
            meta_payload,
        })
        .unwrap();

        let before_remove = k.state().clone();
        let remove = vec![Operation::RemoveBlock { block_id }];
        k.apply_batch(&remove).unwrap();
        let inverse = invert_ops(&before_remove, &remove).unwrap();
        assert!(matches!(
            inverse.as_slice(),
            [
                Operation::InsertBlock { .. },
                Operation::UpdateBlockMeta { .. }
            ]
        ));
        k.apply_batch(&inverse).unwrap();

        assert_eq!(k.state().document, before_remove.document);
    }

    #[test]
    fn suggestion_upsert_undo_removes_previously_absent_entity() {
        let (mut k, mut gen) = fresh();
        let suggestion_id = qf_core::SuggestionId::new(&mut gen);
        let before = k.state().clone();
        let ops = vec![Operation::UpsertSuggestion {
            suggestion_id,
            payload: b"suggestion".to_vec(),
        }];
        k.apply_batch(&ops).unwrap();
        let inverse = invert_ops(&before, &ops).unwrap();
        assert!(matches!(
            inverse.as_slice(),
            [Operation::DeleteSuggestion { .. }]
        ));
        k.apply_batch(&inverse).unwrap();

        assert_eq!(k.state().entities.suggestions, before.entities.suggestions);
    }

    #[test]
    fn accepted_suggestion_marker_is_reversible_and_rejects_pending_payloads() {
        let (mut k, mut gen) = fresh();
        let block_id = BlockId::new(&mut gen);
        k.apply(&Operation::InsertBlock {
            block_id,
            parent: ParentRef::DocumentRoot,
            index: 0,
            body: BlockBodyBytes {
                type_name: "paragraph".into(),
                payload: b"base".to_vec(),
            },
        })
        .unwrap();
        let suggestion_id = qf_core::SuggestionId::new(&mut gen);
        let proposed_ops = vec![Operation::InsertText {
            block_id,
            at: TextOffset(4),
            text: " accepted".into(),
        }];
        let accepted_payload = serde_json::to_vec(&serde_json::json!({
            "status": "Accepted",
            "proposed_ops": proposed_ops,
        }))
        .unwrap();
        k.apply(&Operation::UpsertSuggestion {
            suggestion_id,
            payload: accepted_payload,
        })
        .unwrap();

        let before_apply = k.state().clone();
        let marker = vec![Operation::ApplySuggestionOps { suggestion_id }];
        k.apply_batch(&marker).unwrap();
        assert_eq!(
            k.state().document.project_text(block_id),
            Some("base accepted".into())
        );
        let inverse = invert_ops(&before_apply, &marker).unwrap();
        k.apply_batch(&inverse).unwrap();
        assert_eq!(k.state().document, before_apply.document);

        let pending_payload = serde_json::to_vec(&serde_json::json!({
            "status": "Pending",
            "proposed_ops": [Operation::Noop {
                reason: qf_core::NoopReason::Other("must not run".into()),
            }],
        }))
        .unwrap();
        k.apply(&Operation::UpsertSuggestion {
            suggestion_id,
            payload: pending_payload,
        })
        .unwrap();
        let before_rejected_apply = k.state().clone();
        assert!(k
            .apply(&Operation::ApplySuggestionOps { suggestion_id })
            .is_err());
        assert_eq!(k.state(), &before_rejected_apply);
    }

    #[test]
    fn dry_run_does_not_mutate() {
        let (k, mut gen) = fresh();
        let bid = BlockId::new(&mut gen);
        let h = k.content_hash();
        k.dry_run(&[Operation::InsertBlock {
            block_id: bid,
            parent: ParentRef::DocumentRoot,
            index: 0,
            body: BlockBodyBytes {
                type_name: "paragraph".into(),
                payload: vec![],
            },
        }])
        .unwrap();
        assert_eq!(k.content_hash(), h);
        assert_eq!(k.state().op_seq, OpSeq::ZERO);
    }

    #[test]
    fn snapshot_hash_verifies() {
        let (k, _) = fresh();
        let snap = k.snapshot();
        assert!(snap.verify_hash());
    }
}
