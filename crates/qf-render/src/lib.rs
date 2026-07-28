//! Render scheduler: dirty tracking, viewport cull, composition into software GPU.
//!
//! **Derived only.** Never reads/writes journal or mutates kernel state.
//! Consumes `LayoutTree` + theme + viewport → `qf-gpu` software framebuffer.
//! Native wgpu presentation is roadmap — not claimed here.

use qf_gpu::{Color, FrameEncoder, GpuContext, GpuError, GpuRect};
use qf_layout::{FragmentKind, LayoutTree};
use qf_theme::Theme;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileId {
    pub page: u32,
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Viewport {
    pub fn contains_point(&self, px: f32, py: f32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }

    pub fn intersects_rect(&self, x: f32, y: f32, w: f32, h: f32) -> bool {
        x < self.x + self.w && x + w > self.x && y < self.y + self.h && y + h > self.y
    }
}

#[derive(Default)]
pub struct DirtyTracker {
    pub dirty_tiles: HashSet<TileId>,
    pub full_invalidated: bool,
}

impl DirtyTracker {
    pub fn invalidate_all(&mut self) {
        self.full_invalidated = true;
        self.dirty_tiles.clear();
    }

    pub fn on_layout_change(&mut self) {
        self.invalidate_all();
    }

    pub fn mark_tile(&mut self, id: TileId) {
        if !self.full_invalidated {
            self.dirty_tiles.insert(id);
        }
    }
}

/// Statistics from one composed frame (for tests / telemetry counters).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrameStats {
    pub draw_calls: u32,
    pub fragments_considered: u32,
    pub fragments_drawn: u32,
    pub tiles_touched: u32,
    pub device_lost: bool,
}

pub struct RenderScheduler {
    pub gpu: GpuContext,
    pub layout: Option<Arc<LayoutTree>>,
    pub theme: Theme,
    pub dirty: DirtyTracker,
    pub tile_size_px: u32,
    /// Overscan margin in pixels around viewport for prefetch.
    pub overscan_px: f32,
    pub last_stats: FrameStats,
}

impl RenderScheduler {
    pub fn new(gpu: GpuContext, theme: Theme) -> Self {
        Self {
            gpu,
            layout: None,
            theme,
            dirty: DirtyTracker::default(),
            tile_size_px: 512,
            overscan_px: 128.0,
            last_stats: FrameStats::default(),
        }
    }

    pub fn set_layout(&mut self, layout: Arc<LayoutTree>) {
        self.layout = Some(layout);
        self.dirty.on_layout_change();
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
        self.dirty.invalidate_all();
    }

    /// Recreate GPU resources after device loss; full invalidate.
    pub fn recover_device(&mut self) -> Result<(), GpuError> {
        self.gpu.recreate()?;
        self.dirty.invalidate_all();
        Ok(())
    }

    pub fn render_frame(&mut self, viewport: Viewport) -> Result<FrameStats, GpuError> {
        if self.gpu.device_lost() {
            self.recover_device()?;
        }

        let mut stats = FrameStats {
            device_lost: false,
            ..Default::default()
        };

        let mut enc: FrameEncoder = self.gpu.begin_frame()?;
        let bg = theme_bg(&self.theme);
        enc.clear(bg);

        let expanded = Viewport {
            x: viewport.x - self.overscan_px,
            y: viewport.y - self.overscan_px,
            w: viewport.w + self.overscan_px * 2.0,
            h: viewport.h + self.overscan_px * 2.0,
        };

        let mut tiles: HashSet<TileId> = HashSet::new();

        if let Some(layout) = self.layout.clone() {
            for page in &layout.pages {
                // Simple vertical page stack: page N starts at N * page_height_pt
                let page_origin_y = page.index as f32 * 792.0;
                for frag in &page.frames {
                    stats.fragments_considered += 1;
                    let x = fixed_to_px(frag.bounds.x);
                    let y = fixed_to_px(frag.bounds.y) + page_origin_y;
                    let w = fixed_to_px(frag.bounds.w);
                    let h = fixed_to_px(frag.bounds.h);
                    if !expanded.intersects_rect(x, y, w, h) {
                        continue;
                    }
                    stats.fragments_drawn += 1;
                    let color = color_for_kind(&frag.kind, &self.theme);
                    enc.draw_rects(&[GpuRect { x, y, w, h, color }]);

                    // Track tiles
                    let ts = self.tile_size_px as f32;
                    let x0 = (x / ts).floor() as u32;
                    let y0 = (y / ts).floor() as u32;
                    let x1 = ((x + w) / ts).floor() as u32;
                    let y1 = ((y + h) / ts).floor() as u32;
                    for ty in y0..=y1 {
                        for tx in x0..=x1 {
                            let id = TileId {
                                page: page.index,
                                x: tx,
                                y: ty,
                            };
                            tiles.insert(id);
                            self.dirty.mark_tile(id);
                        }
                    }
                }
            }
        }

        stats.draw_calls = enc.draws;
        stats.tiles_touched = tiles.len() as u32;
        enc.end()?;
        self.dirty.full_invalidated = false;
        self.dirty.dirty_tiles = tiles;
        self.last_stats = stats.clone();
        Ok(stats)
    }
}

fn fixed_to_px(f: qf_layout::Fixed32) -> f32 {
    // Layout uses 1/64 pt; treat 1pt ≈ 1px for headless composition.
    f.0 as f32 / 64.0
}

fn theme_bg(theme: &Theme) -> Color {
    if let Some(c) = theme.resolve(qf_core::ColorId(1)) {
        Color {
            r: c.r as f32 / 255.0,
            g: c.g as f32 / 255.0,
            b: c.b as f32 / 255.0,
            a: c.a as f32 / 255.0,
        }
    } else {
        Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        }
    }
}

fn color_for_kind(kind: &FragmentKind, _theme: &Theme) -> Color {
    match kind {
        FragmentKind::Text => Color {
            r: 0.92,
            g: 0.92,
            b: 0.94,
            a: 1.0,
        },
        FragmentKind::Heading { .. } => Color {
            r: 0.85,
            g: 0.88,
            b: 0.95,
            a: 1.0,
        },
        FragmentKind::Code => Color {
            r: 0.90,
            g: 0.90,
            b: 0.88,
            a: 1.0,
        },
        FragmentKind::List => Color {
            r: 0.91,
            g: 0.94,
            b: 0.91,
            a: 1.0,
        },
        FragmentKind::Table => Color {
            r: 0.93,
            g: 0.91,
            b: 0.88,
            a: 1.0,
        },
        FragmentKind::Media => Color {
            r: 0.88,
            g: 0.90,
            b: 0.95,
            a: 1.0,
        },
        FragmentKind::Callout => Color {
            r: 0.95,
            g: 0.93,
            b: 0.85,
            a: 1.0,
        },
        FragmentKind::Equation | FragmentKind::Diagram => Color {
            r: 0.90,
            g: 0.88,
            b: 0.95,
            a: 1.0,
        },
        FragmentKind::Unknown => Color {
            r: 0.95,
            g: 0.85,
            b: 0.85,
            a: 1.0,
        },
        FragmentKind::Break | FragmentKind::Other => Color {
            r: 0.95,
            g: 0.95,
            b: 0.95,
            a: 1.0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_blocks::{BlockBody, BlockMetadata, ParagraphBlock, Span};
    use qf_core::{ActorId, BlockId, DocumentId, OpSeq, SectionId, SeqIdGen, Timestamp, Version};
    use qf_doc::{Block, Document};
    use qf_kernel::{CanonicalState, EntityTables};
    use qf_layout::{LayoutConfig, LayoutEngine};

    fn sample_layout() -> LayoutTree {
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
                        text: "render me".into(),
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
        LayoutEngine::new().full_layout(&state, &LayoutConfig::default())
    }

    #[test]
    fn headless_render_draws_fragments_in_viewport() {
        let gpu = GpuContext::headless(800, 600).unwrap();
        let mut sched = RenderScheduler::new(gpu, Theme::light_default());
        let layout = Arc::new(sample_layout());
        let gen_before = layout.state_gen;
        sched.set_layout(layout);
        let stats = sched
            .render_frame(Viewport {
                x: 0.0,
                y: 0.0,
                w: 800.0,
                h: 600.0,
            })
            .unwrap();
        assert!(stats.fragments_drawn >= 1);
        assert!(stats.draw_calls >= 1);
        // Layout generation unchanged by render (derived only).
        assert_eq!(sched.layout.as_ref().unwrap().state_gen, gen_before);
    }

    #[test]
    fn out_of_view_fragments_skipped() {
        let gpu = GpuContext::headless(800, 600).unwrap();
        let mut sched = RenderScheduler::new(gpu, Theme::light_default());
        sched.set_layout(Arc::new(sample_layout()));
        let stats = sched
            .render_frame(Viewport {
                x: 10_000.0,
                y: 10_000.0,
                w: 100.0,
                h: 100.0,
            })
            .unwrap();
        assert_eq!(stats.fragments_drawn, 0);
    }

    #[test]
    fn device_loss_recovery() {
        let mut gpu = GpuContext::headless(100, 100).unwrap();
        gpu.device_lost = true;
        let mut sched = RenderScheduler::new(gpu, Theme::light_default());
        sched.set_layout(Arc::new(sample_layout()));
        let stats = sched
            .render_frame(Viewport {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 100.0,
            })
            .unwrap();
        assert!(!sched.gpu.device_lost());
        assert!(stats.fragments_drawn >= 1);
    }
}
