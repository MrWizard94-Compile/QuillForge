//! Structured deterministic OOXML (.docx) writer.
//!
//! Walks canonical blocks (headings, paragraphs, quotes, code, breaks, unknown)
//! into WordprocessingML with outline levels. Fixed ZIP order/timestamps → byte-stable.
//!
//! Image blocks embed as DrawingML inline pictures when package asset bytes are
//! supplied (`export_docx_with_assets`); otherwise a caption line is emitted.
//! Optional [`FontPack`] embeds TrueType under `word/fonts/` + `fontTable.xml`.

use crate::fonts::FontPack;
use crate::ExportConfig;
use qf_blocks::{BlockBody, Mark, Span};
use qf_citation::{format_source_marker, SourceTable};
use qf_kernel::CanonicalState;
use std::collections::BTreeMap;
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

/// EMUs (English Metric Units) per pixel at 96 DPI: 914400 / 96.
const EMU_PER_PX: i64 = 9525;
/// Cap display width at 6.5 inches (US Letter content width-ish).
const MAX_CX_EMU: i64 = 6_501_600;

/// Build a structured .docx from canonical state (not plain-text flattening).
pub(crate) fn export_docx(
    state: &CanonicalState,
    sources: &SourceTable,
    cfg: &ExportConfig,
) -> Result<Vec<u8>, String> {
    export_docx_with_assets(state, sources, cfg, None)
}

/// DOCX export with optional package asset map (UUID string → bytes) for DrawingML images.
pub fn export_docx_with_assets(
    state: &CanonicalState,
    sources: &SourceTable,
    cfg: &ExportConfig,
    assets: Option<&BTreeMap<String, Vec<u8>>>,
) -> Result<Vec<u8>, String> {
    export_docx_with_assets_and_font(state, sources, cfg, assets, None)
}

/// DOCX export with optional images and optional embedded TrueType [`FontPack`].
pub fn export_docx_with_assets_and_font(
    state: &CanonicalState,
    sources: &SourceTable,
    _cfg: &ExportConfig,
    assets: Option<&BTreeMap<String, Vec<u8>>>,
    font: Option<&FontPack>,
) -> Result<Vec<u8>, String> {
    let paragraphs = collect_paragraphs(state, sources);
    build_docx_paragraphs(&paragraphs, assets, font)
}

#[derive(Clone, Debug)]
enum DocxPara {
    Heading {
        level: u8,
        text: String,
    },
    Body {
        text: String,
    },
    Quote {
        text: String,
    },
    Code {
        text: String,
    },
    /// Grid of cell plain text (row-major).
    Table {
        rows: Vec<Vec<String>>,
    },
    Image {
        asset_key: String,
        alt: String,
        width_px: u32,
        height_px: u32,
    },
    PageBreak,
}

/// Media part planned for the ZIP package.
struct DocxMedia {
    /// Package asset key (UUID string) this part was resolved from.
    asset_key: String,
    /// Relationship id, e.g. rId2
    rel_id: String,
    /// Path inside package, e.g. word/media/image1.png
    part_path: String,
    /// Target relative to word/, e.g. media/image1.png
    rel_target: String,
    /// Content type extension: png | jpeg
    ext: &'static str,
    bytes: Vec<u8>,
    alt: String,
    width_px: u32,
    height_px: u32,
    /// DrawingML docPr id (1-based unique).
    doc_pr_id: u32,
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
                    let rows: Vec<Vec<String>> = t
                        .rows
                        .iter()
                        .map(|row| {
                            row.cells
                                .iter()
                                .map(|cell| {
                                    let text: String = cell
                                        .blocks
                                        .iter()
                                        .filter_map(|bid| state.document.project_text(*bid))
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    if text.is_empty() {
                                        format!("[cell {}x{}]", cell.colspan, cell.rowspan)
                                    } else {
                                        text
                                    }
                                })
                                .collect()
                        })
                        .collect();
                    out.push(DocxPara::Table { rows });
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
                    out.push(DocxPara::Image {
                        asset_key: i.asset_id.as_uuid().to_string(),
                        alt: i.alt.clone(),
                        width_px: i.width_px.unwrap_or(0),
                        height_px: i.height_px.unwrap_or(0),
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

fn build_docx_paragraphs(
    paras: &[DocxPara],
    assets: Option<&BTreeMap<String, Vec<u8>>>,
    font: Option<&FontPack>,
) -> Result<Vec<u8>, String> {
    let embed_font = font.filter(|f| !f.ttf_bytes.is_empty());
    // rId1 = styles; optional rId2 = fontTable; then images.
    let font_table_rid = if embed_font.is_some() {
        Some("rId2")
    } else {
        None
    };
    let image_rid_base = if embed_font.is_some() { 3u32 } else { 2u32 };
    let media = collect_media(paras, assets, image_rid_base);
    let family = embed_font.map(|f| f.family.as_str()).unwrap_or("Calibri");
    let document_xml = document_xml(paras, &media);
    let styles_xml = styles_xml_with_font(family);
    let content_types = content_types_xml(&media, embed_font.is_some());
    let rels = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>
"#;
    let doc_rels = document_rels_xml(&media, font_table_rid);

    let fixed_time = DateTime::from_date_and_time(2020, 1, 1, 0, 0, 0)
        .map_err(|e| format!("docx datetime: {e}"))?;
    let opts = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .last_modified_time(fixed_time);

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        write_entry(
            &mut zip,
            "[Content_Types].xml",
            content_types.as_bytes(),
            opts,
        )?;
        write_entry(&mut zip, "_rels/.rels", rels.as_bytes(), opts)?;
        write_entry(&mut zip, "word/document.xml", document_xml.as_bytes(), opts)?;
        write_entry(&mut zip, "word/styles.xml", styles_xml.as_bytes(), opts)?;
        write_entry(
            &mut zip,
            "word/_rels/document.xml.rels",
            doc_rels.as_bytes(),
            opts,
        )?;
        if let Some(pack) = embed_font {
            let font_file = font_file_name(pack);
            let font_table = font_table_xml(&pack.family);
            let font_table_rels = font_table_rels_xml(&font_file);
            write_entry(&mut zip, "word/fontTable.xml", font_table.as_bytes(), opts)?;
            write_entry(
                &mut zip,
                "word/_rels/fontTable.xml.rels",
                font_table_rels.as_bytes(),
                opts,
            )?;
            write_entry(
                &mut zip,
                &format!("word/fonts/{font_file}"),
                &pack.ttf_bytes,
                opts,
            )?;
        }
        for m in &media {
            write_entry(&mut zip, &m.part_path, &m.bytes, opts)?;
        }
        zip.finish().map_err(|e| format!("docx zip finish: {e}"))?;
    }
    Ok(cursor.into_inner())
}

fn font_file_name(pack: &FontPack) -> String {
    let base: String = pack
        .postscript_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let base = if base.is_empty() {
        "EmbeddedFont".into()
    } else {
        base
    };
    format!("{base}.ttf")
}

fn font_table_xml(family: &str) -> String {
    let fam = xml_escape(family);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:fonts xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <w:font w:name="{fam}">
    <w:panose1 w:val="020B0604020202020204"/>
    <w:charset w:val="00"/>
    <w:family w:val="modern"/>
    <w:pitch w:val="variable"/>
    <w:embedRegular r:id="rId1"/>
  </w:font>
</w:fonts>
"#
    )
}

fn font_table_rels_xml(font_file: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/font" Target="fonts/{font_file}"/>
</Relationships>
"#
    )
}

fn collect_media(
    paras: &[DocxPara],
    assets: Option<&BTreeMap<String, Vec<u8>>>,
    image_rid_base: u32,
) -> Vec<DocxMedia> {
    let Some(assets) = assets else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut img_n = 0u32;
    for p in paras {
        let DocxPara::Image {
            asset_key,
            alt,
            width_px,
            height_px,
        } = p
        else {
            continue;
        };
        let Some(bytes) = assets.get(asset_key) else {
            continue;
        };
        let Some((ext, nat_w, nat_h)) = sniff_image(bytes) else {
            continue;
        };
        img_n += 1;
        let w = if *width_px > 0 { *width_px } else { nat_w };
        let h = if *height_px > 0 { *height_px } else { nat_h };
        let file = format!("image{img_n}.{ext}");
        // rId base: 2 (styles only) or 3 (styles + fontTable).
        let rel_id = format!("rId{}", image_rid_base + img_n - 1);
        out.push(DocxMedia {
            asset_key: asset_key.clone(),
            rel_id,
            part_path: format!("word/media/{file}"),
            rel_target: format!("media/{file}"),
            ext,
            bytes: bytes.clone(),
            alt: alt.clone(),
            width_px: w.max(1),
            height_px: h.max(1),
            doc_pr_id: img_n,
        });
    }
    out
}

/// Detect PNG/JPEG and natural pixel size.
fn sniff_image(bytes: &[u8]) -> Option<(&'static str, u32, u32)> {
    if bytes.len() >= 24 && bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        // IHDR width/height at offset 16
        let w = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let h = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        if w > 0 && h > 0 {
            return Some(("png", w, h));
        }
    }
    if bytes.len() > 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        // Decode dims via image crate (JPEG SOF scan is finicky without full parser).
        if let Ok(img) = image::load_from_memory(bytes) {
            let w = img.width();
            let h = img.height();
            if w > 0 && h > 0 {
                return Some(("jpeg", w, h));
            }
        }
    }
    None
}

fn content_types_xml(media: &[DocxMedia], has_font: bool) -> String {
    let mut s = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
"#,
    );
    let mut has_png = false;
    let mut has_jpeg = false;
    for m in media {
        match m.ext {
            "png" => has_png = true,
            "jpeg" => has_jpeg = true,
            _ => {}
        }
    }
    if has_png {
        s.push_str(
            r#"  <Default Extension="png" ContentType="image/png"/>
"#,
        );
    }
    if has_jpeg {
        s.push_str(
            r#"  <Default Extension="jpeg" ContentType="image/jpeg"/>
"#,
        );
    }
    if has_font {
        s.push_str(
            r#"  <Default Extension="ttf" ContentType="application/x-font-ttf"/>
"#,
        );
    }
    s.push_str(
        r#"  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/word/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml"/>
"#,
    );
    if has_font {
        s.push_str(
            r#"  <Override PartName="/word/fontTable.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.fontTable+xml"/>
"#,
        );
    }
    s.push_str("</Types>\n");
    s
}

fn document_rels_xml(media: &[DocxMedia], font_table_rid: Option<&str>) -> String {
    let mut s = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
"#,
    );
    if let Some(rid) = font_table_rid {
        s.push_str(&format!(
            r#"  <Relationship Id="{rid}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/fontTable" Target="fontTable.xml"/>
"#
        ));
    }
    for m in media {
        s.push_str(&format!(
            r#"  <Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="{}"/>
"#,
            m.rel_id, m.rel_target
        ));
    }
    s.push_str("</Relationships>\n");
    s
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

/// Styles with an explicit body font family (embedded via fontTable when pack supplied).
fn styles_xml_with_font(family: &str) -> String {
    let fam = xml_escape(family);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:style w:type="paragraph" w:styleId="Normal" w:default="1">
    <w:name w:val="Normal"/>
    <w:qFormat/>
    <w:rPr><w:rFonts w:ascii="{fam}" w:hAnsi="{fam}" w:cs="{fam}"/></w:rPr>
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
    )
}

fn document_xml(paras: &[DocxPara], media: &[DocxMedia]) -> String {
    let mut body = String::new();
    let mut next_media = 0usize;
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
                DocxPara::Table { rows } => body.push_str(&table_xml(rows)),
                DocxPara::Image {
                    asset_key,
                    alt,
                    width_px,
                    height_px,
                } => {
                    // Lockstep with collect_media: same order, only embeddable images are listed.
                    let take = media
                        .get(next_media)
                        .is_some_and(|m| m.asset_key == *asset_key);
                    if take {
                        let m = &media[next_media];
                        body.push_str(&drawing_para_xml(m));
                        if !m.alt.is_empty() {
                            body.push_str(&para_xml("Normal", &m.alt, false));
                        }
                        next_media += 1;
                    } else {
                        let label = if *width_px > 0 && *height_px > 0 {
                            format!("[image: {alt} {width_px}×{height_px}]")
                        } else {
                            format!("[image: {alt}]")
                        };
                        body.push_str(&para_xml("Normal", &label, false));
                    }
                }
                DocxPara::PageBreak => {
                    body.push_str(r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#);
                }
            }
        }
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture">
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

fn drawing_para_xml(m: &DocxMedia) -> String {
    let mut cx = i64::from(m.width_px) * EMU_PER_PX;
    let mut cy = i64::from(m.height_px) * EMU_PER_PX;
    if cx > MAX_CX_EMU {
        let scale = MAX_CX_EMU as f64 / cx as f64;
        cx = MAX_CX_EMU;
        cy = ((cy as f64) * scale).round() as i64;
    }
    cx = cx.max(1);
    cy = cy.max(1);
    let name = format!("Picture {}", m.doc_pr_id);
    let descr = xml_escape(&m.alt);
    let name_esc = xml_escape(&name);
    format!(
        r#"<w:p><w:r><w:drawing><wp:inline distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{cx}" cy="{cy}"/><wp:docPr id="{id}" name="{name_esc}" descr="{descr}"/><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic><pic:nvPicPr><pic:cNvPr id="0" name="{name_esc}" descr="{descr}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip r:embed="{rel}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#,
        cx = cx,
        cy = cy,
        id = m.doc_pr_id,
        name_esc = name_esc,
        descr = descr,
        rel = m.rel_id,
    )
}

fn table_xml(rows: &[Vec<String>]) -> String {
    if rows.is_empty() {
        return para_xml("Normal", "[table]", false);
    }
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(1).max(1);
    let mut xml = String::from(r#"<w:tbl><w:tblPr><w:tblW w:w="0" w:type="auto"/></w:tblPr>"#);
    xml.push_str("<w:tblGrid>");
    for _ in 0..cols {
        xml.push_str(r#"<w:gridCol w:w="2400"/>"#);
    }
    xml.push_str("</w:tblGrid>");
    for row in rows {
        xml.push_str("<w:tr>");
        for c in 0..cols {
            let text = row.get(c).map(|s| s.as_str()).unwrap_or("");
            xml.push_str(&format!(
                r#"<w:tc><w:tcPr><w:tcW w:w="2400" w:type="dxa"/></w:tcPr>{inner}</w:tc>"#,
                inner = para_xml("Normal", text, false)
            ));
        }
        xml.push_str("</w:tr>");
    }
    xml.push_str("</w:tbl>");
    xml
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
        format!(r#"<w:p><w:pPr><w:pStyle w:val="{style}"/></w:pPr><w:r>{parts}</w:r></w:p>"#)
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
    use qf_core::{ActorId, BlockId, DocumentId, OpSeq, SectionId, SeqIdGen, Timestamp, Version};
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

    fn push(state: &mut CanonicalState, gen: &mut SeqIdGen, body: BlockBody) {
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
        state
            .document
            .sections
            .get_mut(&root)
            .unwrap()
            .blocks
            .push(bid);
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
    fn docx_emits_real_table_markup() {
        use qf_blocks::{TableBlock, TableCell, TableRow};
        let mut gen = SeqIdGen::default();
        let mut state = empty_state(&mut gen);
        let cell_a = BlockId::new(&mut gen);
        let cell_b = BlockId::new(&mut gen);
        let actor = state.document.meta.authors[0];
        let meta = BlockMetadata {
            created_by: actor,
            created_at_seq: OpSeq(1),
            attrs: Default::default(),
        };
        state.document.blocks.insert(
            cell_a,
            Block {
                id: cell_a,
                meta: meta.clone(),
                body: BlockBody::Paragraph(ParagraphBlock {
                    spans: vec![span("Alpha")],
                }),
            },
        );
        state.document.blocks.insert(
            cell_b,
            Block {
                id: cell_b,
                meta: meta.clone(),
                body: BlockBody::Paragraph(ParagraphBlock {
                    spans: vec![span("Beta")],
                }),
            },
        );
        let tid = BlockId::new(&mut gen);
        state.document.blocks.insert(
            tid,
            Block {
                id: tid,
                meta,
                body: BlockBody::Table(TableBlock {
                    column_count: 2,
                    rows: vec![TableRow {
                        cells: vec![
                            TableCell {
                                id: BlockId::new(&mut gen),
                                colspan: 1,
                                rowspan: 1,
                                blocks: vec![cell_a],
                            },
                            TableCell {
                                id: BlockId::new(&mut gen),
                                colspan: 1,
                                rowspan: 1,
                                blocks: vec![cell_b],
                            },
                        ],
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
            .push(tid);
        // cell paragraphs also need to be in map only (not section order) — ok
        let bytes = export_docx(&state, &SourceTable::default(), &ExportConfig::default()).unwrap();
        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        let mut doc = archive.by_name("word/document.xml").unwrap();
        let mut xml = String::new();
        doc.read_to_string(&mut xml).unwrap();
        assert!(xml.contains("<w:tbl>"), "missing table: {xml}");
        assert!(xml.contains("Alpha") && xml.contains("Beta"), "{xml}");
    }

    fn tiny_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(4, 3, image::Rgba([32, 200, 64, 255]));
        let mut cur = Cursor::new(Vec::new());
        img.write_to(&mut cur, image::ImageFormat::Png).unwrap();
        cur.into_inner()
    }

    #[test]
    fn docx_embeds_drawingml_image_from_assets() {
        use qf_blocks::ImageBlock;
        let mut gen = SeqIdGen::default();
        let mut state = empty_state(&mut gen);
        let asset_id = qf_core::AssetId::new(&mut gen);
        push(
            &mut state,
            &mut gen,
            BlockBody::Image(ImageBlock {
                asset_id,
                alt: "Demo mark".into(),
                caption: vec![],
                width_px: Some(4),
                height_px: Some(3),
            }),
        );
        push(
            &mut state,
            &mut gen,
            BlockBody::Paragraph(ParagraphBlock {
                spans: vec![span("After image")],
            }),
        );
        let mut assets = BTreeMap::new();
        assets.insert(asset_id.as_uuid().to_string(), tiny_png());
        let bytes = export_docx_with_assets(
            &state,
            &SourceTable::default(),
            &ExportConfig::default(),
            Some(&assets),
        )
        .unwrap();
        let bytes2 = export_docx_with_assets(
            &state,
            &SourceTable::default(),
            &ExportConfig::default(),
            Some(&assets),
        )
        .unwrap();
        assert_eq!(bytes, bytes2, "docx+media must be deterministic");

        let mut archive = ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert!(
            archive.by_name("word/media/image1.png").is_ok(),
            "missing media part"
        );
        let mut doc = archive.by_name("word/document.xml").unwrap();
        let mut xml = String::new();
        doc.read_to_string(&mut xml).unwrap();
        assert!(xml.contains("w:drawing"), "missing DrawingML: {xml}");
        assert!(xml.contains("a:blip"), "missing blip: {xml}");
        assert!(xml.contains("r:embed=\"rId2\""), "missing image rel: {xml}");
        assert!(xml.contains("Demo mark"), "missing alt: {xml}");
        drop(doc);
        let mut rels = archive.by_name("word/_rels/document.xml.rels").unwrap();
        let mut rels_xml = String::new();
        rels.read_to_string(&mut rels_xml).unwrap();
        assert!(
            rels_xml.contains("relationships/image") && rels_xml.contains("media/image1.png"),
            "{rels_xml}"
        );
    }

    #[test]
    fn docx_embeds_truetype_font_table() {
        use crate::fonts::quill_mono_v1;
        let mut gen = SeqIdGen::default();
        let mut state = empty_state(&mut gen);
        push(
            &mut state,
            &mut gen,
            BlockBody::Paragraph(ParagraphBlock {
                spans: vec![span("Font embed body")],
            }),
        );
        let pack = quill_mono_v1();
        let a = export_docx_with_assets_and_font(
            &state,
            &SourceTable::default(),
            &ExportConfig::default(),
            None,
            Some(&pack),
        )
        .unwrap();
        let b = export_docx_with_assets_and_font(
            &state,
            &SourceTable::default(),
            &ExportConfig::default(),
            None,
            Some(&pack),
        )
        .unwrap();
        assert_eq!(a, b, "docx+font must be deterministic");

        let mut archive = ZipArchive::new(Cursor::new(a)).unwrap();
        assert!(
            archive.by_name("word/fontTable.xml").is_ok(),
            "missing fontTable"
        );
        assert!(
            archive.by_name("word/fonts/QuillForgeMono.ttf").is_ok()
                || archive
                    .file_names()
                    .any(|n| n.starts_with("word/fonts/") && n.ends_with(".ttf")),
            "missing word/fonts/*.ttf"
        );
        let mut styles = String::new();
        archive
            .by_name("word/styles.xml")
            .unwrap()
            .read_to_string(&mut styles)
            .unwrap();
        assert!(
            styles.contains("QuillForge Mono") || styles.contains("QuillForge"),
            "styles should name embedded family: {styles}"
        );
        let mut ft = String::new();
        archive
            .by_name("word/fontTable.xml")
            .unwrap()
            .read_to_string(&mut ft)
            .unwrap();
        assert!(ft.contains("embedRegular"), "{ft}");
        assert!(ft.contains("r:id=\"rId1\""), "{ft}");
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
