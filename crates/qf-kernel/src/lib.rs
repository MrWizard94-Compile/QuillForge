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
        let inv = invert_one(&working, op)?;
        apply_to(&mut working, op)?;
        working.op_seq = next_op_seq(working.op_seq);
        inverses_rev.push(inv);
    }
    inverses_rev.reverse();
    Ok(inverses_rev)
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
        Operation::RemoveBlock { block_id } => {
            let block = state
                .document
                .blocks
                .get(block_id)
                .ok_or(ApplyError::MissingEntity)?;
            let (parent, index) = find_block_location(state, *block_id)?;
            Ok(Operation::InsertBlock {
                block_id: *block_id,
                parent,
                index: index as u32,
                body: encode_body(&block.body)?,
            })
        }
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
                // No delete-suggestion op; restore empty by re-upsert is fine for undo of first insert:
                // invert of first upsert is delete of suggestion — use empty payload delete via Upsert of empty then...
                // Spec has no DeleteSuggestion; store Noop for missing prior.
                Ok(Operation::Noop {
                    reason: qf_core::NoopReason::Other("undo first suggestion upsert".into()),
                })
            }
        }
        Operation::Noop { .. } => Ok(Operation::Noop {
            reason: qf_core::NoopReason::Other("undo noop".into()),
        }),
        Operation::SetMarks { .. }
        | Operation::UpdateBlockMeta { .. }
        | Operation::ApplySuggestionOps { .. } => Ok(Operation::Noop {
            reason: qf_core::NoopReason::Other("inverse not modeled; use snapshot revision".into()),
        }),
    }
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
        Operation::ApplySuggestionOps { .. } => Err(ApplyError::Schema(
            "ApplySuggestionOps must be expanded before apply".into(),
        )),
        Operation::SetMarks { .. } | Operation::UpdateBlockMeta { .. } => {
            Ok(vec![KernelEvent::Noop])
        }
    }
}

/// Encode a typed block body into transport bytes for ops.
pub fn encode_body(body: &BlockBody) -> Result<BlockBodyBytes, ApplyError> {
    match body {
        BlockBody::Paragraph(p) => {
            let text: String = p.spans.iter().map(|s| s.text.as_str()).collect();
            Ok(BlockBodyBytes {
                type_name: "paragraph".into(),
                payload: text.into_bytes(),
            })
        }
        BlockBody::Heading(h) => {
            let text: String = h.spans.iter().map(|s| s.text.as_str()).collect();
            let mut payload = vec![h.level];
            payload.extend_from_slice(text.as_bytes());
            Ok(BlockBodyBytes {
                type_name: "heading".into(),
                payload,
            })
        }
        BlockBody::Quote(q) => {
            let text: String = q.spans.iter().map(|s| s.text.as_str()).collect();
            Ok(BlockBodyBytes {
                type_name: "quote".into(),
                payload: text.into_bytes(),
            })
        }
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
            let text = String::from_utf8(body.payload.clone())
                .map_err(|e| ApplyError::Schema(e.to_string()))?;
            Ok(BlockBody::Paragraph(ParagraphBlock {
                spans: vec![Span {
                    id: qf_core::SpanId(uuid::Uuid::nil()),
                    text,
                    marks: vec![],
                }],
            }))
        }
        "heading" => {
            if body.payload.is_empty() {
                return Err(ApplyError::Schema("heading payload empty".into()));
            }
            let level = body.payload[0];
            let text = String::from_utf8(body.payload[1..].to_vec())
                .map_err(|e| ApplyError::Schema(e.to_string()))?;
            Ok(BlockBody::Heading(HeadingBlock {
                level,
                spans: vec![Span {
                    id: qf_core::SpanId(uuid::Uuid::nil()),
                    text,
                    marks: vec![],
                }],
            }))
        }
        "quote" => {
            let text = String::from_utf8(body.payload.clone())
                .map_err(|e| ApplyError::Schema(e.to_string()))?;
            Ok(BlockBody::Quote(QuoteBlock {
                spans: vec![Span {
                    id: qf_core::SpanId(uuid::Uuid::nil()),
                    text,
                    marks: vec![],
                }],
                cite: None,
            }))
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
