//! Pure command → operation expansion for document editing.

use crate::Document;
use qf_core::{
    BlockBodyBytes, BlockId, Command, CommandError, IdGen, OffsetRange, Operation, ParentRef,
    TextOffset,
};

/// Expand an editing command against a document snapshot.
///
/// This is pure: no journal, no kernel mutation. New IDs come from `ids`.
pub fn expand_command(
    document: &Document,
    ids: &mut dyn IdGen,
    command: &Command,
) -> Result<Vec<Operation>, CommandError> {
    match command {
        Command::InsertText { target, text } => {
            if !document.contains_block(target.block_id) {
                return Err(CommandError::InvalidTarget);
            }
            if document.project_text(target.block_id).is_none() {
                return Err(CommandError::Invalid("block does not accept text".into()));
            }
            let len = document
                .project_text(target.block_id)
                .map(|t| t.chars().count() as u32)
                .unwrap_or(0);
            if target.offset.0 > len {
                return Err(CommandError::InvalidTarget);
            }
            Ok(vec![Operation::InsertText {
                block_id: target.block_id,
                at: target.offset,
                text: text.clone(),
            }])
        }
        Command::DeleteRange { range } => {
            if range.start.block_id != range.end.block_id {
                return Err(CommandError::Invalid(
                    "cross-block delete not supported in this release".into(),
                ));
            }
            if !document.contains_block(range.start.block_id) {
                return Err(CommandError::InvalidTarget);
            }
            if range.end.offset.0 < range.start.offset.0 {
                return Err(CommandError::Invalid("inverted range".into()));
            }
            Ok(vec![Operation::DeleteText {
                block_id: range.start.block_id,
                range: OffsetRange {
                    start: range.start.offset,
                    end: range.end.offset,
                },
            }])
        }
        Command::ReplaceRange { range, text } => {
            let mut ops = expand_command(
                document,
                ids,
                &Command::DeleteRange {
                    range: range.clone(),
                },
            )?;
            ops.push(Operation::InsertText {
                block_id: range.start.block_id,
                at: range.start.offset,
                text: text.clone(),
            });
            Ok(ops)
        }
        Command::InsertBlock {
            parent,
            index,
            body_type,
            body_payload,
        } => {
            validate_parent(document, parent)?;
            let block_id = BlockId::new(ids);
            Ok(vec![Operation::InsertBlock {
                block_id,
                parent: parent.clone(),
                index: *index,
                body: BlockBodyBytes {
                    type_name: body_type.clone(),
                    payload: body_payload.clone(),
                },
            }])
        }
        Command::DeleteBlock { block_id } => {
            if !document.contains_block(*block_id) {
                return Err(CommandError::InvalidTarget);
            }
            Ok(vec![Operation::RemoveBlock {
                block_id: *block_id,
            }])
        }
        Command::MoveBlock {
            block_id,
            new_parent,
            index,
        } => {
            if !document.contains_block(*block_id) {
                return Err(CommandError::InvalidTarget);
            }
            validate_parent(document, new_parent)?;
            Ok(vec![Operation::MoveBlock {
                block_id: *block_id,
                new_parent: new_parent.clone(),
                index: *index,
            }])
        }
        Command::SetDocumentMeta { patch } => Ok(vec![Operation::SetDocumentMeta {
            patch: patch.clone(),
        }]),
        Command::ApplyMark {
            range,
            mark_payload,
        } => {
            if range.start.block_id != range.end.block_id {
                return Err(CommandError::Invalid(
                    "cross-block marks not supported".into(),
                ));
            }
            let block_id = range.start.block_id;
            if !document.contains_block(block_id) {
                return Err(CommandError::InvalidTarget);
            }
            let block = document
                .blocks
                .get(&block_id)
                .ok_or(CommandError::InvalidTarget)?;
            let span_id = first_span_id(block);
            let mut marks = current_marks(block);
            // mark_payload is CBOR of a single Mark or Vec<Mark>
            if let Ok(one) = ciborium::from_reader::<qf_blocks::Mark, _>(mark_payload.as_slice()) {
                if !marks.iter().any(|m| mark_eq(m, &one)) {
                    marks.push(one);
                }
            } else if let Ok(many) =
                ciborium::from_reader::<Vec<qf_blocks::Mark>, _>(mark_payload.as_slice())
            {
                for m in many {
                    if !marks.iter().any(|x| mark_eq(x, &m)) {
                        marks.push(m);
                    }
                }
            } else {
                return Err(CommandError::Invalid("mark_payload decode failed".into()));
            }
            let mut buf = Vec::new();
            ciborium::into_writer(&marks, &mut buf)
                .map_err(|e| CommandError::Invalid(e.to_string()))?;
            Ok(vec![Operation::SetMarks {
                span_id,
                marks_payload: buf,
            }])
        }
        Command::RemoveMark { range, mark_kind } => {
            if range.start.block_id != range.end.block_id {
                return Err(CommandError::Invalid(
                    "cross-block marks not supported".into(),
                ));
            }
            let block_id = range.start.block_id;
            if !document.contains_block(block_id) {
                return Err(CommandError::InvalidTarget);
            }
            let block = document
                .blocks
                .get(&block_id)
                .ok_or(CommandError::InvalidTarget)?;
            let span_id = first_span_id(block);
            let marks: Vec<qf_blocks::Mark> = current_marks(block)
                .into_iter()
                .filter(|m| mark_kind_name(m) != mark_kind.as_str())
                .collect();
            let mut buf = Vec::new();
            ciborium::into_writer(&marks, &mut buf)
                .map_err(|e| CommandError::Invalid(e.to_string()))?;
            Ok(vec![Operation::SetMarks {
                span_id,
                marks_payload: buf,
            }])
        }
        Command::AddComment { .. }
        | Command::SplitBlock { .. }
        | Command::MergeBlocks { .. }
        | Command::ResolveComment { .. }
        | Command::CreateSuggestion { .. }
        | Command::AcceptSuggestion { .. }
        | Command::RejectSuggestion { .. }
        | Command::AcceptAiSuggestion { .. }
        | Command::PluginCommand { .. }
        | Command::ApplyRemoteBatch { .. } => Err(CommandError::Handler(
            "handled by DocumentSession / specialized paths".into(),
        )),
        Command::Undo { .. } | Command::Redo { .. } => Err(CommandError::Handler(
            "undo/redo must be handled by DocumentSession".into(),
        )),
    }
}

fn first_span_id(block: &crate::Block) -> qf_core::SpanId {
    use qf_blocks::BlockBody;
    match &block.body {
        BlockBody::Paragraph(p) => p
            .spans
            .first()
            .map(|s| s.id)
            .unwrap_or(qf_core::SpanId(uuid::Uuid::nil())),
        BlockBody::Heading(h) => h
            .spans
            .first()
            .map(|s| s.id)
            .unwrap_or(qf_core::SpanId(uuid::Uuid::nil())),
        BlockBody::Quote(q) => q
            .spans
            .first()
            .map(|s| s.id)
            .unwrap_or(qf_core::SpanId(uuid::Uuid::nil())),
        _ => qf_core::SpanId(uuid::Uuid::nil()),
    }
}

fn current_marks(block: &crate::Block) -> Vec<qf_blocks::Mark> {
    use qf_blocks::BlockBody;
    match &block.body {
        BlockBody::Paragraph(p) => p.spans.first().map(|s| s.marks.clone()).unwrap_or_default(),
        BlockBody::Heading(h) => h.spans.first().map(|s| s.marks.clone()).unwrap_or_default(),
        BlockBody::Quote(q) => q.spans.first().map(|s| s.marks.clone()).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn mark_kind_name(m: &qf_blocks::Mark) -> &'static str {
    use qf_blocks::Mark;
    match m {
        Mark::Bold => "bold",
        Mark::Italic => "italic",
        Mark::Underline => "underline",
        Mark::Strike => "strike",
        Mark::Code => "code",
        Mark::Link { .. } => "link",
        Mark::Highlight { .. } => "highlight",
        Mark::CommentRef { .. } => "comment",
        Mark::CitationRef { .. } => "citation",
        Mark::Unknown { .. } => "unknown",
    }
}

fn mark_eq(a: &qf_blocks::Mark, b: &qf_blocks::Mark) -> bool {
    mark_kind_name(a) == mark_kind_name(b)
}

fn validate_parent(document: &Document, parent: &ParentRef) -> Result<(), CommandError> {
    match parent {
        ParentRef::DocumentRoot => Ok(()),
        ParentRef::Section(sid) => {
            if document.sections.contains_key(sid) {
                Ok(())
            } else {
                Err(CommandError::InvalidTarget)
            }
        }
        ParentRef::Block(_) => Err(CommandError::Invalid(
            "nested block parent not supported".into(),
        )),
    }
}

/// Helper to build a paragraph insert command expansion with a known id.
pub fn insert_paragraph_ops(
    block_id: BlockId,
    parent: ParentRef,
    index: u32,
    text: &str,
) -> Vec<Operation> {
    let mut ops = vec![Operation::InsertBlock {
        block_id,
        parent,
        index,
        body: BlockBodyBytes {
            type_name: "paragraph".into(),
            payload: Vec::new(),
        },
    }];
    if !text.is_empty() {
        ops.push(Operation::InsertText {
            block_id,
            at: TextOffset(0),
            text: text.to_string(),
        });
    }
    ops
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;
    use qf_core::{ActorId, DocumentId, SectionId, SeqIdGen, TextTarget, Timestamp};

    #[test]
    fn expand_insert_text_validates_target() {
        let mut gen = SeqIdGen::default();
        let doc = Document::empty(
            DocumentId::new(&mut gen),
            SectionId::new(&mut gen),
            ActorId::new(&mut gen),
            Timestamp { unix_ms: 0 },
        );
        let missing = BlockId::new(&mut gen);
        let err = expand_command(
            &doc,
            &mut gen,
            &Command::InsertText {
                target: TextTarget {
                    block_id: missing,
                    span_id: None,
                    offset: TextOffset(0),
                },
                text: "x".into(),
            },
        )
        .unwrap_err();
        assert_eq!(err, CommandError::InvalidTarget);
    }
}
