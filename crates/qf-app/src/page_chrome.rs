//! Word-class page chrome: rulers, headers/footers, image insert.
//!
//! Header/footer text is durable via `DocumentMeta.custom` keys (journaled
//! `SetDocumentMeta`). Images are `Image` blocks + package assets.

use qf_blocks::{BlockBody, ImageBlock};
use qf_core::{
    AssetId, Command, MetaPatch, ParentRef, PreservedMap, PreservedValue, TextOffset,
};
use qf_kernel::encode_body;

use crate::{EditorController, EditorError};

/// Durable meta keys (PreservedMap text values).
pub const META_HEADER: &str = "qf.page.header";
pub const META_FOOTER: &str = "qf.page.footer";

/// Minimal valid 1×1 PNG (transparent) used as image asset placeholder.
pub const PLACEHOLDER_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

/// Editable page chrome drafts in the shell (synced from / to meta).
#[derive(Clone, Debug, Default)]
pub struct PageChrome {
    pub header: String,
    pub footer: String,
    /// When true, Layout side panel shows header/footer editors.
    pub panel_visible: bool,
}

impl PageChrome {
    pub fn from_meta(custom: &PreservedMap) -> Self {
        Self {
            header: text_key(custom, META_HEADER).unwrap_or_default(),
            footer: text_key(custom, META_FOOTER).unwrap_or_default(),
            panel_visible: false,
        }
    }
}

fn text_key(map: &PreservedMap, key: &str) -> Option<String> {
    match map.get(key)? {
        PreservedValue::Text(s) => Some(s.clone()),
        _ => None,
    }
}

/// Decode image width/height (PNG via `image` crate).
pub fn decode_image_size(bytes: &[u8]) -> Option<(u32, u32)> {
    let img = image::load_from_memory(bytes).ok()?;
    Some((img.width(), img.height()))
}

/// Decode to RGBA8 pixels.
pub fn decode_image_rgba(bytes: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some((w, h, img.into_raw()))
}

/// Official Quillforge product mark (PNG), shipped under `brand/assets/`.
///
/// Embedded at compile time so the app never depends on a relative path at runtime.
pub const PRODUCT_MARK_PNG: &[u8] =
    include_bytes!("../../../brand/assets/quillforge-product.png");

/// Product mark bytes (same as [`PRODUCT_MARK_PNG`]).
pub fn product_mark_png() -> &'static [u8] {
    PRODUCT_MARK_PNG
}

/// Decode product mark; optionally downscale longest edge for document inserts.
pub fn product_mark_png_for_document(max_edge: u32) -> Result<Vec<u8>, String> {
    use image::imageops::FilterType;
    let img = image::load_from_memory(PRODUCT_MARK_PNG).map_err(|e| e.to_string())?;
    let w = img.width();
    let h = img.height();
    let max_edge = max_edge.clamp(32, 2048);
    let scaled = if w.max(h) > max_edge {
        let scale = max_edge as f32 / w.max(h) as f32;
        let nw = ((w as f32) * scale).round().max(1.0) as u32;
        let nh = ((h as f32) * scale).round().max(1.0) as u32;
        img.resize(nw, nh, FilterType::Triangle)
    } else {
        img
    };
    let mut buf = std::io::Cursor::new(Vec::new());
    scaled
        .write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}

/// Build a solid-color PNG for demos (ember-ish) without the product mark.
pub fn demo_png_bytes(width: u32, height: u32) -> Result<Vec<u8>, String> {
    use image::{ImageBuffer, Rgba, RgbaImage};
    let w = width.clamp(8, 512);
    let h = height.clamp(8, 512);
    let mut img: RgbaImage = ImageBuffer::new(w, h);
    for (x, y, pixel) in img.enumerate_pixels_mut() {
        // Ember gradient for WPAI brand (not Office blue).
        let t = (x + y) as f32 / (w + h) as f32;
        let r = (0xff as f32 * (0.85 + 0.15 * t)) as u8;
        let g = (0x7a as f32 * (0.7 + 0.3 * (1.0 - t))) as u8;
        let b = 0x26u8;
        *pixel = Rgba([r, g, b, 255]);
    }
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(buf.into_inner())
}

impl EditorController {
    pub fn page_header(&self) -> String {
        text_key(&self.session.state().document.meta.custom, META_HEADER).unwrap_or_default()
    }

    pub fn page_footer(&self) -> String {
        text_key(&self.session.state().document.meta.custom, META_FOOTER).unwrap_or_default()
    }

    /// Journal header and/or footer text into document meta.
    pub fn set_page_chrome(
        &mut self,
        header: Option<&str>,
        footer: Option<&str>,
    ) -> Result<(), EditorError> {
        let mut custom = self.session.state().document.meta.custom.clone();
        if let Some(h) = header {
            custom.insert(META_HEADER, PreservedValue::Text(h.to_string()));
        }
        if let Some(f) = footer {
            custom.insert(META_FOOTER, PreservedValue::Text(f.to_string()));
        }
        self.session.commit_command(Command::SetDocumentMeta {
            patch: MetaPatch {
                title: None,
                language: None,
                custom: Some(custom),
            },
        })?;
        Ok(())
    }

    /// Insert an image block (placeholder or real PNG bytes) after the last block.
    pub fn insert_image(
        &mut self,
        alt: &str,
        width_px: u32,
        height_px: u32,
    ) -> Result<AssetId, EditorError> {
        self.insert_image_bytes(alt, PLACEHOLDER_PNG, Some(width_px), Some(height_px))
    }

    /// Insert image from PNG/JPEG bytes: decode dimensions when possible, store asset.
    pub fn insert_image_bytes(
        &mut self,
        alt: &str,
        bytes: &[u8],
        width_hint: Option<u32>,
        height_hint: Option<u32>,
    ) -> Result<AssetId, EditorError> {
        let (decoded_w, decoded_h) = decode_image_size(bytes).unwrap_or((0, 0));
        let width_px = width_hint
            .or(Some(decoded_w))
            .unwrap_or(64)
            .max(1);
        let height_px = height_hint
            .or(Some(decoded_h))
            .unwrap_or(48)
            .max(1);

        let asset_id = AssetId::new(self.session.ids.as_mut());
        let asset_key = asset_id.as_uuid().to_string();
        self.session.assets.insert(asset_key, bytes.to_vec());

        let body = BlockBody::Image(ImageBlock {
            asset_id,
            alt: alt.to_string(),
            caption: vec![],
            width_px: Some(width_px),
            height_px: Some(height_px),
        });
        let bb = encode_body(&body).map_err(|e| EditorError::Other(e.to_string()))?;

        let index = {
            let doc = &self.session.state().document;
            let root = doc.root_section_id;
            doc.sections
                .get(&root)
                .map(|s| s.blocks.len() as u32)
                .unwrap_or(0)
        };

        self.session.commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index,
            body_type: bb.type_name,
            body_payload: bb.payload,
        })?;

        let last = {
            let doc = &self.session.state().document;
            let root = doc.root_section_id;
            doc.sections
                .get(&root)
                .and_then(|s| s.blocks.last().copied())
        };
        if let Some(id) = last {
            if self.session.state().document.project_text(id).is_some() {
                let _ = self.focus(id, TextOffset(0));
            }
        }
        self.reflow();
        Ok(asset_id)
    }

    /// Load image path into the package asset map and document.
    pub fn insert_image_path(&mut self, path: &std::path::Path, alt: &str) -> Result<AssetId, EditorError> {
        let bytes = std::fs::read(path).map_err(|e| EditorError::Other(e.to_string()))?;
        self.insert_image_bytes(alt, &bytes, None, None)
    }

    /// Decode asset to RGBA8 for GUI paint (None if missing/undecodable).
    pub fn asset_rgba(&self, asset_id: AssetId) -> Option<(u32, u32, Vec<u8>)> {
        let key = asset_id.as_uuid().to_string();
        let bytes = self.session.assets.get(&key)?;
        decode_image_rgba(bytes)
    }

    /// Asset raw bytes by id.
    pub fn asset_bytes(&self, asset_id: AssetId) -> Option<&[u8]> {
        let key = asset_id.as_uuid().to_string();
        self.session.assets.get(&key).map(|v| v.as_slice())
    }

    /// Count image blocks in the document (for tests / status).
    pub fn image_block_count(&self) -> usize {
        self.session
            .state()
            .document
            .blocks
            .values()
            .filter(|b| matches!(b.body, BlockBody::Image(_)))
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        decode_image_rgba, decode_image_size, demo_png_bytes, product_mark_png,
        product_mark_png_for_document, PRODUCT_MARK_PNG,
    };
    use crate::EditorController;
    use tempfile::tempdir;

    #[test]
    fn header_footer_round_trip_via_journal() {
        let dir = tempdir().unwrap();
        let mut ed = EditorController::create_default(&dir.path().join("hf.journal")).unwrap();
        ed.set_page_chrome(Some("Acme Report"), Some("Page · Confidential"))
            .unwrap();
        assert_eq!(ed.page_header(), "Acme Report");
        assert_eq!(ed.page_footer(), "Page · Confidential");
        // Second patch preserves header when only footer changes.
        ed.set_page_chrome(None, Some("v2 footer")).unwrap();
        assert_eq!(ed.page_header(), "Acme Report");
        assert_eq!(ed.page_footer(), "v2 footer");
    }

    #[test]
    fn insert_image_adds_block_and_asset() {
        let dir = tempdir().unwrap();
        let mut ed = EditorController::create_default(&dir.path().join("img.journal")).unwrap();
        ed.ensure_editable_paragraph().unwrap();
        let id = ed.insert_image("Diagram", 320, 180).unwrap();
        assert_eq!(ed.image_block_count(), 1);
        assert!(ed
            .session
            .assets
            .contains_key(&id.as_uuid().to_string()));
        assert!(ed
            .layout_tree
            .pages
            .iter()
            .flat_map(|p| &p.frames)
            .any(|f| matches!(f.kind, qf_layout::FragmentKind::Media)));
    }

    #[test]
    fn insert_demo_png_decodes_rgba() {
        let png = demo_png_bytes(32, 24).unwrap();
        let (w, h) = decode_image_size(&png).unwrap();
        assert_eq!((w, h), (32, 24));
        let (rw, rh, rgba) = decode_image_rgba(&png).unwrap();
        assert_eq!((rw, rh), (32, 24));
        assert_eq!(rgba.len(), 32 * 24 * 4);

        let dir = tempdir().unwrap();
        let mut ed = EditorController::create_default(&dir.path().join("png.journal")).unwrap();
        ed.ensure_editable_paragraph().unwrap();
        let id = ed.insert_image_bytes("Ember", &png, None, None).unwrap();
        let (aw, ah, _) = ed.asset_rgba(id).unwrap();
        assert_eq!((aw, ah), (32, 24));
        // PDF plain lines include image placeholder with size.
        let lines = ed.layout_tree.page_plain_lines(ed.session.state());
        let flat: Vec<_> = lines.into_iter().flatten().collect();
        assert!(
            flat.iter().any(|l| l.contains("[image:") && l.contains("32")),
            "expected image line in {flat:?}"
        );
    }

    #[test]
    fn product_mark_png_is_valid_and_insertable() {
        assert!(PRODUCT_MARK_PNG.len() > 1000);
        assert_eq!(&PRODUCT_MARK_PNG[0..8], b"\x89PNG\r\n\x1a\n");
        let (w, h) = decode_image_size(product_mark_png()).expect("decode product mark");
        assert!(w >= 16 && h >= 16, "mark size {w}x{h}");
        let doc = product_mark_png_for_document(256).unwrap();
        let (dw, dh) = decode_image_size(&doc).unwrap();
        assert!(dw.max(dh) <= 256);
        let dir = tempdir().unwrap();
        let mut ed = EditorController::create_default(&dir.path().join("mark.journal")).unwrap();
        ed.ensure_editable_paragraph().unwrap();
        let id = ed
            .insert_image_bytes("Quillforge", &doc, Some(dw), Some(dh))
            .unwrap();
        assert_eq!(ed.image_block_count(), 1);
        assert!(ed.asset_rgba(id).is_some());
    }
}
