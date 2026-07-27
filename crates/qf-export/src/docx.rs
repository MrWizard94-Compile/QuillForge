//! Minimal deterministic OOXML (.docx) writer.
//!
//! Produces a valid WordprocessingML package (ZIP) with fixed entry order and
//! fixed timestamps so identical document input yields identical bytes.

use crate::{export_plain, ExportConfig};
use qf_citation::SourceTable;
use qf_kernel::CanonicalState;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

/// Build a minimal .docx (Office Open XML) package from plain export text.
pub fn export_docx(
    state: &CanonicalState,
    sources: &SourceTable,
    cfg: &ExportConfig,
) -> Result<Vec<u8>, String> {
    let plain = export_plain(state, sources, cfg);
    let lines: Vec<&str> = plain.lines().collect();
    build_docx_lines(&lines)
}

fn build_docx_lines(lines: &[&str]) -> Result<Vec<u8>, String> {
    let document_xml = document_xml(lines);
    let content_types = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>
"#;
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
"#;
    let doc_rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>
"#;

    // Fixed DOS timestamp for determinism (2020-01-01 00:00:00).
    let fixed_time = DateTime::from_date_and_time(2020, 1, 1, 0, 0, 0)
        .map_err(|e| format!("docx datetime: {e}"))?;
    let opts = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(fixed_time);

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        // Stable entry order is part of the byte contract.
        write_entry(&mut zip, "[Content_Types].xml", content_types.as_bytes(), opts)?;
        write_entry(&mut zip, "_rels/.rels", rels.as_bytes(), opts)?;
        write_entry(
            &mut zip,
            "word/document.xml",
            document_xml.as_bytes(),
            opts,
        )?;
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

fn document_xml(lines: &[&str]) -> String {
    let mut body = String::new();
    if lines.is_empty() {
        body.push_str("<w:p><w:r><w:t></w:t></w:r></w:p>");
    } else {
        for line in lines {
            body.push_str("<w:p><w:r><w:t xml:space=\"preserve\">");
            body.push_str(&xml_escape(line));
            body.push_str("</w:t></w:r></w:p>");
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
    use qf_blocks::{BlockBody, BlockMetadata, ParagraphBlock, Span};
    use qf_core::{
        ActorId, BlockId, DocumentId, OpSeq, SectionId, SeqIdGen, Timestamp, Version,
    };
    use qf_doc::{Block, Document};
    use qf_kernel::{CanonicalState, EntityTables};
    use std::io::Read;
    use zip::ZipArchive;

    fn sample_state(text: &str) -> CanonicalState {
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
                        text: text.into(),
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
        state
    }

    #[test]
    fn docx_is_zip_with_document_and_deterministic() {
        let state = sample_state("Hello DOCX");
        let sources = SourceTable::default();
        let a = export_docx(&state, &sources, &ExportConfig::default()).unwrap();
        let b = export_docx(&state, &sources, &ExportConfig::default()).unwrap();
        assert_eq!(a, b);
        // ZIP local file header signature
        assert_eq!(&a[0..4], b"PK\x03\x04");

        let mut archive = ZipArchive::new(Cursor::new(a)).unwrap();
        let mut doc = archive.by_name("word/document.xml").unwrap();
        let mut xml = String::new();
        doc.read_to_string(&mut xml).unwrap();
        assert!(xml.contains("Hello DOCX"));
        assert!(xml.contains("w:document"));
    }

    #[test]
    fn docx_escapes_xml_specials() {
        let state = sample_state("A & B <C>");
        let sources = SourceTable::default();
        let bytes = export_docx(&state, &sources, &ExportConfig::default()).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut doc = archive.by_name("word/document.xml").unwrap();
        let mut xml = String::new();
        doc.read_to_string(&mut xml).unwrap();
        assert!(xml.contains("A &amp; B &lt;C&gt;"));
    }
}
