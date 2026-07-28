//! Import OOXML (.docx) into an intermediate block list for journaled apply.
//!
//! Parser is intentionally dependency-light (ZIP + WordprocessingML scan).
//! Supports Quillforge-exported styles and common Word paragraph styles
//! (Heading1–3, Normal, Quote, Code, page breaks). Unknown runs become body text.

use std::io::{Cursor, Read};
use zip::ZipArchive;

/// One imported structural unit ready for Command → Journal → Kernel apply.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DocxImportBlock {
    Heading { level: u8, text: String },
    Paragraph { text: String },
    Quote { text: String },
    Code { text: String },
    PageBreak,
}

/// Parse a .docx package into import blocks (no kernel mutation).
pub fn import_docx_blocks(bytes: &[u8]) -> Result<Vec<DocxImportBlock>, String> {
    let mut archive =
        ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("docx zip open: {e}"))?;
    let mut entry = archive
        .by_name("word/document.xml")
        .map_err(|e| format!("docx missing word/document.xml: {e}"))?;
    let mut xml = String::new();
    entry
        .read_to_string(&mut xml)
        .map_err(|e| format!("docx read document.xml: {e}"))?;
    Ok(parse_document_xml(&xml))
}

/// Load path and parse.
pub fn import_docx_path(path: &std::path::Path) -> Result<Vec<DocxImportBlock>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    import_docx_blocks(&bytes)
}

fn parse_document_xml(xml: &str) -> Vec<DocxImportBlock> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(start) = find_open_tag(rest, "w:p") {
        let after_start = &rest[start..];
        let Some(end_rel) = find_close_tag(after_start, "w:p") else {
            break;
        };
        let para = &after_start[..end_rel];
        rest = &after_start[end_rel..];
        // Skip sectPr-only noise: paragraphs inside body only.
        if para.contains("w:sectPr") && !para.contains("w:t") && !para.contains("w:br") {
            continue;
        }
        if let Some(block) = parse_paragraph(para) {
            out.push(block);
        }
    }
    if out.is_empty() {
        out.push(DocxImportBlock::Paragraph {
            text: String::new(),
        });
    }
    out
}

/// Find `<w:p` or `<w:p>` or `<w:p ...>` start index in haystack.
fn find_open_tag(hay: &str, local: &str) -> Option<usize> {
    let needle = format!("<{local}");
    let mut search = hay;
    let mut base = 0usize;
    while let Some(i) = search.find(&needle) {
        let abs = base + i;
        let after = &hay[abs + needle.len()..];
        if after.starts_with('>') || after.starts_with(' ') || after.starts_with('/') {
            return Some(abs);
        }
        // e.g. w:pPr / w:pgSz — keep searching
        base = abs + needle.len();
        search = &hay[base..];
    }
    None
}

fn find_close_tag(hay: &str, local: &str) -> Option<usize> {
    let close = format!("</{local}>");
    hay.find(&close).map(|i| i + close.len())
}

fn parse_paragraph(para_xml: &str) -> Option<DocxImportBlock> {
    if para_xml.contains(r#"w:type="page""#) || para_xml.contains("w:type='page'") {
        return Some(DocxImportBlock::PageBreak);
    }
    let style = paragraph_style(para_xml);
    let text = collect_text(para_xml);
    match style.as_deref() {
        Some("Heading1") | Some("heading 1") | Some("Title") => Some(DocxImportBlock::Heading {
            level: 1,
            text,
        }),
        Some("Heading2") | Some("heading 2") => Some(DocxImportBlock::Heading {
            level: 2,
            text,
        }),
        Some("Heading3") | Some("heading 3") => Some(DocxImportBlock::Heading {
            level: 3,
            text,
        }),
        Some("Heading4") | Some("heading 4") => Some(DocxImportBlock::Heading {
            level: 4,
            text,
        }),
        Some("Heading5") | Some("heading 5") => Some(DocxImportBlock::Heading {
            level: 5,
            text,
        }),
        Some("Heading6") | Some("heading 6") => Some(DocxImportBlock::Heading {
            level: 6,
            text,
        }),
        Some("Quote") | Some("IntenseQuote") | Some("BlockText") => {
            Some(DocxImportBlock::Quote { text })
        }
        Some("Code") | Some("HTMLPreformatted") | Some("SourceCode") => {
            Some(DocxImportBlock::Code { text })
        }
        _ => {
            // outlineLvl fallback (0-based → heading level)
            if let Some(lvl) = outline_level(para_xml) {
                return Some(DocxImportBlock::Heading {
                    level: (lvl + 1).min(6),
                    text,
                });
            }
            Some(DocxImportBlock::Paragraph { text })
        }
    }
}

fn paragraph_style(para_xml: &str) -> Option<String> {
    // <w:pStyle w:val="Heading1"/>
    let key = "w:pStyle";
    let idx = para_xml.find(key)?;
    let slice = &para_xml[idx..];
    attr_val(slice, "w:val").or_else(|| attr_val(slice, "val"))
}

fn outline_level(para_xml: &str) -> Option<u8> {
    let key = "w:outlineLvl";
    let idx = para_xml.find(key)?;
    let slice = &para_xml[idx..];
    let v = attr_val(slice, "w:val").or_else(|| attr_val(slice, "val"))?;
    v.parse().ok()
}

fn attr_val(slice: &str, name: &str) -> Option<String> {
    let pat = format!("{name}=\"");
    if let Some(i) = slice.find(&pat) {
        let rest = &slice[i + pat.len()..];
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }
    let pat2 = format!("{name}='");
    if let Some(i) = slice.find(&pat2) {
        let rest = &slice[i + pat2.len()..];
        let end = rest.find('\'')?;
        return Some(rest[..end].to_string());
    }
    None
}

fn collect_text(para_xml: &str) -> String {
    let mut out = String::new();
    let mut rest = para_xml;
    loop {
        let Some(t_start) = rest.find("<w:t") else {
            break;
        };
        let after_tag = &rest[t_start + 4..];
        let Some(gt) = after_tag.find('>') else {
            break;
        };
        let tag_body = &after_tag[..gt];
        if tag_body.ends_with('/') {
            rest = &after_tag[gt + 1..];
            continue;
        }
        let content = &after_tag[gt + 1..];
        let Some(close) = content.find("</w:t>") else {
            break;
        };
        out.push_str(&xml_unescape(&content[..close]));
        rest = &content[close + 6..];
        // Soft line breaks between text runs → newline (code blocks).
        if let Some(br_at) = rest.find("<w:br") {
            let next_t = rest.find("<w:t").unwrap_or(usize::MAX);
            if br_at < next_t {
                out.push('\n');
            }
        }
    }
    out
}

fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::docx::export_docx;
    use crate::ExportConfig;
    use qf_blocks::{
        BlockBody, BlockMetadata, CodeBlock, HeadingBlock, ParagraphBlock, QuoteBlock, Span,
    };
    use qf_citation::SourceTable;
    use qf_core::{
        ActorId, BlockId, DocumentId, OpSeq, SectionId, SeqIdGen, Timestamp, Version,
    };
    use qf_doc::{Block, Document};
    use qf_kernel::{CanonicalState, EntityTables};

    fn span(text: &str) -> Span {
        Span {
            id: qf_core::SpanId(uuid::Uuid::nil()),
            text: text.into(),
            marks: vec![],
        }
    }

    fn state_mixed() -> CanonicalState {
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
        let meta = BlockMetadata {
            created_by: actor,
            created_at_seq: OpSeq(1),
            attrs: Default::default(),
        };
        let blocks = [
            BlockBody::Heading(HeadingBlock {
                level: 1,
                spans: vec![span("Title")],
            }),
            BlockBody::Paragraph(ParagraphBlock {
                spans: vec![span("Hello & body")],
            }),
            BlockBody::Quote(QuoteBlock {
                spans: vec![span("Quoted")],
                cite: None,
            }),
            BlockBody::Code(CodeBlock {
                language: None,
                text: "fn x() {}".into(),
                show_line_numbers: false,
            }),
            BlockBody::PageBreak,
            BlockBody::Paragraph(ParagraphBlock {
                spans: vec![span("After break")],
            }),
        ];
        for body in blocks {
            let bid = BlockId::new(&mut gen);
            state.document.blocks.insert(
                bid,
                Block {
                    id: bid,
                    meta: meta.clone(),
                    body,
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
    fn import_round_trips_quillforge_export() {
        let state = state_mixed();
        let bytes = export_docx(&state, &SourceTable::default(), &ExportConfig::default()).unwrap();
        let blocks = import_docx_blocks(&bytes).unwrap();
        assert!(
            blocks.iter().any(|b| matches!(
                b,
                DocxImportBlock::Heading {
                    level: 1,
                    text
                } if text == "Title"
            )),
            "expected H1 Title, got {blocks:?}"
        );
        assert!(blocks.iter().any(|b| matches!(
            b,
            DocxImportBlock::Paragraph { text } if text == "Hello & body"
        )));
        assert!(blocks
            .iter()
            .any(|b| matches!(b, DocxImportBlock::Quote { text } if text == "Quoted")));
        assert!(blocks
            .iter()
            .any(|b| matches!(b, DocxImportBlock::Code { text } if text.contains("fn x"))));
        assert!(blocks
            .iter()
            .any(|b| matches!(b, DocxImportBlock::PageBreak)));
        assert!(blocks.iter().any(|b| matches!(
            b,
            DocxImportBlock::Paragraph { text } if text == "After break"
        )));
    }

    #[test]
    fn parse_minimal_handwritten_xml() {
        let xml = r#"<?xml version="1.0"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:pPr><w:pStyle w:val="Heading2"/></w:pPr><w:r><w:t>Sub</w:t></w:r></w:p>
    <w:p><w:r><w:t xml:space="preserve">Plain</w:t></w:r></w:p>
  </w:body>
</w:document>"#;
        let blocks = parse_document_xml(xml);
        assert_eq!(
            blocks[0],
            DocxImportBlock::Heading {
                level: 2,
                text: "Sub".into()
            }
        );
        assert_eq!(
            blocks[1],
            DocxImportBlock::Paragraph {
                text: "Plain".into()
            }
        );
    }
}
