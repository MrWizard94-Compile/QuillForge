//! Deterministic export pipelines.
//!
//! Derived only — never mutates journal/kernel. Citation markers follow Zotero-inspired
//! locator fidelity (see `qf-citation`).

mod docx;
mod docx_import;
mod pdf;

/// Layout-aligned PDF helpers (print fidelity with `qf_layout`).
pub use pdf::{export_pdf_with_assets, pdf_layout_page_count};
/// DOCX import intermediate blocks (apply via journal in app/storage).
pub use docx_import::{import_docx_blocks, import_docx_path, DocxImportBlock};

use qf_blocks::{BlockBody, Mark, Span};
use qf_citation::{format_source_marker, SourceKind, SourceRef, SourceTable};
use qf_core::{Hash256, SourceId};
use qf_kernel::CanonicalState;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Html,
    Pdf,
    PlainText,
    Docx,
}

#[derive(Clone, Debug, Default)]
pub struct ExportConfig {
    pub include_timestamps: bool,
    /// When true, append a bibliography section from document entity sources.
    pub include_bibliography: bool,
}

#[derive(Clone, Debug)]
pub struct ExportRequest {
    pub format: ExportFormat,
    pub config: ExportConfig,
}

impl Default for ExportRequest {
    fn default() -> Self {
        Self {
            format: ExportFormat::Markdown,
            config: ExportConfig {
                include_timestamps: false,
                include_bibliography: true,
            },
        }
    }
}

#[derive(Clone, Debug)]
pub struct ExportOutput {
    pub bytes: Vec<u8>,
    pub content_hash: Hash256,
}

pub trait Exporter: Send + Sync {
    fn export(&self, state: &CanonicalState, req: &ExportRequest) -> Result<ExportOutput, String>;
}

/// Build a source table from kernel entity payloads (opaque CBOR/JSON or locator UTF-8).
pub fn sources_from_state(state: &CanonicalState) -> SourceTable {
    let mut table = SourceTable::default();
    for (id, payload) in &state.entities.sources {
        if let Some(src) = decode_source_payload(*id, payload) {
            // Prefer validated insert; fall back unchecked so export never drops known entities.
            if table.insert(src.clone()).is_err() {
                table.insert_unchecked(src);
            }
        }
    }
    table
}

fn decode_source_payload(id: SourceId, payload: &[u8]) -> Option<SourceRef> {
    // v1 wire format: UTF-8 "kind|locator" e.g. "url|https://example.com"
    let s = std::str::from_utf8(payload).ok()?;
    let (kind_s, locator) = s.split_once('|')?;
    let kind = match kind_s {
        "url" => SourceKind::Url,
        "doi" => SourceKind::Doi,
        "isbn" => SourceKind::Isbn,
        "local" => SourceKind::Local,
        _ => SourceKind::Other,
    };
    Some(SourceRef {
        id,
        kind,
        locator: locator.to_string(),
        meta: Default::default(),
    })
}

/// Encode a source for entity table storage.
pub fn encode_source_payload(source: &SourceRef) -> Vec<u8> {
    let kind = match source.kind {
        SourceKind::Url => "url",
        SourceKind::Doi => "doi",
        SourceKind::Isbn => "isbn",
        SourceKind::Local => "local",
        SourceKind::Other => "other",
    };
    format!("{kind}|{}", source.locator).into_bytes()
}

pub struct CoreExporter;

impl Exporter for CoreExporter {
    fn export(&self, state: &CanonicalState, req: &ExportRequest) -> Result<ExportOutput, String> {
        let sources = sources_from_state(state);
        let bytes = match req.format {
            ExportFormat::Markdown => {
                export_markdown(state, &sources, &req.config).into_bytes()
            }
            ExportFormat::PlainText => export_plain(state, &sources, &req.config).into_bytes(),
            ExportFormat::Html => export_html(state, &sources, &req.config).into_bytes(),
            ExportFormat::Pdf => pdf::export_pdf(state, &sources, &req.config)?,
            ExportFormat::Docx => docx::export_docx(state, &sources, &req.config)?,
        };
        let content_hash = Hash256::sha256(&bytes);
        Ok(ExportOutput {
            bytes,
            content_hash,
        })
    }
}

fn export_markdown(state: &CanonicalState, sources: &SourceTable, cfg: &ExportConfig) -> String {
    let mut out = String::new();
    let mut cited: BTreeSet<SourceId> = BTreeSet::new();

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
                    out.push_str(&"#".repeat(h.level as usize));
                    out.push(' ');
                    out.push_str(&spans_markdown(&h.spans, sources, &mut cited));
                    out.push('\n');
                }
                BlockBody::Paragraph(p) => {
                    out.push_str(&spans_markdown(&p.spans, sources, &mut cited));
                    out.push('\n');
                }
                BlockBody::Quote(q) => {
                    out.push_str("> ");
                    out.push_str(&spans_markdown(&q.spans, sources, &mut cited));
                    if let Some(cid) = q.cite {
                        cited.insert(cid);
                        if let Some(src) = sources.get(cid) {
                            out.push_str(" — ");
                            out.push_str(&format_source_marker(src));
                        }
                    }
                    out.push('\n');
                }
                BlockBody::Citation(c) => {
                    for id in &c.source_ids {
                        cited.insert(*id);
                    }
                    let markers: Vec<String> = c
                        .source_ids
                        .iter()
                        .filter_map(|id| sources.get(*id).map(format_source_marker))
                        .collect();
                    let body = spans_markdown(&c.spans, sources, &mut cited);
                    if body.is_empty() {
                        out.push_str(&format!("[{}]\n", markers.join("; ")));
                    } else {
                        out.push_str(&format!("{body} [{}]\n", markers.join("; ")));
                    }
                }
                BlockBody::Code(c) => {
                    out.push_str("```");
                    if let Some(lang) = &c.language {
                        out.push_str(lang);
                    }
                    out.push('\n');
                    out.push_str(&c.text);
                    if !c.text.ends_with('\n') {
                        out.push('\n');
                    }
                    out.push_str("```\n");
                }
                BlockBody::Unknown(u) => {
                    out.push_str(&format!(
                        "\n<!-- qf-unknown type=\"{}\" payload-b64=\"{}\" -->\n",
                        u.original_type,
                        b64(&u.original_payload)
                    ));
                }
                BlockBody::PageBreak => out.push_str("\n---\n"),
                BlockBody::SectionBreak => out.push_str("\n\n"),
                _ => {
                    if let Some(t) = state.document.project_text(*bid) {
                        out.push_str(&t);
                        out.push('\n');
                    }
                }
            }
        }
    }

    if cfg.include_bibliography && !cited.is_empty() {
        out.push_str("\n## References\n\n");
        for id in cited {
            if let Some(src) = sources.get(id) {
                out.push_str(&format!("- {}\n", format_source_marker(src)));
            } else {
                out.push_str(&format!("- [missing-source:{}]\n", id.as_uuid()));
            }
        }
    }
    out
}

fn export_plain(state: &CanonicalState, sources: &SourceTable, cfg: &ExportConfig) -> String {
    let md = export_markdown(
        state,
        sources,
        &ExportConfig {
            include_timestamps: cfg.include_timestamps,
            include_bibliography: cfg.include_bibliography,
        },
    );
    // Strip simple markdown markers for plain text.
    md.replace("```", "")
        .lines()
        .map(|l| l.trim_start_matches(['#', '>', ' ']).to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

// note: plain span strip uses single replace below

fn export_html(state: &CanonicalState, sources: &SourceTable, cfg: &ExportConfig) -> String {
    let mut out = String::from("<!DOCTYPE html><html><body>\n");
    let mut cited: BTreeSet<SourceId> = BTreeSet::new();

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
                    let t = spans_plain(&h.spans, sources, &mut cited);
                    out.push_str(&format!(
                        "<h{level}>{body}</h{level}>\n",
                        level = h.level.clamp(1, 6),
                        body = html_escape(&t)
                    ));
                }
                BlockBody::Paragraph(p) => {
                    let t = spans_html(&p.spans, sources, &mut cited);
                    out.push_str(&format!("<p>{t}</p>\n"));
                }
                BlockBody::Citation(c) => {
                    for id in &c.source_ids {
                        cited.insert(*id);
                    }
                    let body = spans_html(&c.spans, sources, &mut cited);
                    let mut cites = String::new();
                    for id in &c.source_ids {
                        if let Some(src) = sources.get(*id) {
                            cites.push_str(&format!(
                                "<cite data-qf-source=\"{}\">{}</cite> ",
                                html_escape(&src.locator),
                                html_escape(&format_source_marker(src))
                            ));
                        }
                    }
                    out.push_str(&format!("<p class=\"qf-citation\">{body} {cites}</p>\n"));
                }
                BlockBody::Unknown(u) => {
                    out.push_str(&format!(
                        "<div data-qf-unknown-type=\"{}\" data-qf-payload-b64=\"{}\"></div>\n",
                        html_escape(&u.original_type),
                        b64(&u.original_payload)
                    ));
                }
                _ => {
                    if let Some(t) = state.document.project_text(*bid) {
                        out.push_str(&format!("<p>{}</p>\n", html_escape(&t)));
                    }
                }
            }
        }
    }

    if cfg.include_bibliography && !cited.is_empty() {
        out.push_str("<h2>References</h2>\n<ul>\n");
        for id in cited {
            if let Some(src) = sources.get(id) {
                out.push_str(&format!(
                    "<li data-qf-source-id=\"{}\">{}</li>\n",
                    id.as_uuid(),
                    html_escape(&format_source_marker(src))
                ));
            }
        }
        out.push_str("</ul>\n");
    }
    out.push_str("</body></html>\n");
    out
}

fn spans_markdown(spans: &[Span], sources: &SourceTable, cited: &mut BTreeSet<SourceId>) -> String {
    let mut out = String::new();
    for sp in spans {
        let mut t = sp.text.clone();
        for m in &sp.marks {
            match m {
                Mark::Bold => t = format!("**{t}**"),
                Mark::Italic => t = format!("*{t}*"),
                Mark::Code => t = format!("`{t}`"),
                Mark::Link { href, .. } => t = format!("[{t}]({href})"),
                Mark::CitationRef { source_id } => {
                    cited.insert(*source_id);
                    if let Some(src) = sources.get(*source_id) {
                        t.push_str(&format!("[{}]", format_source_marker(src)));
                    } else {
                        t.push_str(&format!("[cite:{}]", source_id.as_uuid()));
                    }
                }
                _ => {}
            }
        }
        out.push_str(&t);
    }
    out
}

fn spans_plain(spans: &[Span], sources: &SourceTable, cited: &mut BTreeSet<SourceId>) -> String {
    spans_markdown(spans, sources, cited)
        .replace("**", "")
        .replace(['*', '`'], "")
}

fn spans_html(spans: &[Span], sources: &SourceTable, cited: &mut BTreeSet<SourceId>) -> String {
    let mut out = String::new();
    for sp in spans {
        let mut t = html_escape(&sp.text);
        for m in &sp.marks {
            match m {
                Mark::Bold => t = format!("<strong>{t}</strong>"),
                Mark::Italic => t = format!("<em>{t}</em>"),
                Mark::Code => t = format!("<code>{t}</code>"),
                Mark::Link { href, .. } => {
                    t = format!("<a href=\"{}\">{t}</a>", html_escape(href));
                }
                Mark::CitationRef { source_id } => {
                    cited.insert(*source_id);
                    if let Some(src) = sources.get(*source_id) {
                        t.push_str(&format!(
                            "<sup class=\"qf-cite\" data-qf-source=\"{}\">{}</sup>",
                            html_escape(&src.locator),
                            html_escape(&format_source_marker(src))
                        ));
                    }
                }
                _ => {}
            }
        }
        out.push_str(&t);
    }
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn b64(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (a << 16) | (b << 8) | c;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Collect citation source ids referenced in the document (blocks + marks).
pub fn collect_cited_ids(state: &CanonicalState) -> BTreeSet<SourceId> {
    let mut cited = BTreeSet::new();
    for block in state.document.blocks.values() {
        match &block.body {
            BlockBody::Citation(c) => {
                cited.extend(c.source_ids.iter().copied());
            }
            BlockBody::Quote(q) => {
                if let Some(id) = q.cite {
                    cited.insert(id);
                }
            }
            BlockBody::Paragraph(p) => collect_mark_cites(&p.spans, &mut cited),
            BlockBody::Heading(h) => collect_mark_cites(&h.spans, &mut cited),
            _ => {}
        }
    }
    cited
}

fn collect_mark_cites(spans: &[Span], cited: &mut BTreeSet<SourceId>) {
    for sp in spans {
        for m in &sp.marks {
            if let Mark::CitationRef { source_id } = m {
                cited.insert(*source_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_blocks::{BlockBody, BlockMetadata, CitationBlock, ParagraphBlock, Span};
    use qf_core::{ActorId, BlockId, DocumentId, OpSeq, SectionId, SeqIdGen, Timestamp, Version};
    use qf_doc::{Block, Document};
    use qf_kernel::{CanonicalState, EntityTables};

    fn base_state(gen: &mut SeqIdGen) -> CanonicalState {
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

    #[test]
    fn markdown_exports_citation_and_bibliography() {
        let mut gen = SeqIdGen::default();
        let mut state = base_state(&mut gen);
        let src_id = SourceId::new(&mut gen);
        let src = SourceRef {
            id: src_id,
            kind: SourceKind::Doi,
            locator: "10.1000/test".into(),
            meta: Default::default(),
        };
        state
            .entities
            .sources
            .insert(src_id, encode_source_payload(&src));

        let bid = BlockId::new(&mut gen);
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
                body: BlockBody::Citation(CitationBlock {
                    source_ids: vec![src_id],
                    style_hint: None,
                    spans: vec![Span {
                        id: qf_core::SpanId(uuid::Uuid::nil()),
                        text: "See study".into(),
                        marks: vec![],
                    }],
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
            .push(bid);

        let out = CoreExporter
            .export(
                &state,
                &ExportRequest {
                    format: ExportFormat::Markdown,
                    config: ExportConfig {
                        include_bibliography: true,
                        ..Default::default()
                    },
                },
            )
            .unwrap();
        let s = String::from_utf8(out.bytes).unwrap();
        assert!(s.contains("See study"));
        assert!(s.contains("10.1000/test") || s.contains("doi:"));
        assert!(s.contains("## References"));
        // Deterministic
        let out2 = CoreExporter
            .export(
                &state,
                &ExportRequest {
                    format: ExportFormat::Markdown,
                    config: ExportConfig {
                        include_bibliography: true,
                        ..Default::default()
                    },
                },
            )
            .unwrap();
        assert_eq!(out.content_hash, out2.content_hash);
    }

    #[test]
    fn html_emits_cite_markers() {
        let mut gen = SeqIdGen::default();
        let mut state = base_state(&mut gen);
        let src_id = SourceId::new(&mut gen);
        state.entities.sources.insert(
            src_id,
            encode_source_payload(&SourceRef {
                id: src_id,
                kind: SourceKind::Url,
                locator: "https://example.com/a".into(),
                meta: Default::default(),
            }),
        );
        let bid = BlockId::new(&mut gen);
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
                body: BlockBody::Paragraph(ParagraphBlock {
                    spans: vec![Span {
                        id: qf_core::SpanId(uuid::Uuid::nil()),
                        text: "claim".into(),
                        marks: vec![Mark::CitationRef { source_id: src_id }],
                    }],
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
            .push(bid);

        let out = CoreExporter
            .export(
                &state,
                &ExportRequest {
                    format: ExportFormat::Html,
                    config: ExportConfig {
                        include_bibliography: true,
                        ..Default::default()
                    },
                },
            )
            .unwrap();
        let s = String::from_utf8(out.bytes).unwrap();
        assert!(s.contains("qf-cite") || s.contains("data-qf-source"));
        assert!(s.contains("example.com"));
    }
}
