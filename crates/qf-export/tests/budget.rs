//! PB-EXPORT-001 — Markdown + PDF + DOCX export budget.

use qf_blocks::{BlockBody, BlockMetadata, ParagraphBlock, Span};
use qf_core::{ActorId, BlockId, DocumentId, OpSeq, SectionId, SeqIdGen, Timestamp, Version};
use qf_doc::{Block, Document};
use qf_export::{CoreExporter, ExportFormat, ExportRequest, Exporter};
use qf_kernel::{CanonicalState, EntityTables};
use std::time::{Duration, Instant};

#[test]
fn budget_export_triple_under_limit() {
    let mut gen = SeqIdGen::default();
    let doc_id = DocumentId::new(&mut gen);
    let root = SectionId::new(&mut gen);
    let actor = ActorId::new(&mut gen);
    let mut state = CanonicalState {
        document_id: doc_id,
        package_format_version: Version::V1_0_0,
        op_seq: OpSeq(1),
        document: Document::empty(doc_id, root, actor, Timestamp { unix_ms: 0 }),
        entities: EntityTables::default(),
    };
    for i in 0..100 {
        let id = BlockId::new(&mut gen);
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
                    spans: vec![Span {
                        id: qf_core::SpanId(uuid::Uuid::nil()),
                        text: format!("Export paragraph {i}"),
                        marks: vec![],
                    }],
                }),
            },
        );
        state
            .document
            .sections
            .get_mut(&root)
            .unwrap()
            .blocks
            .push(id);
    }

    let exporter = CoreExporter;
    let start = Instant::now();
    for format in [
        ExportFormat::Markdown,
        ExportFormat::Pdf,
        ExportFormat::Docx,
    ] {
        let out = exporter
            .export(
                &state,
                &ExportRequest {
                    format,
                    config: Default::default(),
                },
            )
            .unwrap();
        assert!(!out.bytes.is_empty());
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(5),
        "PB-EXPORT-001 exceeded: {elapsed:?}"
    );
}
