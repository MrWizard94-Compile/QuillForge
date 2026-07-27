//! Native block bodies, marks, and unknown-content preservation.

use qf_core::{ActorId, AssetId, BlockId, ColorId, CommentId, OpSeq, PreservedMap, SourceId};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValidationError {
    #[error("{0}")]
    Message(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockMetadata {
    pub created_by: ActorId,
    pub created_at_seq: OpSeq,
    pub attrs: PreservedMap,
}

/// Unknown / unregistered block. Payload bytes MUST round-trip verbatim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnknownBlock {
    pub original_type: String,
    pub original_payload: Vec<u8>,
    pub metadata: BlockMetadata,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Span {
    pub id: qf_core::SpanId,
    pub text: String,
    pub marks: Vec<Mark>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mark {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
    Link { href: String, title: Option<String> },
    Highlight { color: ColorId },
    CommentRef { comment_id: CommentId },
    CitationRef { source_id: SourceId },
    Unknown { type_name: String, payload: Vec<u8> },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BlockBody {
    Paragraph(ParagraphBlock),
    Heading(HeadingBlock),
    Quote(QuoteBlock),
    Code(CodeBlock),
    List(ListBlock),
    Checklist(ChecklistBlock),
    Table(TableBlock),
    Image(ImageBlock),
    Video(VideoBlock),
    Equation(EquationBlock),
    Diagram(DiagramBlock),
    Citation(CitationBlock),
    Callout(CalloutBlock),
    Embed(EmbedBlock),
    PageBreak,
    SectionBreak,
    Unknown(UnknownBlock),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParagraphBlock {
    pub spans: Vec<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeadingBlock {
    pub level: u8,
    pub spans: Vec<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuoteBlock {
    pub spans: Vec<Span>,
    pub cite: Option<SourceId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeBlock {
    pub language: Option<String>,
    pub text: String,
    pub show_line_numbers: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListItem {
    pub id: BlockId,
    pub spans: Vec<Span>,
    pub children: Vec<ListItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListBlock {
    pub ordered: bool,
    pub start: u32,
    pub items: Vec<ListItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChecklistItem {
    pub id: BlockId,
    pub checked: bool,
    pub spans: Vec<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChecklistBlock {
    pub items: Vec<ChecklistItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableCell {
    pub id: BlockId,
    pub colspan: u32,
    pub rowspan: u32,
    pub blocks: Vec<BlockId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableRow {
    pub cells: Vec<TableCell>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableBlock {
    pub rows: Vec<TableRow>,
    pub column_count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageBlock {
    pub asset_id: AssetId,
    pub alt: String,
    pub caption: Vec<Span>,
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoBlock {
    pub asset_id: AssetId,
    pub alt: String,
    pub caption: Vec<Span>,
    pub poster_asset_id: Option<AssetId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquationBlock {
    pub latex: String,
    pub display_mode: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiagramFormat {
    Mermaid,
    Graphviz,
    Unknown(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagramBlock {
    pub format: DiagramFormat,
    pub source: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationBlock {
    pub source_ids: Vec<SourceId>,
    pub style_hint: Option<String>,
    pub spans: Vec<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum CalloutKind {
    Info,
    Warn,
    Danger,
    Success,
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CalloutBlock {
    pub kind: CalloutKind,
    pub title: Vec<Span>,
    pub body_blocks: Vec<BlockId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbedBlock {
    pub provider: String,
    pub reference: String,
    pub payload: Vec<u8>,
}

/// Encode unknown block for journal/package transport.
pub fn encode_unknown(u: &UnknownBlock) -> (String, Vec<u8>) {
    (u.original_type.clone(), u.original_payload.clone())
}

/// Decode unregistered type into UnknownBlock — payload preserved exactly.
pub fn unknown_from_raw(
    original_type: String,
    original_payload: Vec<u8>,
    metadata: BlockMetadata,
) -> UnknownBlock {
    UnknownBlock {
        original_type,
        original_payload,
        metadata,
    }
}

pub fn validate_block_body(body: &BlockBody) -> Result<(), ValidationError> {
    match body {
        BlockBody::Heading(h) if !(1..=6).contains(&h.level) => Err(ValidationError::Message(
            "heading level must be 1..=6".into(),
        )),
        BlockBody::Unknown(u) if u.original_type.is_empty() => Err(ValidationError::Message(
            "unknown block type must be non-empty".into(),
        )),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::{ActorId, OpSeq, SeqIdGen};

    #[test]
    fn unknown_payload_identity() {
        let mut gen = SeqIdGen::default();
        let meta = BlockMetadata {
            created_by: ActorId::new(&mut gen),
            created_at_seq: OpSeq(1),
            attrs: PreservedMap::new(),
        };
        let payload = vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0xff];
        let u = unknown_from_raw("com.test/v1".into(), payload.clone(), meta);
        let (t, p) = encode_unknown(&u);
        assert_eq!(t, "com.test/v1");
        assert_eq!(p, payload);
    }
}
