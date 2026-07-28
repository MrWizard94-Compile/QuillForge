//! Deterministic multi-page PDF 1.4 writer aligned with the layout engine.
//!
//! Pagination follows [`qf_layout::LayoutEngine`] (US Letter, same config as the
//! editor) so screen page count and PDF `/Count` match. Fixed Helvetica, no
//! external fonts — same inputs → same bytes.

use crate::{export_plain, ExportConfig};
use qf_citation::SourceTable;
use qf_core::PreservedValue;
use qf_kernel::CanonicalState;
use qf_layout::{LayoutConfig, LayoutEngine};

/// Max lines exported into a single PDF page content stream (safety cap).
const MAX_LINES_PER_PAGE: usize = 80;
/// Meta keys shared with the Word-class shell page chrome.
const META_HEADER: &str = "qf.page.header";
const META_FOOTER: &str = "qf.page.footer";

/// Build a multi-page PDF from layout-tree pages (print fidelity).
pub fn export_pdf(
    state: &CanonicalState,
    sources: &SourceTable,
    cfg: &ExportConfig,
) -> Result<Vec<u8>, String> {
    let layout_cfg = LayoutConfig::default();
    let tree = LayoutEngine::new().full_layout(state, &layout_cfg);
    let mut page_lines = tree.page_plain_lines(state);

    // Bibliography (if any) appends to the last page as plain lines so export
    // remains layout-primary without inventing phantom pages.
    if cfg.include_bibliography {
        let plain = export_plain(state, sources, cfg);
        if let Some(idx) = plain.find("\n## References\n") {
            let bib: Vec<String> = plain[idx..]
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
            if !bib.is_empty() {
                if page_lines.is_empty() {
                    page_lines.push(bib);
                } else if let Some(last) = page_lines.last_mut() {
                    last.extend(bib);
                }
            }
        }
    }

    if page_lines.is_empty() {
        page_lines.push(Vec::new());
    }

    let header = meta_text(state, META_HEADER);
    let footer = meta_text(state, META_FOOTER);

    // Cap lines drawn per page (layout already owns pagination — do not invent pages).
    // Reserve room for header/footer chrome lines when present.
    let reserve = usize::from(header.is_some()) + usize::from(footer.is_some());
    let body_cap = MAX_LINES_PER_PAGE.saturating_sub(reserve).max(1);
    let capped: Vec<Vec<String>> = page_lines
        .into_iter()
        .map(|page| {
            let mut lines: Vec<String> = Vec::new();
            if let Some(h) = &header {
                lines.push(h.clone());
            }
            lines.extend(page.into_iter().take(body_cap));
            if let Some(f) = &footer {
                lines.push(f.clone());
            }
            lines
        })
        .collect();

    build_pdf_pages(&capped)
}

fn meta_text(state: &CanonicalState, key: &str) -> Option<String> {
    match state.document.meta.custom.get(key)? {
        PreservedValue::Text(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// PDF page count for a state using the same layout config as the editor.
pub fn pdf_layout_page_count(state: &CanonicalState) -> u32 {
    let tree = LayoutEngine::new().full_layout(state, &LayoutConfig::default());
    tree.page_count()
}

fn build_pdf_pages(pages: &[Vec<String>]) -> Result<Vec<u8>, String> {
    let page_count = pages.len().max(1);

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

    for (i, chunk) in pages.iter().enumerate() {
        let page_obj = 4 + i * 2;
        let content_obj = 5 + i * 2;
        numbered.push((
            page_obj,
            format!(
                "{page_obj} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {content_obj} 0 R /Resources << /Font << /F1 3 0 R >> >> >>\nendobj\n"
            )
            .into_bytes(),
        ));
        let line_refs: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
        let stream = page_content_stream(&line_refs);
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
    use qf_layout::{LayoutConfig, LayoutEngine};

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
    fn pdf_page_count_matches_layout_engine() {
        // ~30 short paragraphs fill one US Letter body with default line metrics;
        // 50 forces a second layout page.
        let state = state_with_paragraphs(50, "Line");
        let layout_pages = LayoutEngine::new()
            .full_layout(&state, &LayoutConfig::default())
            .page_count();
        assert!(layout_pages >= 2, "fixture must multi-page layout");
        let sources = SourceTable::default();
        let pdf = export_pdf(
            &state,
            &sources,
            &ExportConfig {
                include_bibliography: false,
                ..ExportConfig::default()
            },
        )
        .unwrap();
        let s = String::from_utf8_lossy(&pdf);
        assert!(
            s.contains(&format!("/Count {layout_pages}")),
            "PDF Count must equal layout page_count={layout_pages}, got: {s}"
        );
        assert_eq!(pdf_layout_page_count(&state), layout_pages);
        assert!(s.matches("/Type /Page ").count() as u32 >= layout_pages);
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
        assert_eq!(pdf_layout_page_count(&state), 1);
    }
}
