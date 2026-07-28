//! Structured deterministic OOXML (.docx) writer.
//!
//! Walks canonical blocks (headings, paragraphs, quotes, code, breaks, unknown)
//! into WordprocessingML with outline levels. Fixed ZIP order/timestamps → byte-stable.

use crate::ExportConfig;
use qf_blocks::{BlockBody, Mark, Span};
use qf_citation::{format_source_marker, SourceTable};
use qf_kernel::CanonicalState;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

/// Build a structured .docx from canonical state (not plain-text flattening).
pub fn export_docx(
    state: &CanonicalState,
    sources: &SourceTable,
    _cfg: &ExportConfig,
) -> Result<Vec<u8>, String> {
    let paragraphs = collect_paragraphs(state, sources);
    build_docx_paragraphs(&paragraphs)
}

#[derive(Clone, Debug)]
enum DocxPara {
    Heading { level: u8, text: String },
    Body { text: String },
    Quote { text: String },
    Code { text: String },
    PageBreak,
}

fn collect_paragraphs(state: &CanonicalState, sources: &SourceTable) -> Vec<DocxPara> {
    let mut out = Vec::new();
    for sid in &state.document.section_order {
        let Some(sec) = state.document.sections.get(sid) else {
            continue;
        };
        for bid in &sec.blocks {
            let Some(block) = state.document.blocks.get(bid) else {
                continue;
            };
            match &block.body {
                BlockBody::Heading(h) => {
                    out.push(DocxPara::Heading {
                        level: h.level.clamp(1, 6),
                        text: spans_plain(&h.spans, sources),
                    });
                }
                BlockBody::Paragraph(p) => {
                    out.push(DocxPara::Body {
                        text: spans_plain(&p.spans, sources),
                    });
                }
                BlockBody::Quote(q) => {
                    let mut t = spans_plain(&q.spans, sources);
                    if let Some(cid) = q.cite {
                        if let Some(src) = sources.get(cid) {
                            t.push_str(" — ");
                            t.push_str(&format_source_marker(src));
                        }
                    }
                    out.push(DocxPara::Quote { text: t });
                }
                BlockBody::Citation(c) => {
                    let mut t = spans_plain(&c.spans, sources);
                    let markers: Vec<String> = c
                        .source_ids
                        .iter()
                        .filter_map(|id| sources.get(*id).map(format_source_marker))
                        .collect();
                    if !markers.is_empty() {
                        if !t.is_empty() {
                            t.push(' ');
                        }
                        t.push('[');
                        t.push_str(&markers.join("; "));
                        t.push(']');
                    }
                    out.push(DocxPara::Body { text: t });
                }
                BlockBody::Code(c) => {
                    out.push(DocxPara::Code {
                        text: c.text.clone(),
                    });
                }
                BlockBody::Unknown(u) => {
                    out.push(DocxPara::Body {
                        text: format!(
                            "[unknown block type={} bytes={}]",
                            u.original_type,
                            u.original_payload.len()
                        ),
                    });
                }
                BlockBody::PageBreak => out.push(DocxPara::PageBreak),
                BlockBody::SectionBreak => out.push(DocxPara::Body {
                    text: String::new(),
                }),
                BlockBody::List(l) => push_list_items(&mut out, &l.items, sources, l.ordered, 0),
                BlockBody::Checklist(c) => {
                    for item in &c.items {
                        let mark = if item.checked { "[x]" } else { "[ ]" };
                        out.push(DocxPara::Body {
                            text: format!("{mark} {}", spans_plain(&item.spans, sources)),
                        });
                    }
                }
                BlockBody::Table(t) => {
                    for row in &t.rows {
                        let cells: Vec<String> = row
                            .cells
                            .iter()
                            .map(|cell| format!("[cell {}x{}]", cell.colspan, cell.rowspan))
                            .collect();
                        out.push(DocxPara::Body {
                            text: cells.join(" | "),
                        });
                    }
                }
                BlockBody::Callout(c) => {
                    let title = spans_plain(&c.title, sources);
                    out.push(DocxPara::Quote {
                        text: if title.is_empty() {
                            "[callout]".into()
                        } else {
                            title
                        },
                    });
                }
                BlockBody::Image(i) => {
                    out.push(DocxPara::Body {
                        text: format!("[image: {}]", i.alt),
                    });
                }
                BlockBody::Video(v) => {
                    out.push(DocxPara::Body {
                        text: format!("[video: {}]", v.alt),
                    });
                }
                BlockBody::Equation(e) => {
                    out.push(DocxPara::Code {
                        text: e.latex.clone(),
                    });
                }
                BlockBody::Diagram(d) => {
                    out.push(DocxPara::Code {
                        text: d.source.clone(),
                    });
                }
                BlockBody::Embed(e) => {
                    out.push(DocxPara::Body {
                        text: format!("[embed {} {}]", e.provider, e.reference),
                    });
                }
            }
        }
    }
    out
}

fn push_list_items(
    out: &mut Vec<DocxPara>,
    items: &[qf_blocks::ListItem],
    sources: &SourceTable,
    ordered: bool,
    depth: usize,
) {
    for (i, item) in items.iter().enumerate() {
        let pad = "  ".repeat(depth);
        let bullet = if ordered {
            format!("{}.", i + 1)
        } else {
            "•".into()
        };
        out.push(DocxPara::Body {
            text: format!("{pad}{bullet} {}", spans_plain(&item.spans, sources)),
        });
        if !item.children.is_empty() {
            push_list_items(out, &item.children, sources, ordered, depth + 1);
        }
    }
}

fn spans_plain(spans: &[Span], sources: &SourceTable) -> String {
    let mut s = String::new();
    for sp in spans {
        s.push_str(&sp.text);
        for m in &sp.marks {
            if let Mark::CitationRef { source_id } = m {
                if let Some(src) = sources.get(*source_id) {
                    s.push(' ');
                    s.push_str(&format_source_marker(src));
                }
            }
        }
    }
    s
}

fn build_docx_paragraphs(paras: &[DocxPara]) -> Result<Vec<u8>, String> {
    let document_xml = document_xml(paras);
    let styles_xml = styles_xml();
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
</Types>
"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
"#;
    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>
"#;

    let fixed_time = DateTime::from_date_and_time(2020, 1, 1, 0, 0, 0)
        .map_err(|e| format!("docx datetime: {e}"))?;
    let opts = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(fixed_time);

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        write_entry(&mut zip, "[Content_Types].xml", content_types.as_bytes(), opts)?;
        write_entry(&mut zip, "_rels/.rels", rels.as_bytes(), opts)?;
        write_entry(
            &mut zip,
            "word/document.xml",
            document_xml.as_bytes(),
            opts,
        )?;
        write_entry(&mut zip, "word/styles.xml", styles_xml.as_bytes(), opts)?;
        write_entry(
            &mut zip,
            "word/_rels/document.xml.rels",
            doc_rels.as_bytes(),
            opts,
        )?;
        zip.finish()
            .map_err(|e| format!("docx zip finish: {e}"))?;
    }
    Ok(cursor.into_inner())
}

fn write_entry(
    zip: &mut ZipWriter<&mut Cursor<Vec<u8>>>,
    name: &str,
    data: &[u8],
    opts: SimpleFileOptions,
) -> Result<(), String> {
    zip.start_file(name, opts)
        .map_err(|e| format!("docx start {name}: {e}"))?;
    zip.write_all(data)
        .map_err(|e| format!("docx write {name}: {e}"))?;
    Ok(())
}

fn styles_xml() -> String {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Normal" w:default="1">
    <w:name w:val="Normal"/>
    <w:qFormat/>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading1">
    <w:name w:val="heading 1"/>
    <w:basedOn w:val="Normal"/>
    <w:qFormat/>
    <w:pPr><w:outlineLvl w:val="0"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="32"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading2">
    <w:name w:val="heading 2"/>
    <w:basedOn w:val="Normal"/>
    <w:qFormat/>
    <w:pPr><w:outlineLvl w:val="1"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="28"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Heading3">
    <w:name w:val="heading 3"/>
    <w:basedOn w:val="Normal"/>
    <w:qFormat/>
    <w:pPr><w:outlineLvl w:val="2"/></w:pPr>
    <w:rPr><w:b/><w:sz w:val="24"/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Quote">
    <w:name w:val="Quote"/>
    <w:basedOn w:val="Normal"/>
    <w:pPr><w:ind w:left="720"/></w:pPr>
    <w:rPr><w:i/></w:rPr>
  </w:style>
  <w:style w:type="paragraph" w:styleId="Code">
    <w:name w:val="Code"/>
    <w:basedOn w:val="Normal"/>
    <w:rPr><w:rFonts w:ascii="Courier New" w:hAnsi="Courier New"/><w:sz w:val="20"/></w:rPr>
  </w:style>
</w:styles>
"#
    .to_string()
}

fn document_xml(paras: &[DocxPara]) -> String {
    let mut body = String::new();
    if paras.is_empty() {
        body.push_str(&para_xml("Normal", "", false));
    } else {
        for p in paras {
            match p {
                DocxPara::Heading { level, text } => {
                    let style = match level {
                        1 => "Heading1",
                        2 => "Heading2",
                        _ => "Heading3",
                    };
                    body.push_str(&para_xml(style, text, false));
                }
                DocxPara::Body { text } => body.push_str(&para_xml("Normal", text, false)),
                DocxPara::Quote { text } => body.push_str(&para_xml("Quote", text, false)),
                DocxPara::Code { text } => body.push_str(&para_xml("Code", text, true)),
                DocxPara::PageBreak => {
                    body.push_str(
                        r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#,
                    );
                }
            }
        }
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    {body}
    <w:sectPr>
      <w:pgSz w:w="12240" w:h="15840"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>
"#
    )
}

fn para_xml(style: &str, text: &str, preserve_newlines: bool) -> String {
    if preserve_newlines && text.contains('\n') {
        let mut parts = String::new();
        for (i, line) in text.lines().enumerate() {
            if i > 0 {
                parts.push_str("<w:br/>");
            }
            parts.push_str(&format!(
                r#"<w:t xml:space="preserve">{}</w:t>"#,
                xml_escape(line)
            ));
        }
        format!(
            r#"<w:p><w:pPr><w:pStyle w:val="{style}"/></w:pPr><w:r>{parts}</w:r></w:p>"#
        )
    } else {
        format!(
            r#"<w:p><w:pPr><w:pStyle w:val="{style}"/></w:pPr><w:r><w:t xml:space="preserve">{}</w:t></w:r></w:p>"#,
            xml_escape(text)
        )
    }
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if c.is_control() && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_blocks::{
        BlockBody, BlockMetadata, CodeBlock, HeadingBlock, ParagraphBlock, QuoteBlock, Span,
    };
    use qf_core::{
        ActorId, BlockId, DocumentId, OpSeq, SectionId, SeqIdGen, Timestamp, Version,
    };
    use qf_doc::{Block, Document};
    use qf_kernel::{CanonicalState, EntityTables};
    use std::io::Read;
    use zip::ZipArchive;

    fn empty_state(gen: &mut SeqIdGen) -> CanonicalState {
        let doc_id = DocumentId::new(gen);
        let root = SectionId::new(gen);
        let actor = ActorId::new(gen);
        CanonicalState {
            document_id: doc_id,
            package_format_version: Version::V1_0_0,
            op_seq: OpSeq(1),
            document: Document::empty(doc_id, root, actor, Timestamp { unix_ms: 0 }),
            entities: EntityTables::default(),
        }
    }

    fn push(
        state: &mut CanonicalState,
        gen: &mut SeqIdGen,
        body: BlockBody,
    ) {
        let bid = BlockId::new(gen);
        let actor = state.document.meta.authors[0];
        state.document.blocks.insert(
            bid,
            Block {
                id: bid,
                meta: BlockMetadata {
                    created_by: actor,
                    created_at_seq: OpSeq(1),
                    attrs: Default::default(),
                },
                body,
            },
        );
        let root = state.document.root_section_id;
        state.document.sections.get_mut(&root).unwrap().blocks.push(bid);
    }

    fn span(text: &str) -> Span {
        Span {
            id: qf_core::SpanId(uuid::Uuid::nil()),
            text: text.into(),
            marks: vec![],
        }
    }

    #[test]
    fn docx_is_zip_with_document_and_deterministic() {
        let mut gen = SeqIdGen::default();
        let mut state = empty_state(&mut gen);
        push(
            &mut state,
            &mut gen,
            BlockBody::Paragraph(ParagraphBlock {
                spans: vec![span("Hello DOCX")],
            }),
        );
        let sources = SourceTable::default();
        let a = export_docx(&state, &sources, &ExportConfig::default()).unwrap();
        let b = export_docx(&state, &sources, &ExportConfig::default()).unwrap();
        assert_eq!(a, b);
        assert_eq!(&a[0..4], b"PK\x03\x04");

        let mut archive = ZipArchive::new(Cursor::new(a)).unwrap();
        let mut doc = archive.by_name("word/document.xml").unwrap();
        let mut xml = String::new();
        doc.read_to_string(&mut xml).unwrap();
        assert!(xml.contains("Hello DOCX"));
        assert!(xml.contains("w:document"));
        drop(doc);
        assert!(archive.by_name("word/styles.xml").is_ok());
    }

    #[test]
    fn docx_escapes_xml_specials() {
        let mut gen = SeqIdGen::default();
        let mut state = empty_state(&mut gen);
        push(
            &mut state,
            &mut gen,
            BlockBody::Paragraph(ParagraphBlock {
                spans: vec![span("A & B <C>")],
            }),
        );
        let sources = SourceTable::default();
        let bytes = export_docx(&state, &sources, &ExportConfig::default()).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut doc = archive.by_name("word/document.xml").unwrap();
        let mut xml = String::new();
        doc.read_to_string(&mut xml).unwrap();
        assert!(xml.contains("A &amp; B &lt;C&gt;"));
    }

    #[test]
    fn docx_uses_heading_styles_from_ast() {
        let mut gen = SeqIdGen::default();
        let mut state = empty_state(&mut gen);
        push(
            &mut state,
            &mut gen,
            BlockBody::Heading(HeadingBlock {
                level: 1,
                spans: vec![span("Title One")],
            }),
        );
        push(
            &mut state,
            &mut gen,
            BlockBody::Paragraph(ParagraphBlock {
                spans: vec![span("Body text")],
            }),
        );
        push(
            &mut state,
            &mut gen,
            BlockBody::Quote(QuoteBlock {
                spans: vec![span("Quoted")],
                cite: None,
            }),
        );
        push(
            &mut state,
            &mut gen,
            BlockBody::Code(CodeBlock {
                language: Some("rs".into()),
                text: "fn main() {}\n".into(),
                show_line_numbers: false,
            }),
        );
        let xml = {
            let bytes =
                export_docx(&state, &SourceTable::default(), &ExportConfig::default()).unwrap();
            let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
            let mut doc = archive.by_name("word/document.xml").unwrap();
            let mut s = String::new();
            doc.read_to_string(&mut s).unwrap();
            s
        };
        assert!(xml.contains(r#"w:pStyle w:val="Heading1""#));
        assert!(xml.contains("Title One"));
        assert!(xml.contains(r#"w:pStyle w:val="Quote""#));
        assert!(xml.contains(r#"w:pStyle w:val="Code""#));
        assert!(xml.contains("fn main()"));
    }
}
