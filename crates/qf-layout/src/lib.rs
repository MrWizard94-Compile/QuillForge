//! Deterministic layout using fixed-point coordinates (1/64 pt).

use qf_core::{BlockId, Hash256, OpSeq};
use qf_kernel::CanonicalState;
use serde::{Deserialize, Serialize};

/// Fixed-point: 1 unit = 1/64 point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Fixed32(pub i32);

impl Fixed32 {
    pub const fn from_pt(pt: i32) -> Self {
        Self(pt.saturating_mul(64))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutConfig {
    pub page_width: Fixed32,
    pub page_height: Fixed32,
    pub margin: Fixed32,
    pub line_height: Fixed32,
    pub version: u32,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            page_width: Fixed32::from_pt(612),  // US Letter width
            page_height: Fixed32::from_pt(792), // US Letter height
            margin: Fixed32::from_pt(72),
            line_height: Fixed32::from_pt(14),
            version: 1,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RectFixed {
    pub x: Fixed32,
    pub y: Fixed32,
    pub w: Fixed32,
    pub h: Fixed32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FragmentFrame {
    pub block_id: BlockId,
    pub bounds: RectFixed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageFrame {
    pub index: u32,
    pub frames: Vec<FragmentFrame>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LayoutTree {
    pub state_gen: OpSeq,
    pub config_hash: Hash256,
    pub pages: Vec<PageFrame>,
}

#[derive(Clone, Debug, Default)]
pub struct DirtySet {
    pub block_ids: Vec<BlockId>,
    pub full: bool,
}

pub struct LayoutEngine;

impl LayoutEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn full_layout(&self, state: &CanonicalState, cfg: &LayoutConfig) -> LayoutTree {
        let config_hash = Hash256::sha256(format!("{cfg:?}").as_bytes());
        let content_w = Fixed32(cfg.page_width.0 - cfg.margin.0 * 2);
        let mut y = cfg.margin;
        let mut pages = vec![PageFrame {
            index: 0,
            frames: vec![],
        }];
        let mut page_idx = 0u32;

        let order: Vec<BlockId> = state
            .document
            .section_order
            .iter()
            .filter_map(|sid| state.document.sections.get(sid))
            .flat_map(|s| s.blocks.clone())
            .collect();

        for block_id in order {
            let h = block_height(state, block_id, cfg);
            if y.0 + h.0 > cfg.page_height.0 - cfg.margin.0 {
                page_idx += 1;
                pages.push(PageFrame {
                    index: page_idx,
                    frames: vec![],
                });
                y = cfg.margin;
            }
            pages.last_mut().unwrap().frames.push(FragmentFrame {
                block_id,
                bounds: RectFixed {
                    x: cfg.margin,
                    y,
                    w: content_w,
                    h,
                },
            });
            y = Fixed32(y.0 + h.0 + cfg.line_height.0 / 2);
        }

        LayoutTree {
            state_gen: state.op_seq,
            config_hash,
            pages,
        }
    }

    pub fn reflow_dirty(
        &self,
        state: &CanonicalState,
        cfg: &LayoutConfig,
        _dirty: DirtySet,
    ) -> LayoutTree {
        // Foundation: full reflow (incremental comes later; must equal full).
        self.full_layout(state, cfg)
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn block_height(state: &CanonicalState, id: BlockId, cfg: &LayoutConfig) -> Fixed32 {
    use qf_blocks::BlockBody;
    let Some(block) = state.document.blocks.get(&id) else {
        return cfg.line_height;
    };
    match &block.body {
        BlockBody::Heading(_) => Fixed32(cfg.line_height.0 * 2),
        BlockBody::Unknown(_) => Fixed32::from_pt(48),
        BlockBody::PageBreak => Fixed32(0),
        _ => cfg.line_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::{ActorId, DocumentId, SectionId, SeqIdGen, Timestamp, Version};
    use qf_doc::Document;
    use qf_kernel::EntityTables;

    #[test]
    fn empty_layout_one_page() {
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
        let tree = LayoutEngine::new().full_layout(&state, &LayoutConfig::default());
        assert_eq!(tree.pages.len(), 1);
        assert!(tree.pages[0].frames.is_empty());
    }
}
