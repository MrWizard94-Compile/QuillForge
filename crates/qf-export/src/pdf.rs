//! Deterministic multi-page PDF 1.4 writer aligned with the layout engine.
//!
//! Pagination follows [`qf_layout::LayoutEngine`] (US Letter, same config as the
//! editor) so screen page count and PDF `/Count` match. Fonts: Helvetica Type1 by
//! default, or embedded TrueType via [`crate::FontPack`] (`/FontFile2`). Optional
//! DeviceRGB image XObjects from package assets (PNG decode).

use crate::fonts::FontPack;
use crate::{export_plain, ExportConfig};
use qf_blocks::BlockBody;
use qf_citation::SourceTable;
use qf_core::PreservedValue;
use qf_kernel::CanonicalState;
use qf_layout::{fixed_to_pt, LayoutConfig, LayoutEngine, PAGE_HEIGHT_PT, PAGE_WIDTH_PT};
use std::collections::BTreeMap;

/// Max lines exported into a single PDF page content stream (safety cap).
const MAX_LINES_PER_PAGE: usize = 80;
/// Meta keys shared with the Word-class shell page chrome.
const META_HEADER: &str = "qf.page.header";
const META_FOOTER: &str = "qf.page.footer";
/// Cap decoded image edge for foundation export budget.
const MAX_IMAGE_EDGE: u32 = 240;

/// Build a multi-page PDF from layout-tree pages (print fidelity).
pub fn export_pdf(
    state: &CanonicalState,
    sources: &SourceTable,
    cfg: &ExportConfig,
) -> Result<Vec<u8>, String> {
    export_pdf_with_assets(state, sources, cfg, None)
}

/// PDF export with optional package asset map (UUID string → bytes) for images.
pub fn export_pdf_with_assets(
    state: &CanonicalState,
    sources: &SourceTable,
    cfg: &ExportConfig,
    assets: Option<&BTreeMap<String, Vec<u8>>>,
) -> Result<Vec<u8>, String> {
    export_pdf_with_assets_and_font(state, sources, cfg, assets, None)
}

/// PDF export with optional images and optional embedded TrueType [`FontPack`].
///
/// When `font` is `None`, uses standard Helvetica (Type1). When `Some`, embeds
/// the pack's TTF via `/FontFile2` (deterministic for fixed pack bytes).
pub fn export_pdf_with_assets_and_font(
    state: &CanonicalState,
    sources: &SourceTable,
    cfg: &ExportConfig,
    assets: Option<&BTreeMap<String, Vec<u8>>>,
    font: Option<&FontPack>,
) -> Result<Vec<u8>, String> {
    let layout_cfg = LayoutConfig::default();
    let tree = LayoutEngine::new().full_layout(state, &layout_cfg);
    let mut page_lines = tree.page_plain_lines(state);

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

    // Per-page image draws from layout Media frames + asset RGB.
    let page_images = collect_page_images(state, &tree, assets);
    build_pdf_pages(&capped, &page_images, font)
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

#[derive(Clone, Debug)]
struct PdfImage {
    /// Resource name, e.g. Im0
    name: String,
    width: u32,
    height: u32,
    /// Raw RGB8 bytes length = w*h*3
    rgb: Vec<u8>,
    /// PDF user-space bottom-left placement (points).
    x_pt: f32,
    y_pt: f32,
    draw_w_pt: f32,
    draw_h_pt: f32,
}

fn collect_page_images(
    state: &CanonicalState,
    tree: &qf_layout::LayoutTree,
    assets: Option<&BTreeMap<String, Vec<u8>>>,
) -> Vec<Vec<PdfImage>> {
    let Some(assets) = assets else {
        return tree.pages.iter().map(|_| Vec::new()).collect();
    };
    let mut out = Vec::with_capacity(tree.pages.len());
    let mut img_i = 0usize;
    for page in &tree.pages {
        let mut imgs = Vec::new();
        for frame in &page.frames {
            let Some(block) = state.document.blocks.get(&frame.block_id) else {
                continue;
            };
            let BlockBody::Image(img) = &block.body else {
                continue;
            };
            let key = img.asset_id.as_uuid().to_string();
            let Some(bytes) = assets.get(&key) else {
                continue;
            };
            let Some((w, h, rgb)) = decode_rgb_capped(bytes) else {
                continue;
            };
            // Layout y is top-down in points; PDF y is bottom-up.
            let x_pt = fixed_to_pt(frame.bounds.x);
            let top_pt = fixed_to_pt(frame.bounds.y);
            let frame_h = fixed_to_pt(frame.bounds.h).max(24.0);
            let frame_w = fixed_to_pt(frame.bounds.w).max(24.0);
            // Fit image into frame width, preserve aspect.
            let aspect = h as f32 / w as f32;
            let draw_w = frame_w.min(200.0);
            let draw_h = (draw_w * aspect).min(frame_h).max(12.0);
            let y_pt = (PAGE_HEIGHT_PT - top_pt - draw_h).max(0.0);
            imgs.push(PdfImage {
                name: format!("Im{img_i}"),
                width: w,
                height: h,
                rgb,
                x_pt,
                y_pt,
                draw_w_pt: draw_w,
                draw_h_pt: draw_h,
            });
            img_i += 1;
        }
        out.push(imgs);
    }
    let _ = PAGE_WIDTH_PT;
    out
}

fn decode_rgb_capped(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory(bytes).ok()?;
    let mut rgba = img.to_rgba8();
    let (mut w, mut h) = rgba.dimensions();
    if w == 0 || h == 0 {
        return None;
    }
    if w > MAX_IMAGE_EDGE || h > MAX_IMAGE_EDGE {
        let scale = (MAX_IMAGE_EDGE as f32 / w.max(h) as f32).min(1.0);
        let nw = ((w as f32) * scale).round().max(1.0) as u32;
        let nh = ((h as f32) * scale).round().max(1.0) as u32;
        rgba = image::imageops::resize(&rgba, nw, nh, image::imageops::FilterType::Triangle);
        w = nw;
        h = nh;
    }
    let mut rgb = Vec::with_capacity((w * h * 3) as usize);
    for p in rgba.pixels() {
        rgb.push(p[0]);
        rgb.push(p[1]);
        rgb.push(p[2]);
    }
    Some((w, h, rgb))
}

fn build_pdf_pages(
    pages: &[Vec<String>],
    page_images: &[Vec<PdfImage>],
    font: Option<&FontPack>,
) -> Result<Vec<u8>, String> {
    let page_count = pages.len().max(1);

    // Flatten images for global XObject objects.
    let all_images: Vec<&PdfImage> = page_images.iter().flatten().collect();
    let n_img = all_images.len();

    // Objects:
    // 1 Catalog, 2 Pages, 3 Font,
    // [4 FontDescriptor, 5 FontFile2] when embedding TTF
    // then Image XObjects, then for page i: Page, Content
    let embed = font.filter(|f| !f.ttf_bytes.is_empty());
    let font_extra = if embed.is_some() { 2usize } else { 0 };
    let first_img_obj = 4 + font_extra;
    let first_page_obj = first_img_obj + n_img;

    let mut numbered: Vec<(usize, Vec<u8>)> = Vec::new();

    numbered.push((
        1,
        b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n".to_vec(),
    ));

    let mut kids = String::new();
    for i in 0..page_count {
        let page_obj = first_page_obj + i * 2;
        kids.push_str(&format!("{page_obj} 0 R "));
    }
    numbered.push((
        2,
        format!("2 0 obj\n<< /Type /Pages /Kids [{kids}] /Count {page_count} >>\nendobj\n")
            .into_bytes(),
    ));

    if let Some(pack) = embed {
        push_embedded_font_objects(&mut numbered, pack)?;
    } else {
        numbered.push((
            3,
            b"3 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n".to_vec(),
        ));
    }

    for (i, img) in all_images.iter().enumerate() {
        let obj = first_img_obj + i;
        let mut body = format!(
            "{obj} 0 obj\n<< /Type /XObject /Subtype /Image /Width {} /Height {} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {} >>\nstream\n",
            img.width,
            img.height,
            img.rgb.len()
        )
        .into_bytes();
        body.extend_from_slice(&img.rgb);
        body.extend_from_slice(b"\nendstream\nendobj\n");
        numbered.push((obj, body));
    }

    for i in 0..page_count {
        let page_obj = first_page_obj + i * 2;
        let content_obj = page_obj + 1;
        let imgs = page_images.get(i).map(|v| v.as_slice()).unwrap_or(&[]);

        let mut xobjects = String::new();
        for img in imgs {
            // Resolve object number by name.
            if let Some((idx, _)) = all_images
                .iter()
                .enumerate()
                .find(|(_, im)| im.name == img.name)
            {
                let obj = first_img_obj + idx;
                xobjects.push_str(&format!("/{} {} 0 R ", img.name, obj));
            }
        }
        let resources = if xobjects.is_empty() {
            "<< /Font << /F1 3 0 R >> >>".to_string()
        } else {
            format!("<< /Font << /F1 3 0 R >> /XObject << {xobjects}>> >>")
        };

        numbered.push((
            page_obj,
            format!(
                "{page_obj} 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {content_obj} 0 R /Resources {resources} >>\nendobj\n"
            )
            .into_bytes(),
        ));

        let chunk = pages.get(i).map(|c| c.as_slice()).unwrap_or(&[]);
        let line_refs: Vec<&str> = chunk.iter().map(|s| s.as_str()).collect();
        let stream = page_content_stream(&line_refs, imgs);
        let mut content_obj_bytes = format!(
            "{content_obj} 0 obj\n<< /Length {} >>\nstream\n",
            stream.len()
        )
        .into_bytes();
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

/// Objects 3=Font, 4=FontDescriptor, 5=FontFile2 for an embedded TrueType pack.
fn push_embedded_font_objects(
    numbered: &mut Vec<(usize, Vec<u8>)>,
    pack: &FontPack,
) -> Result<(), String> {
    if pack.ttf_bytes.is_empty() {
        return Err("font pack has no TTF bytes".into());
    }
    let base = pdf_name_escape(&pack.postscript_name);
    let widths = pack.pdf_widths_32_126();
    let widths_s = widths
        .iter()
        .map(|w| w.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let upem = pack.units_per_em() as i32;
    let ascent = upem * 8 / 10;
    let descent = -(upem / 5);
    let cap = upem * 7 / 10;

    // 3 Font
    numbered.push((
        3,
        format!(
            "3 0 obj\n<< /Type /Font /Subtype /TrueType /BaseFont /{base} /FirstChar 32 /LastChar 126 /Widths [{widths_s}] /FontDescriptor 4 0 R /Encoding /WinAnsiEncoding >>\nendobj\n"
        )
        .into_bytes(),
    ));
    // 4 FontDescriptor
    numbered.push((
        4,
        format!(
            "4 0 obj\n<< /Type /FontDescriptor /FontName /{base} /Flags 32 /FontBBox [0 {descent} {upem} {ascent}] /ItalicAngle 0 /Ascent {ascent} /Descent {descent} /CapHeight {cap} /StemV 80 /MissingWidth {} /FontFile2 5 0 R >>\nendobj\n",
            pack.advance_width(32)
        )
        .into_bytes(),
    ));
    // 5 FontFile2 stream
    let n = pack.ttf_bytes.len();
    let mut body = format!("5 0 obj\n<< /Length {n} /Length1 {n} >>\nstream\n").into_bytes();
    body.extend_from_slice(&pack.ttf_bytes);
    body.extend_from_slice(b"\nendstream\nendobj\n");
    numbered.push((5, body));
    Ok(())
}

fn pdf_name_escape(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn page_content_stream(lines: &[&str], images: &[PdfImage]) -> Vec<u8> {
    let mut content = String::new();
    // Images first (under text).
    for img in images {
        content.push_str("q\n");
        content.push_str(&format!(
            "{} 0 0 {} {} {} cm\n",
            img.draw_w_pt, img.draw_h_pt, img.x_pt, img.y_pt
        ));
        content.push_str(&format!("/{} Do\n", img.name));
        content.push_str("Q\n");
    }
    content.push_str("BT\n/F1 12 Tf\n14 TL\n72 720 Td\n");
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
    use qf_blocks::{BlockBody, BlockMetadata, ImageBlock, ParagraphBlock, Span};
    use qf_core::{ActorId, BlockId, DocumentId, OpSeq, SectionId, SeqIdGen, Timestamp, Version};
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

    fn tiny_png() -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba([255, 32, 32, 255]));
        let mut cur = std::io::Cursor::new(Vec::new());
        img.write_to(&mut cur, image::ImageFormat::Png).unwrap();
        cur.into_inner()
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

    #[test]
    fn pdf_embeds_truetype_fontfile2() {
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
                        text: "Font embed check".into(),
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

        let pack = crate::fonts::quill_mono_v1();
        let a = export_pdf_with_assets_and_font(
            &state,
            &SourceTable::default(),
            &ExportConfig::default(),
            None,
            Some(&pack),
        )
        .unwrap();
        let b = export_pdf_with_assets_and_font(
            &state,
            &SourceTable::default(),
            &ExportConfig::default(),
            None,
            Some(&pack),
        )
        .unwrap();
        assert_eq!(a, b, "embedded font PDF must be deterministic");
        let s = String::from_utf8_lossy(&a);
        assert!(
            s.contains("/Subtype /TrueType"),
            "missing TrueType font: {s}"
        );
        assert!(s.contains("/FontFile2"), "missing FontFile2: {s}");
        assert!(
            s.contains("QuillForgeMono") || s.contains("QuillForge"),
            "{s}"
        );
        // TTF magic inside stream
        assert!(a.windows(4).any(|w| w == b"\x00\x01\x00\x00"));
    }

    #[test]
    fn pdf_embeds_image_xobject_from_assets() {
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
        let asset_id = qf_core::AssetId::new(&mut gen);
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
                body: BlockBody::Image(ImageBlock {
                    asset_id,
                    alt: "ember".into(),
                    caption: vec![],
                    width_px: Some(2),
                    height_px: Some(2),
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

        let mut assets = BTreeMap::new();
        assets.insert(asset_id.as_uuid().to_string(), tiny_png());
        let pdf = export_pdf_with_assets(
            &state,
            &SourceTable::default(),
            &ExportConfig {
                include_bibliography: false,
                ..Default::default()
            },
            Some(&assets),
        )
        .unwrap();
        let s = String::from_utf8_lossy(&pdf);
        assert!(s.contains("/Subtype /Image"), "missing Image XObject: {s}");
        assert!(s.contains("/DeviceRGB"), "missing DeviceRGB");
        assert!(s.contains("/Im0"), "missing image resource name");
        assert!(pdf.starts_with(b"%PDF-1.4"));
    }
}
