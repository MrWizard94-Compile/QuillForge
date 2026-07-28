//! Word-class page chrome: rulers, headers/footers, image insert (P5).
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

    /// Insert an image block (placeholder asset) after the focused block / at end.
    pub fn insert_image(
        &mut self,
        alt: &str,
        width_px: u32,
        height_px: u32,
    ) -> Result<AssetId, EditorError> {
        let asset_id = AssetId::new(self.session.ids.as_mut());
        let asset_key = asset_id.as_uuid().to_string();
        self.session
            .assets
            .insert(asset_key, PLACEHOLDER_PNG.to_vec());

        let body = BlockBody::Image(ImageBlock {
            asset_id,
            alt: alt.to_string(),
            caption: vec![],
            width_px: Some(width_px.max(24)),
            height_px: Some(height_px.max(24)),
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

        // Focus last block if it accepts text; otherwise leave selection.
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
}
