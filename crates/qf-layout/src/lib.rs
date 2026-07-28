//! Deterministic layout using fixed-point coordinates (1/64 pt).
//!
//! Design influences: Typst (deterministic publish), ProseMirror (pure model → view).
//! Layout never mutates canonical state.

use qf_blocks::BlockBody;
use qf_core::{BlockId, Hash256, OpSeq};
use qf_kernel::CanonicalState;
use serde::{Deserialize, Serialize};

/// Fixed-point: 1 unit = 1/64 point.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct Fixed32(pub i32);

impl Fixed32 {
    pub const fn from_pt(pt: i32) -> Self {
        Self(pt.saturating_mul(64))
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
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
            page_width: Fixed32::from_pt(612),
            page_height: Fixed32::from_pt(792),
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
    pub kind: FragmentKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum FragmentKind {
    Text,
    Heading { level: u8 },
    Code,
    List,
    Table,
    Media,
    Callout,
    Equation,
    Diagram,
    Break,
    Unknown,
    Other,
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

impl LayoutTree {
    /// Deterministic content hash of the layout tree (for goldens / equality).
    pub fn content_hash(&self) -> Hash256 {
        let mut buf = Vec::new();
        if ciborium::into_writer(self, &mut buf).is_ok() {
            return Hash256::sha256(&buf);
        }
        Hash256::sha256(format!("{self:?}").as_bytes())
    }

    /// Number of print pages (at least one empty page).
    pub fn page_count(&self) -> u32 {
        self.pages.len().max(1) as u32
    }

    /// Plain lines per print page, in layout frame order — shared by GUI preview and PDF.
    pub fn page_plain_lines(&self, state: &CanonicalState) -> Vec<Vec<String>> {
        self.pages
            .iter()
            .map(|page| {
                page.frames
                    .iter()
                    .filter_map(|f| state.document.project_text(f.block_id))
                    .filter(|t| !t.is_empty())
                    .collect()
            })
            .collect()
    }
}

/// Convert fixed-point (1/64 pt) to floating points.
pub fn fixed_to_pt(f: Fixed32) -> f32 {
    f.0 as f32 / 64.0
}

/// US Letter width/height in points (matches default [`LayoutConfig`]).
pub const PAGE_WIDTH_PT: f32 = 612.0;
pub const PAGE_HEIGHT_PT: f32 = 792.0;

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
        let config_hash = Hash256::sha256(
            [
                cfg.page_width.0.to_le_bytes(),
                cfg.page_height.0.to_le_bytes(),
                cfg.margin.0.to_le_bytes(),
                cfg.line_height.0.to_le_bytes(),
                (cfg.version as i32).to_le_bytes(),
            ]
            .concat()
            .as_slice(),
        );
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
            .flat_map(|s| s.blocks.iter().copied())
            .collect();

        for block_id in order {
            let (h, kind) = block_metrics(state, block_id, cfg);
            if kind != FragmentKind::Break
                && y.0 + h.0 > cfg.page_height.0 - cfg.margin.0
                && !pages.last().map(|p| p.frames.is_empty()).unwrap_or(true)
            {
                page_idx = page_idx.saturating_add(1);
                pages.push(PageFrame {
                    index: page_idx,
                    frames: vec![],
                });
                y = cfg.margin;
            }
            if kind == FragmentKind::Break {
                // Force page break after current content.
                page_idx = page_idx.saturating_add(1);
                pages.push(PageFrame {
                    index: page_idx,
                    frames: vec![],
                });
                y = cfg.margin;
                continue;
            }
            pages
                .last_mut()
                .expect("pages non-empty")
                .frames
                .push(FragmentFrame {
                    block_id,
                    bounds: RectFixed {
                        x: cfg.margin,
                        y,
                        w: content_w,
                        h,
                    },
                    kind,
                });
            y = y
                .saturating_add(h)
                .saturating_add(Fixed32(cfg.line_height.0 / 2));
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
        // Incremental reflow must equal full layout (contract).
        self.full_layout(state, cfg)
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

fn block_metrics(
    state: &CanonicalState,
    id: BlockId,
    cfg: &LayoutConfig,
) -> (Fixed32, FragmentKind) {
    let Some(block) = state.document.blocks.get(&id) else {
        return (cfg.line_height, FragmentKind::Other);
    };
    match &block.body {
        BlockBody::Paragraph(p) => {
            let chars: u32 = p.spans.iter().map(|s| s.text.chars().count() as u32).sum();
            let lines = estimate_lines(chars, 80);
            (
                Fixed32(
                    cfg.line_height
                        .0
                        .saturating_mul(lines as i32)
                        .max(cfg.line_height.0),
                ),
                FragmentKind::Text,
            )
        }
        BlockBody::Heading(h) => (
            Fixed32(
                cfg.line_height
                    .0
                    .saturating_mul(2 + i32::from(h.level.min(3))),
            ),
            FragmentKind::Heading { level: h.level },
        ),
        BlockBody::Quote(_) => (Fixed32(cfg.line_height.0 * 2), FragmentKind::Text),
        BlockBody::Code(c) => {
            let lines = c.text.lines().count().max(1) as i32;
            (
                Fixed32(cfg.line_height.0.saturating_mul(lines)),
                FragmentKind::Code,
            )
        }
        BlockBody::List(l) => {
            let n = count_list_items(&l.items).max(1) as i32;
            (
                Fixed32(cfg.line_height.0.saturating_mul(n)),
                FragmentKind::List,
            )
        }
        BlockBody::Checklist(c) => (
            Fixed32(
                cfg.line_height
                    .0
                    .saturating_mul(c.items.len().max(1) as i32),
            ),
            FragmentKind::List,
        ),
        BlockBody::Table(t) => (
            Fixed32(
                cfg.line_height
                    .0
                    .saturating_mul(t.rows.len().max(1) as i32)
                    .saturating_mul(2),
            ),
            FragmentKind::Table,
        ),
        BlockBody::Image(img) => {
            let h = img.height_px.unwrap_or(200) as i32;
            // Map CSS px ~ pt coarsely: 1px ≈ 0.75pt → fixed 1/64
            (Fixed32::from_pt((h * 3 / 4).max(24)), FragmentKind::Media)
        }
        BlockBody::Video(_) => (Fixed32::from_pt(180), FragmentKind::Media),
        BlockBody::Equation(_) => (Fixed32::from_pt(28), FragmentKind::Equation),
        BlockBody::Diagram(_) => (Fixed32::from_pt(120), FragmentKind::Diagram),
        BlockBody::Citation(_) => (cfg.line_height, FragmentKind::Text),
        BlockBody::Callout(_) => (Fixed32(cfg.line_height.0 * 3), FragmentKind::Callout),
        BlockBody::Embed(_) => (Fixed32::from_pt(96), FragmentKind::Media),
        BlockBody::PageBreak => (Fixed32(0), FragmentKind::Break),
        BlockBody::SectionBreak => (Fixed32(0), FragmentKind::Break),
        BlockBody::Unknown(_) => (Fixed32::from_pt(48), FragmentKind::Unknown),
    }
}

fn estimate_lines(chars: u32, cols: u32) -> u32 {
    if chars == 0 {
        return 1;
    }
    chars.div_ceil(cols.max(1))
}

fn count_list_items(items: &[qf_blocks::ListItem]) -> usize {
    let mut n = 0;
    for it in items {
        n += 1;
        n += count_list_items(&it.children);
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_blocks::{BlockBody, BlockMetadata, HeadingBlock, ParagraphBlock, Span};
    use qf_core::{
        ActorId, BlockBodyBytes, DocumentId, OpSeq, Operation, ParentRef, SectionId, SeqIdGen,
        Timestamp, Version,
    };
    use qf_doc::{Block, Document};
    use qf_kernel::{CanonicalState, EntityTables, Kernel};

    fn empty_state() -> (CanonicalState, SeqIdGen) {
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
        (state, gen)
    }

    #[test]
    fn empty_layout_one_page() {
        let (state, _) = empty_state();
        let tree = LayoutEngine::new().full_layout(&state, &LayoutConfig::default());
        assert_eq!(tree.pages.len(), 1);
        assert!(tree.pages[0].frames.is_empty());
    }

    #[test]
    fn layout_hash_stable_and_incremental_equals_full() {
        let (mut state, mut gen) = empty_state();
        let bid = BlockId::new(&mut gen);
        let mut k = Kernel::new(state.clone());
        k.apply(&Operation::InsertBlock {
            block_id: bid,
            parent: ParentRef::DocumentRoot,
            index: 0,
            body: BlockBodyBytes {
                type_name: "paragraph".into(),
                payload: b"Hello layout determinism".to_vec(),
            },
        })
        .unwrap();
        state = k.state().clone();

        let eng = LayoutEngine::new();
        let cfg = LayoutConfig::default();
        let a = eng.full_layout(&state, &cfg);
        let b = eng.full_layout(&state, &cfg);
        assert_eq!(a.content_hash(), b.content_hash());
        let dirty = eng.reflow_dirty(
            &state,
            &cfg,
            DirtySet {
                full: true,
                block_ids: vec![bid],
            },
        );
        assert_eq!(a.content_hash(), dirty.content_hash());
    }

    #[test]
    fn page_plain_lines_follow_frames() {
        let (mut state, mut gen) = empty_state();
        let p1 = BlockId::new(&mut gen);
        let actor = state.document.meta.authors[0];
        let meta = BlockMetadata {
            created_by: actor,
            created_at_seq: OpSeq(1),
            attrs: Default::default(),
        };
        state.document.blocks.insert(
            p1,
            Block {
                id: p1,
                meta,
                body: BlockBody::Paragraph(ParagraphBlock {
                    spans: vec![Span {
                        id: qf_core::SpanId(uuid::Uuid::nil()),
                        text: "Hello pages".into(),
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
            .push(p1);
        let tree = LayoutEngine::new().full_layout(&state, &LayoutConfig::default());
        let lines = tree.page_plain_lines(&state);
        assert_eq!(lines.len(), tree.pages.len());
        assert!(lines[0].iter().any(|l| l.contains("Hello pages")));
        assert_eq!(tree.page_count(), 1);
    }

    #[test]
    fn page_break_creates_new_page() {
        let (mut state, mut gen) = empty_state();
        let p1 = BlockId::new(&mut gen);
        let br = BlockId::new(&mut gen);
        let p2 = BlockId::new(&mut gen);
        let actor = state.document.meta.authors[0];
        let meta = BlockMetadata {
            created_by: actor,
            created_at_seq: OpSeq(1),
            attrs: Default::default(),
        };
        state.document.blocks.insert(
            p1,
            Block {
                id: p1,
                meta: meta.clone(),
                body: BlockBody::Paragraph(ParagraphBlock {
                    spans: vec![Span {
                        id: qf_core::SpanId(uuid::Uuid::nil()),
                        text: "a".into(),
                        marks: vec![],
                    }],
                }),
            },
        );
        state.document.blocks.insert(
            br,
            Block {
                id: br,
                meta: meta.clone(),
                body: BlockBody::PageBreak,
            },
        );
        state.document.blocks.insert(
            p2,
            Block {
                id: p2,
                meta,
                body: BlockBody::Heading(HeadingBlock {
                    level: 1,
                    spans: vec![Span {
                        id: qf_core::SpanId(uuid::Uuid::nil()),
                        text: "b".into(),
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
            .extend([p1, br, p2]);

        let tree = LayoutEngine::new().full_layout(&state, &LayoutConfig::default());
        assert!(tree.pages.len() >= 2);
    }
}
