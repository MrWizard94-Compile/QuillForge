//! Minimal deterministic PDF 1.4 writer for text-bearing blocks.
//!
//! Fixed Helvetica, US Letter, no external fonts — same inputs → same bytes.

use crate::{export_plain, ExportConfig};
use qf_citation::SourceTable;
use qf_kernel::CanonicalState;

/// Build a simple multi-line PDF from plain export text.
pub fn export_pdf(
    state: &CanonicalState,
    sources: &SourceTable,
    cfg: &ExportConfig,
) -> Result<Vec<u8>, String> {
    let plain = export_plain(state, sources, cfg);
    let lines: Vec<&str> = plain.lines().take(60).collect();
    build_pdf_lines(&lines)
}

fn build_pdf_lines(lines: &[&str]) -> Result<Vec<u8>, String> {
    // PDF objects:
    // 1: Catalog
    // 2: Pages
    // 3: Page
    // 4: Font
    // 5: Content stream
    let mut content = String::from("BT\n/F1 12 Tf\n14 TL\n72 720 Td\n");
    for (i, line) in lines.iter().enumerate() {
        let escaped = escape_pdf_string(line);
        if i == 0 {
            content.push_str(&format!("({escaped}) Tj\n"));
        } else {
            content.push_str(&format!("T* ({escaped}) Tj\n"));
        }
    }
    content.push_str("ET\n");
    let stream = content.into_bytes();

    let mut s5 = format!("5 0 obj\n<< /Length {} >>\nstream\n", stream.len()).into_bytes();
    s5.extend_from_slice(&stream);
    s5.extend_from_slice(b"\nendstream\nendobj\n");
    let objs: Vec<Vec<u8>> = vec![
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
        b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n".to_vec(),
        b"3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 5 0 R /Resources << /Font << /F1 4 0 R >> >> >>\nendobj\n".to_vec(),
        b"4 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        s5,
    ];

    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objs.len());
    for obj in &objs {
        offsets.push(out.len());
        out.extend_from_slice(obj);
    }
    let xref_pos = out.len();
    out.extend_from_slice(format!("xref\n0 {}\n", objs.len() + 1).as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{}\n%%EOF\n",
            objs.len() + 1,
            xref_pos
        )
        .as_bytes(),
    );
    Ok(out)
}

fn escape_pdf_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\r' | '\n' => out.push(' '),
            c if c.is_ascii() && !c.is_control() => out.push(c),
            _ => out.push('?'), // non-ASCII → placeholder for Helvetica subset
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_blocks::{BlockBody, BlockMetadata, ParagraphBlock, Span};
    use qf_core::{
        ActorId, BlockId, DocumentId, OpSeq, SectionId, SeqIdGen, Timestamp, Version,
    };
    use qf_doc::{Block, Document};
    use qf_kernel::{CanonicalState, EntityTables};

    #[test]
    fn pdf_starts_with_header_and_is_deterministic() {
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
        let bid = BlockId::new(&mut gen);
        state.document.blocks.insert(
            bid,
            Block {
                id: bid,
                meta: BlockMetadata {
                    created_by: actor,
                    created_at_seq: OpSeq(1),
                    attrs: Default::default(),
                },
                body: BlockBody::Paragraph(ParagraphBlock {
                    spans: vec![Span {
                        id: qf_core::SpanId(uuid::Uuid::nil()),
                        text: "Hello PDF".into(),
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
            .push(bid);

        let sources = SourceTable::default();
        let a = export_pdf(&state, &sources, &ExportConfig::default()).unwrap();
        let b = export_pdf(&state, &sources, &ExportConfig::default()).unwrap();
        assert_eq!(a, b);
        assert!(a.starts_with(b"%PDF-1.4"));
        assert!(a.windows(5).any(|w| w == b"%%EOF"));
    }
}
