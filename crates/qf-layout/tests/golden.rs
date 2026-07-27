//! M3: deterministic layout golden fixtures.
//!
//! Hashes live under `fixtures/layout/*.hash` (workspace-relative).

use qf_blocks::{
    BlockBody, BlockMetadata, CalloutBlock, CalloutKind, ChecklistBlock, ChecklistItem, CodeBlock,
    HeadingBlock, ListBlock, ListItem, ParagraphBlock, Span, TableBlock, TableCell, TableRow,
    UnknownBlock,
};
use qf_core::{
    ActorId, BlockId, DocumentId, Hash256, OpSeq, SectionId, SeqIdGen, Timestamp, Version,
};
use qf_doc::{Block, Document};
use qf_kernel::{CanonicalState, EntityTables};
use qf_layout::{DirtySet, LayoutConfig, LayoutEngine};
use std::fs;
use std::path::PathBuf;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/layout")
}

fn read_hash(name: &str) -> String {
    let path = fixture_dir().join(name);
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("missing golden {}: {e}", path.display()))
        .trim()
        .to_lowercase()
}

fn hash_hex(h: Hash256) -> String {
    let mut s = String::with_capacity(64);
    for b in h.0 {
        use std::fmt::Write;
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn meta(actor: ActorId, seq: u64) -> BlockMetadata {
    BlockMetadata {
        created_by: actor,
        created_at_seq: OpSeq(seq),
        attrs: Default::default(),
    }
}

fn span(text: &str) -> Span {
    Span {
        id: qf_core::SpanId(uuid::Uuid::nil()),
        text: text.into(),
        marks: vec![],
    }
}

fn empty_state(gen: &mut SeqIdGen) -> CanonicalState {
    let doc_id = DocumentId::new(gen);
    let root = SectionId::new(gen);
    let actor = ActorId::new(gen);
    CanonicalState {
        document_id: doc_id,
        package_format_version: Version::V1_0_0,
        op_seq: OpSeq::ZERO,
        document: Document::empty(doc_id, root, actor, Timestamp { unix_ms: 0 }),
        entities: EntityTables::default(),
    }
}

fn push_block(state: &mut CanonicalState, id: BlockId, body: BlockBody) {
    let actor = state.document.meta.authors[0];
    state.document.blocks.insert(
        id,
        Block {
            id,
            meta: meta(actor, 1),
            body,
        },
    );
    let root = state.document.root_section_id;
    state
        .document
        .sections
        .get_mut(&root)
        .expect("root")
        .blocks
        .push(id);
}

fn mixed_document(gen: &mut SeqIdGen) -> CanonicalState {
    let mut state = empty_state(gen);
    let actor = state.document.meta.authors[0];

    let ids: Vec<BlockId> = (0..12).map(|_| BlockId::new(gen)).collect();

    push_block(
        &mut state,
        ids[0],
        BlockBody::Heading(HeadingBlock {
            level: 1,
            spans: vec![span("Title")],
        }),
    );
    push_block(
        &mut state,
        ids[1],
        BlockBody::Paragraph(ParagraphBlock {
            spans: vec![span("Hello golden layout.")],
        }),
    );
    push_block(
        &mut state,
        ids[2],
        BlockBody::Code(CodeBlock {
            language: Some("rs".into()),
            text: "fn main() {}\n// line2".into(),
            show_line_numbers: true,
        }),
    );
    push_block(
        &mut state,
        ids[3],
        BlockBody::List(ListBlock {
            ordered: true,
            start: 1,
            items: vec![
                ListItem {
                    id: BlockId::new(gen),
                    spans: vec![span("one")],
                    children: vec![],
                },
                ListItem {
                    id: BlockId::new(gen),
                    spans: vec![span("two")],
                    children: vec![],
                },
            ],
        }),
    );
    push_block(
        &mut state,
        ids[4],
        BlockBody::Checklist(ChecklistBlock {
            items: vec![ChecklistItem {
                id: BlockId::new(gen),
                checked: true,
                spans: vec![span("done")],
            }],
        }),
    );
    let cell_id = BlockId::new(gen);
    push_block(
        &mut state,
        ids[5],
        BlockBody::Table(TableBlock {
            column_count: 1,
            rows: vec![TableRow {
                cells: vec![TableCell {
                    id: cell_id,
                    colspan: 1,
                    rowspan: 1,
                    blocks: vec![],
                }],
            }],
        }),
    );
    push_block(
        &mut state,
        ids[6],
        BlockBody::Callout(CalloutBlock {
            kind: CalloutKind::Info,
            title: vec![span("Note")],
            body_blocks: vec![],
        }),
    );
    push_block(
        &mut state,
        ids[7],
        BlockBody::Unknown(UnknownBlock {
            original_type: "com.example.x/v1".into(),
            original_payload: vec![1, 2, 3],
            metadata: meta(actor, 2),
        }),
    );
    push_block(&mut state, ids[8], BlockBody::PageBreak);
    push_block(
        &mut state,
        ids[9],
        BlockBody::Paragraph(ParagraphBlock {
            spans: vec![span("After break")],
        }),
    );
    push_block(&mut state, ids[10], BlockBody::SectionBreak);
    push_block(
        &mut state,
        ids[11],
        BlockBody::Equation(qf_blocks::EquationBlock {
            latex: "e=mc^2".into(),
            display_mode: true,
        }),
    );

    state.op_seq = OpSeq(12);
    state
}

fn long_paragraph_document(gen: &mut SeqIdGen) -> CanonicalState {
    let mut state = empty_state(gen);
    let id = BlockId::new(gen);
    let long = "word ".repeat(200);
    push_block(
        &mut state,
        id,
        BlockBody::Paragraph(ParagraphBlock {
            spans: vec![span(&long)],
        }),
    );
    state.op_seq = OpSeq(1);
    state
}

#[test]
fn golden_empty() {
    let mut gen = SeqIdGen::default();
    let state = empty_state(&mut gen);
    let tree = LayoutEngine::new().full_layout(&state, &LayoutConfig::default());
    assert_eq!(hash_hex(tree.content_hash()), read_hash("empty.hash"));
}

#[test]
fn golden_mixed_blocks() {
    let mut gen = SeqIdGen::default();
    // Fixed seed path: SeqIdGen from 1 — golden depends on block id order only via structure
    let state = mixed_document(&mut gen);
    let tree = LayoutEngine::new().full_layout(&state, &LayoutConfig::default());
    assert_eq!(hash_hex(tree.content_hash()), read_hash("mixed_blocks.hash"));
    assert!(tree.pages.len() >= 2, "page break should create pages");
}

#[test]
fn golden_long_paragraph() {
    let mut gen = SeqIdGen::default();
    let state = long_paragraph_document(&mut gen);
    let tree = LayoutEngine::new().full_layout(&state, &LayoutConfig::default());
    assert_eq!(
        hash_hex(tree.content_hash()),
        read_hash("long_paragraph.hash")
    );
}

#[test]
fn incremental_equals_full_on_mixed() {
    let mut gen = SeqIdGen::default();
    let state = mixed_document(&mut gen);
    let eng = LayoutEngine::new();
    let cfg = LayoutConfig::default();
    let full = eng.full_layout(&state, &cfg);
    let dirty = eng.reflow_dirty(
        &state,
        &cfg,
        DirtySet {
            full: true,
            block_ids: vec![],
        },
    );
    assert_eq!(full.content_hash(), dirty.content_hash());
}

/// Helper used when regenerating goldens: `UPDATE_GOLDENS=1 cargo test -p qf-layout --test golden`
#[test]
fn maybe_write_goldens() {
    if std::env::var("UPDATE_GOLDENS").ok().as_deref() != Some("1") {
        return;
    }
    let dir = fixture_dir();
    fs::create_dir_all(&dir).unwrap();
    let mut gen = SeqIdGen::default();
    let eng = LayoutEngine::new();
    let cfg = LayoutConfig::default();

    let empty = eng.full_layout(&empty_state(&mut gen), &cfg);
    fs::write(dir.join("empty.hash"), format!("{}\n", hash_hex(empty.content_hash()))).unwrap();

    let mut gen = SeqIdGen::default();
    let mixed = eng.full_layout(&mixed_document(&mut gen), &cfg);
    fs::write(
        dir.join("mixed_blocks.hash"),
        format!("{}\n", hash_hex(mixed.content_hash())),
    )
    .unwrap();

    let mut gen = SeqIdGen::default();
    let long = eng.full_layout(&long_paragraph_document(&mut gen), &cfg);
    fs::write(
        dir.join("long_paragraph.hash"),
        format!("{}\n", hash_hex(long.content_hash())),
    )
    .unwrap();
}
