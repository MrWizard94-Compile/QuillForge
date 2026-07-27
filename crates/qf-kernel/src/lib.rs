//! Document kernel: pure apply of operations to canonical state.
//!
//! Invariant: apply has no I/O, no RNG, no wall-clock decisions.

use qf_blocks::{BlockBody, BlockMetadata, ParagraphBlock, UnknownBlock};
use qf_core::{
    ApplyError, BlockId, DocumentId, Hash256, OpSeq, Operation, ParentRef, Version,
};
use qf_doc::{Block, Document};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityTables {
    // Entity payloads stored as opaque bytes until specialized crates expand them.
    pub comments: BTreeMap<qf_core::CommentId, Vec<u8>>,
    pub suggestions: BTreeMap<qf_core::SuggestionId, Vec<u8>>,
    pub assets: BTreeMap<qf_core::AssetId, Vec<u8>>,
    pub sources: BTreeMap<qf_core::SourceId, Vec<u8>>,
}

impl Default for EntityTables {
    fn default() -> Self {
        Self {
            comments: BTreeMap::new(),
            suggestions: BTreeMap::new(),
            assets: BTreeMap::new(),
            sources: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CanonicalState {
    pub document_id: DocumentId,
    pub package_format_version: Version,
    pub op_seq: OpSeq,
    pub document: Document,
    pub entities: EntityTables,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocumentSnapshot {
    pub state: CanonicalState,
    pub snapshot_hash: Hash256,
}

impl DocumentSnapshot {
    pub fn from_state(state: CanonicalState) -> Self {
        let bytes = serde_json::to_vec(&state).unwrap_or_default();
        let snapshot_hash = Hash256::sha256(&bytes);
        Self {
            state,
            snapshot_hash,
        }
    }
}

// serde_json used only for snapshot hashing convenience in foundation.
// Production will switch to deterministic CBOR as specified.

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

#[derive(Debug)]
pub struct Kernel {
    state: CanonicalState,
}

impl Kernel {
    pub fn new(state: CanonicalState) -> Self {
        Self { state }
    }

    pub fn from_snapshot(snap: DocumentSnapshot) -> Result<Self, ApplyError> {
        Ok(Self { state: snap.state })
    }

    pub fn state(&self) -> &CanonicalState {
        &self.state
    }

    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot::from_state(self.state.clone())
    }

    pub fn apply_batch(&mut self, ops: &[Operation]) -> Result<Vec<KernelEvent>, ApplyError> {
        let mut events = Vec::new();
        for op in ops {
            events.extend(self.apply(op)?);
        }
        Ok(events)
    }

    pub fn apply(&mut self, op: &Operation) -> Result<Vec<KernelEvent>, ApplyError> {
        // Transactional: work on clone, swap on success.
        let mut next = self.state.clone();
        let events = apply_to(&mut next, op)?;
        next.op_seq = next.op_seq.next();
        if next.op_seq == OpSeq(1) && self.state.op_seq == OpSeq::ZERO {
            // first op already set to 1 via next from ZERO... OpSeq::ZERO.next() = 1. Good.
        }
        self.state = next;
        Ok(events)
    }
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
            let meta = BlockMetadata {
                created_by: state
                    .document
                    .meta
                    .authors
                    .first()
                    .copied()
                    .ok_or_else(|| ApplyError::Invariant("no author".into()))?,
                created_at_seq: state.op_seq.next(),
                attrs: qf_core::PreservedMap::new(),
            };
            let block = Block {
                id: *block_id,
                meta: meta.clone(),
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
        Operation::InsertText {
            block_id,
            at,
            text,
        } => {
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
        Operation::UpsertComment { comment_id, payload } => {
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
        Operation::ApplySuggestionOps { .. } => {
            // Expanded by command handler into concrete ops before journal.
            Err(ApplyError::Schema(
                "ApplySuggestionOps must be expanded before apply".into(),
            ))
        }
        Operation::SetMarks { .. } | Operation::UpdateBlockMeta { .. } => {
            // Foundation: accept without deep mark model wiring.
            Ok(vec![KernelEvent::Noop])
        }
    }
}

fn decode_body(body: &qf_core::BlockBodyBytes) -> Result<BlockBody, ApplyError> {
    if body.type_name == "paragraph" {
        let text = String::from_utf8(body.payload.clone())
            .map_err(|e| ApplyError::Schema(e.to_string()))?;
        return Ok(BlockBody::Paragraph(ParagraphBlock {
            spans: vec![qf_blocks::Span {
                id: qf_core::SpanId(uuid::Uuid::nil()),
                text,
                marks: vec![],
            }],
        }));
    }
    if body.type_name == "unknown" || body.type_name.contains('.') {
        // Plugin-style or explicit unknown: preserve payload.
        return Ok(BlockBody::Unknown(UnknownBlock {
            original_type: body.type_name.clone(),
            original_payload: body.payload.clone(),
            metadata: BlockMetadata {
                created_by: qf_core::ActorId(uuid::Uuid::nil()),
                created_at_seq: OpSeq::ZERO,
                attrs: qf_core::PreservedMap::new(),
            },
        }));
    }
    Err(ApplyError::Schema(format!(
        "unknown body type for foundation decode: {}",
        body.type_name
    )))
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
            "nested block parents not fully wired in foundation".into(),
        )),
    }
}

fn remove_block_refs(state: &mut CanonicalState, block_id: BlockId) {
    for section in state.document.sections.values_mut() {
        section.blocks.retain(|b| *b != block_id);
    }
}

fn insert_text_into_block(
    block: &mut Block,
    at: qf_core::TextOffset,
    text: &str,
) -> Result<(), ApplyError> {
    match &mut block.body {
        BlockBody::Paragraph(p) => {
            if p.spans.is_empty() {
                p.spans.push(qf_blocks::Span {
                    id: qf_core::SpanId(uuid::Uuid::nil()),
                    text: String::new(),
                    marks: vec![],
                });
            }
            let s = &mut p.spans[0].text;
            let idx = char_index(s, at.0)?;
            s.insert_str(idx, text);
            Ok(())
        }
        BlockBody::Unknown(_) => Err(ApplyError::TypeMismatch),
        _ => Err(ApplyError::TypeMismatch),
    }
}

fn delete_text_from_block(block: &mut Block, start: u32, end: u32) -> Result<(), ApplyError> {
    if end < start {
        return Err(ApplyError::OffsetOutOfRange);
    }
    match &mut block.body {
        BlockBody::Paragraph(p) => {
            if p.spans.is_empty() {
                return Ok(());
            }
            let s = &mut p.spans[0].text;
            let a = char_index(s, start)?;
            let b = char_index(s, end)?;
            s.replace_range(a..b, "");
            Ok(())
        }
        _ => Err(ApplyError::TypeMismatch),
    }
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

    fn fresh_kernel() -> Kernel {
        let mut gen = SeqIdGen::default();
        let doc_id = DocumentId::new(&mut gen);
        let root = SectionId::new(&mut gen);
        let actor = ActorId::new(&mut gen);
        let document = Document::empty(doc_id, root, actor, Timestamp { unix_ms: 0 });
        Kernel::new(CanonicalState {
            document_id: doc_id,
            package_format_version: Version::V1_0_0,
            op_seq: OpSeq::ZERO,
            document,
            entities: EntityTables::default(),
        })
    }

    #[test]
    fn insert_paragraph_and_text() {
        let mut gen = SeqIdGen::default();
        let mut k = fresh_kernel();
        let bid = BlockId::new(&mut gen);
        k.apply(&Operation::InsertBlock {
            block_id: bid,
            parent: ParentRef::DocumentRoot,
            index: 0,
            body: qf_core::BlockBodyBytes {
                type_name: "paragraph".into(),
                payload: b"".to_vec(),
            },
        })
        .unwrap();
        k.apply(&Operation::InsertText {
            block_id: bid,
            at: qf_core::TextOffset(0),
            text: "Hello".into(),
        })
        .unwrap();
        assert_eq!(k.state().document.project_text(bid).unwrap(), "Hello");
        assert_eq!(k.state().op_seq, OpSeq(2));
    }

    #[test]
    fn unknown_block_preserved() {
        let mut gen = SeqIdGen::default();
        let mut k = fresh_kernel();
        let bid = BlockId::new(&mut gen);
        let payload = vec![1, 2, 3, 4];
        k.apply(&Operation::InsertBlock {
            block_id: bid,
            parent: ParentRef::DocumentRoot,
            index: 0,
            body: qf_core::BlockBodyBytes {
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
}
