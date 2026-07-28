//! Deterministic multi-page PDF 1.4 writer for text-bearing blocks.
//!
//! Fixed Helvetica, US Letter, no external fonts — same inputs → same bytes.
//! Lines paginate at a fixed line budget so long documents produce multiple pages.

use crate::{export_plain, ExportConfig};
use qf_citation::SourceTable;
use qf_kernel::CanonicalState;

/// Max lines exported into PDF (hard cap for foundation memory budget).
const MAX_LINES: usize = 450;
/// Usable body lines per US Letter page at 12 pt / 14 TL with 72 pt margins.
const LINES_PER_PAGE: usize = 45;

/// Build a multi-page PDF from plain export text.
pub fn export_pdf(
    state: &CanonicalState,
    sources: &SourceTable,
    cfg: &ExportConfig,
) -> Result<Vec<u8>, String> {
    let plain = export_plain(state, sources, cfg);
    let lines: Vec<&str> = plain.lines().take(MAX_LINES).collect();
    build_pdf_lines(&lines)
}

fn build_pdf_lines(lines: &[&str]) -> Result<Vec<u8>, String> {
    let page_chunks: Vec<&[&str]> = if lines.is_empty() {
        vec![&[][..]]
    } else {
        lines.chunks(LINES_PER_PAGE).collect()
    };
    let page_count = page_chunks.len();

    // Object layout:
    // 1 Catalog, 2 Pages, 3 Font,
    // then for page i: obj (4+2i) = Page, obj (5+2i) = Content stream
    let mut numbered: Vec<(usize, Vec<u8>)> = Vec::with_capacity(3 + page_count * 2);

    numbered.push((
        1,
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
    ));

    let mut kids = String::new();
    for i in 0..page_count {
        let page_obj = 4 + i * 2;
        kids.push_str(&format!("{page_obj} 0 R "));
    }
    numbered.push((
        2,
        format!("2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {page_count} >>\nendobj\n")
            .into_bytes(),
    ));
    numbered.push((
        3,
        b"3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
    ));

    for (i, chunk) in page_chunks.iter().enumerate() {
        let page_obj = 4 + i * 2;
        let content_obj = 5 + i * 2;
        numbered.push((
            page_obj,
            format!(
                "{page_obj} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {content_obj} 0 R /Resources << /Font << /F1 3 0 R >> >> >>\nendobj\n"
            )
            .into_bytes(),
        ));
        let stream = page_content_stream(chunk);
        let mut content_obj_bytes =
            format!("{content_obj} 0 obj\n<< /Length {} >>\nstream\n", stream.len()).into_bytes();
        content_obj_bytes.extend_from_slice(&stream);
        content_obj_bytes.extend_from_slice(b"\nendstream\nendobj\n");
        numbered.push((content_obj, content_obj_bytes));
    }

    numbered.sort_by_key(|(n, _)| *n);
    let max_obj = numbered.last().map(|(n, _)| *n).unwrap_or(0);

    let mut out: Vec<u8> = b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = vec![0usize; max_obj + 1];
    for (num, body) in &numbered {
        offsets[*num] = out.len();
        out.extend_from_slice(body);
    }

    let xref_pos = out.len();
    let size = max_obj + 1;
    out.extend_from_slice(format!("xref\n0 {size}\n").as_bytes());
    out.extend_from_slice(b"0000000000 65535 f \n");
    for off in offsets.iter().take(max_obj + 1).skip(1) {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n")
            .as_bytes(),
    );
    Ok(out)
}

fn page_content_stream(lines: &[&str]) -> Vec<u8> {
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
    content.into_bytes()
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
            _ => out.push('?'),
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

    fn state_with_paragraphs(n: usize, prefix: &str) -> CanonicalState {
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
        for i in 0..n {
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
                            text: format!("{prefix} {i}"),
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
        }
        state
    }

    #[test]
    fn pdf_starts_with_header_and_is_deterministic() {
        let state = state_with_paragraphs(1, "Hello PDF");
        let sources = SourceTable::default();
        let a = export_pdf(&state, &sources, &ExportConfig::default()).unwrap();
        let b = export_pdf(&state, &sources, &ExportConfig::default()).unwrap();
        assert_eq!(a, b);
        assert!(a.starts_with(b"%PDF-1.4"));
        assert!(a.windows(5).any(|w| w == b"%%EOF"));
    }

    #[test]
    fn pdf_multipage_for_long_document() {
        let state = state_with_paragraphs(LINES_PER_PAGE + 5, "Line");
        let sources = SourceTable::default();
        let pdf = export_pdf(&state, &sources, &ExportConfig::default()).unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        let s = String::from_utf8_lossy(&pdf);
        assert!(
            s.contains("/Count 2") || s.contains("/Count 3"),
            "expected multi-page Count"
        );
        assert!(s.matches("/Type /Page ").count() >= 2);
    }

    #[test]
    fn pdf_empty_document_is_valid_single_page() {
        let mut gen = SeqIdGen::default();
        let doc_id = DocumentId::new(&mut gen);
        let root = SectionId::new(&mut gen);
        let actor = ActorId::new(&mut gen);
        let state = CanonicalState {
            document_id: doc_id,
            package_format_version: Version::V1_0_0,
            op_seq: OpSeq::ZERO,
            document: Document::empty(doc_id, root, actor, Timestamp { unix_ms: 0 }),
            entities: EntityTables::default(),
        };
        let pdf = export_pdf(&state, &SourceTable::default(), &ExportConfig::default()).unwrap();
        assert!(pdf.starts_with(b"%PDF-1.4"));
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.contains("/Count 1"));
    }
}
