# Document AST Specification

**Document ID:** QF-AST-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Hierarchy

```
Document
 └── Section* (ordered tree; root section implicit or explicit)
      └── Block* (ordered)
           └── Span* (ordered inline runs)   // for text-bearing blocks
```

Plus parallel entity tables: Comment, Suggestion, Revision, AssetRef, SourceRef.

---

## 2. Core types

```rust
pub struct Document {
    pub id: DocumentId,
    pub meta: DocumentMeta,
    pub root_section_id: SectionId,
    pub sections: BTreeMap<SectionId, Section>,
    pub blocks: BTreeMap<BlockId, Block>,
    /// Stable order of top-level sections under root.
    pub section_order: Vec<SectionId>,
}

pub struct DocumentMeta {
    pub title: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp, // audit; kernel may set via meta ops only
    pub authors: Vec<ActorId>,
    pub language: Option<String>, // BCP-47
    pub custom: PreservedMap,     // unknown keys preserved
}

pub struct Section {
    pub id: SectionId,
    pub parent: Option<SectionId>,
    pub children: Vec<SectionId>,
    pub blocks: Vec<BlockId>,
    pub meta: SectionMeta,
}

pub struct SectionMeta {
    pub title: Option<String>,
    pub custom: PreservedMap,
}

pub struct Block {
    pub id: BlockId,
    pub meta: BlockMetadata,
    pub body: BlockBody,
}

pub struct BlockMetadata {
    pub created_by: ActorId,
    pub created_at_seq: OpSeq,
    pub attrs: PreservedMap, // class, anchors, plugin attrs
}

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
    PageBreak(PageBreakBlock),
    SectionBreak(SectionBreakBlock),
    Unknown(UnknownBlock),
}

pub struct Span {
    pub id: SpanId,
    pub text: String,
    pub marks: Vec<Mark>,
}

pub struct Comment {
    pub id: CommentId,
    pub anchor: Anchor,
    pub author: ActorId,
    pub body: String,
    pub created_at: Timestamp,
    pub resolved: bool,
    pub parent: Option<CommentId>, // threading
    pub custom: PreservedMap,
}

pub struct Suggestion {
    pub id: SuggestionId,
    pub author: ActorId,
    pub note: Option<String>,
    pub proposed_ops: Vec<Operation>,
    pub status: SuggestionStatus, // Pending | Accepted | Rejected
    pub source: SuggestionSource, // Human | Ai { provider }
}

pub struct Revision {
    pub id: RevisionId,
    pub label: String,
    pub at_op_seq: OpSeq,
    pub created_by: ActorId,
    pub snapshot_hash: Hash256,
}

pub struct AssetRef {
    pub id: AssetId,
    pub path_in_package: String, // e.g. "assets/img/abc.png"
    pub mime: String,
    pub content_hash: Hash256,
    pub meta: PreservedMap,
}

pub struct SourceRef {
    pub id: SourceId,
    pub kind: SourceKind, // Url | Doi | Isbn | Local | Other
    pub locator: String,
    pub meta: PreservedMap,
}
```

---

## 3. Native block definitions

### 3.1 Paragraph

```rust
pub struct ParagraphBlock {
    pub spans: Vec<Span>,
}
```

### 3.2 Heading

```rust
pub struct HeadingBlock {
    pub level: u8, // 1..=6
    pub spans: Vec<Span>,
}
```

### 3.3 Quote

```rust
pub struct QuoteBlock {
    pub spans: Vec<Span>,
    pub cite: Option<SourceId>,
}
```

### 3.4 Code

```rust
pub struct CodeBlock {
    pub language: Option<String>,
    pub text: String, // plain; marks not applied inside v1
    pub show_line_numbers: bool,
}
```

### 3.5 List

```rust
pub struct ListBlock {
    pub ordered: bool,
    pub start: u32, // for ordered
    pub items: Vec<ListItem>,
}

pub struct ListItem {
    pub id: BlockId, // items are addressable
    pub spans: Vec<Span>,
    pub children: Vec<ListItem>, // nested lists
}
```

### 3.6 Checklist

```rust
pub struct ChecklistBlock {
    pub items: Vec<ChecklistItem>,
}

pub struct ChecklistItem {
    pub id: BlockId,
    pub checked: bool,
    pub spans: Vec<Span>,
}
```

### 3.7 Table

```rust
pub struct TableBlock {
    pub rows: Vec<TableRow>,
    pub column_count: u32,
}

pub struct TableRow {
    pub cells: Vec<TableCell>,
}

pub struct TableCell {
    pub id: BlockId,
    pub colspan: u32,
    pub rowspan: u32,
    pub blocks: Vec<BlockId>, // nested content blocks
}
```

### 3.8 Image

```rust
pub struct ImageBlock {
    pub asset_id: AssetId,
    pub alt: String,
    pub caption: Vec<Span>,
    pub width_px: Option<u32>,
    pub height_px: Option<u32>,
}
```

### 3.9 Video

```rust
pub struct VideoBlock {
    pub asset_id: AssetId,
    pub alt: String,
    pub caption: Vec<Span>,
    pub poster_asset_id: Option<AssetId>,
}
```

### 3.10 Equation

```rust
pub struct EquationBlock {
    pub latex: String,
    pub display_mode: bool,
}
```

### 3.11 Diagram

```rust
pub struct DiagramBlock {
    pub format: DiagramFormat, // Mermaid | Graphviz | Unknown(String)
    pub source: String,
}
```

### 3.12 Citation

```rust
pub struct CitationBlock {
    pub source_ids: Vec<SourceId>,
    pub style_hint: Option<String>,
    pub spans: Vec<Span>, // rendered cite text cache is derived; spans are canonical text
}
```

### 3.13 Callout

```rust
pub struct CalloutBlock {
    pub kind: CalloutKind, // Info | Warn | Danger | Success | Custom(String)
    pub title: Vec<Span>,
    pub body_blocks: Vec<BlockId>,
}
```

### 3.14 Embed

```rust
pub struct EmbedBlock {
    pub provider: String,
    pub reference: String,
    pub payload: Vec<u8>, // provider-specific; treat as potentially unknown
}
```

### 3.15 PageBreak / SectionBreak

```rust
pub struct PageBreakBlock;
pub struct SectionBreakBlock;
```

### 3.16 UnknownBlock

```rust
pub struct UnknownBlock {
    pub original_type: String,
    pub original_payload: Vec<u8>,
    pub metadata: BlockMetadata,
}
```

See [08-unknown-content.md](08-unknown-content.md) for full preservation rules.

---

## 4. Span marks

```rust
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
    /// Extension mark from plugin; preserved if unknown
    Unknown { type_name: String, payload: Vec<u8> },
}

pub enum MarkKind {
    Bold,
    Italic,
    Underline,
    Strike,
    Code,
    Link,
    Highlight,
    CommentRef,
    CitationRef,
    Unknown { type_name: String },
}
```

**Rules:**

- Marks on a span apply to the entire span text.  
- Partial-range mark application splits spans (command handler responsibility).  
- Mark order in `Vec<Mark>` is sorted by discriminant then payload for determinism.

---

## 5. Anchors

```rust
pub enum Anchor {
    Block { block_id: BlockId },
    Range { range: DocRangeStable },
    Point { target: TextTarget },
}

pub struct DocRangeStable {
    pub start_block: BlockId,
    pub start_offset: TextOffset,
    pub end_block: BlockId,
    pub end_offset: TextOffset,
}
```

---

## 6. Serialization rules

| Rule | Detail |
|------|--------|
| Format in package | CBOR (canonical) for snapshots; JSON allowed for debug exports only |
| Map keys | Sorted |
| Enum tags | Explicit string type names for block bodies |
| Unknown fields at document/meta level | Stored in `PreservedMap` |
| Floats | Avoid; colors as sRGB u8 channels |

**Decision:** Canonical on-disk snapshot encoding is **deterministic CBOR** (RFC 8949 definite lengths, sorted map keys). Justification: compactness + strict determinism vs JSON.

---

## 7. Text model

- Strings are UTF-8.  
- Offsets are Unicode scalar offsets within the block's editable text projection.  
- For multi-span blocks, offsets count across span concatenation in order.  
- `\n` inside paragraph spans is discouraged; prefer split blocks. Kernel MAY allow but layout treats as line break.

---

## 8. Invariants (AST-level)

1. Every `BlockId` in section.blocks exists in `blocks`.  
2. No block appears in two parents.  
3. Span IDs unique document-wide.  
4. Heading level in 1..=6.  
5. Table `column_count` matches row structure after colspan accounting.  
6. UnknownBlock payload is opaque and never reparsed into other variants without plugin registry match.

---

## 9. Mapping unknown types on load

```text
If block type registered → decode to native/plugin type
Else → BlockBody::Unknown(UnknownBlock { original_type, original_payload, metadata })
```

Plugin reinstall with same `original_type` MAY promote Unknown → typed on load **without** changing payload bytes if decode succeeds; if decode fails, remain Unknown.

---

*End of document AST specification.*
