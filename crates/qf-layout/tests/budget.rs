//! PB-LAYOUT — CI-gated layout performance budgets (docs/26-performance-budgets.md).

use qf_blocks::{BlockBody, BlockMetadata, HeadingBlock, ParagraphBlock, Span};
use qf_core::{ActorId, BlockId, DocumentId, OpSeq, SectionId, SeqIdGen, Timestamp, Version};
use qf_doc::{Block, Document};
use qf_kernel::{CanonicalState, EntityTables};
use qf_layout::{DirtySet, LayoutConfig, LayoutEngine};
use std::time::{Duration, Instant};

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

fn push_paragraph(state: &mut CanonicalState, gen: &mut SeqIdGen, text: &str) {
    let id = BlockId::new(gen);
    let actor = state.document.meta.authors[0];
    state.document.blocks.insert(
        id,
        Block {
            id,
            meta: BlockMetadata {
                created_by: actor,
                created_at_seq: OpSeq(1),
                attrs: Default::default(),
            },
            body: BlockBody::Paragraph(ParagraphBlock {
                spans: vec![span(text)],
            }),
        },
    );
    let root = state.document.root_section_id;
    state
        .document
        .sections
        .get_mut(&root)
        .unwrap()
        .blocks
        .push(id);
}

#[test]
fn budget_layout_mixed_200_under_limit() {
    let mut gen = SeqIdGen::default();
    let mut state = empty_state(&mut gen);
    // Heading + 199 paragraphs
    let hid = BlockId::new(&mut gen);
    let actor = state.document.meta.authors[0];
    state.document.blocks.insert(
        hid,
        Block {
            id: hid,
            meta: BlockMetadata {
                created_by: actor,
                created_at_seq: OpSeq(1),
                attrs: Default::default(),
            },
            body: BlockBody::Heading(HeadingBlock {
                level: 1,
                spans: vec![span("Budget Doc")],
            }),
        },
    );
    state
        .document
        .sections
        .get_mut(&state.document.root_section_id)
        .unwrap()
        .blocks
        .push(hid);
    for i in 0..199 {
        push_paragraph(
            &mut state,
            &mut gen,
            &format!("Paragraph {i} with some text."),
        );
    }

    let engine = LayoutEngine::new();
    let cfg = LayoutConfig::default();
    let start = Instant::now();
    let tree = engine.full_layout(&state, &cfg);
    let elapsed = start.elapsed();
    assert!(!tree.pages.is_empty());
    assert!(
        elapsed < Duration::from_secs(5),
        "PB-LAYOUT-001 exceeded: {elapsed:?}"
    );
}

#[test]
fn budget_incremental_reflow_under_limit() {
    let mut gen = SeqIdGen::default();
    let mut state = empty_state(&mut gen);
    let mut block_ids = Vec::new();
    for i in 0..50 {
        let id = BlockId::new(&mut gen);
        block_ids.push(id);
        let actor = state.document.meta.authors[0];
        state.document.blocks.insert(
            id,
            Block {
                id,
                meta: BlockMetadata {
                    created_by: actor,
                    created_at_seq: OpSeq(1),
                    attrs: Default::default(),
                },
                body: BlockBody::Paragraph(ParagraphBlock {
                    spans: vec![span(&format!("Line {i}"))],
                }),
            },
        );
        state
            .document
            .sections
            .get_mut(&state.document.root_section_id)
            .unwrap()
            .blocks
            .push(id);
    }

    let engine = LayoutEngine::new();
    let cfg = LayoutConfig::default();
    let start = Instant::now();
    let mut last_hash = engine.full_layout(&state, &cfg).content_hash();
    for edit in 0..20 {
        let target = block_ids[edit % block_ids.len()];
        if let Some(b) = state.document.blocks.get_mut(&target) {
            if let BlockBody::Paragraph(p) = &mut b.body {
                p.spans[0].text = format!("Edited {edit}");
            }
        }
        let dirty = DirtySet {
            block_ids: vec![target],
            full: false,
        };
        let inc = engine.reflow_dirty(&state, &cfg, dirty);
        let full = engine.full_layout(&state, &cfg);
        assert_eq!(inc.content_hash(), full.content_hash());
        last_hash = full.content_hash();
    }
    let elapsed = start.elapsed();
    let _ = last_hash;
    assert!(
        elapsed < Duration::from_secs(8),
        "PB-LAYOUT-002 exceeded: {elapsed:?}"
    );
}
