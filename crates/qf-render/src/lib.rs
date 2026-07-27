//! Render scheduler: dirty tracking, virtualization, composition.

use qf_gpu::{Color, FrameEncoder, GpuContext, GpuError, GpuRect};
use qf_layout::LayoutTree;
use qf_theme::Theme;
use std::collections::HashSet;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TileId {
    pub page: u32,
    pub x: u32,
    pub y: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
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
}

pub struct RenderScheduler {
    pub gpu: GpuContext,
    pub layout: Option<Arc<LayoutTree>>,
    pub theme: Theme,
    pub dirty: DirtyTracker,
    pub tile_size_px: u32,
}

impl RenderScheduler {
    pub fn new(gpu: GpuContext, theme: Theme) -> Self {
        Self {
            gpu,
            layout: None,
            theme,
            dirty: DirtyTracker::default(),
            tile_size_px: 512,
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

    pub fn render_frame(&mut self, _viewport: Viewport) -> Result<u32, GpuError> {
        let mut enc: FrameEncoder = self.gpu.begin_frame()?;
        enc.clear(Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        });
        // Page background + fragment placeholders (foundation).
        if let Some(layout) = &self.layout {
            for page in &layout.pages {
                for frag in &page.frames {
                    enc.draw_rects(&[GpuRect {
                        x: frag.bounds.x.0 as f32 / 64.0,
                        y: frag.bounds.y.0 as f32 / 64.0,
                        w: frag.bounds.w.0 as f32 / 64.0,
                        h: frag.bounds.h.0 as f32 / 64.0,
                        color: Color {
                            r: 0.95,
                            g: 0.95,
                            b: 0.95,
                            a: 1.0,
                        },
                    }]);
                }
            }
        }
        let draws = enc.draws;
        enc.end()?;
        self.dirty.full_invalidated = false;
        Ok(draws)
    }
}
