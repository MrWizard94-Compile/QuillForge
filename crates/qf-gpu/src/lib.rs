//! Custom native Rust GPU abstraction (wgpu-backed in production).
//!
//! Modules: backends, atlas, text, vector, image, effects, debug.
//! Foundation provides the public API surface without binding a GPU at compile time.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GpuError {
    #[error("device lost")]
    DeviceLost,
    #[error("init failed: {0}")]
    Init(String),
    #[error("out of memory")]
    OutOfMemory,
    #[error("{0}")]
    Other(String),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct GpuRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub color: Color,
}

#[derive(Clone, Debug)]
pub struct GpuGlyphRun {
    pub glyphs: Vec<u32>,
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Debug)]
pub struct GpuImageDraw {
    pub texture_id: u32,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Clone, Debug)]
pub struct GpuPath {
    pub points: Vec<(f32, f32)>,
    pub color: Color,
}

pub struct GlyphAtlas {
    pub width: u32,
    pub height: u32,
}

impl GlyphAtlas {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

pub struct ImageAtlas {
    pub width: u32,
    pub height: u32,
}

/// GPU context. Production creates a wgpu device; foundation is CPU-validation ready.
pub struct GpuContext {
    pub width: u32,
    pub height: u32,
    pub device_lost: bool,
    pub glyph_atlas: GlyphAtlas,
    pub image_atlas: ImageAtlas,
}

impl GpuContext {
    pub fn headless(width: u32, height: u32) -> Result<Self, GpuError> {
        Ok(Self {
            width,
            height,
            device_lost: false,
            glyph_atlas: GlyphAtlas::new(1024, 1024),
            image_atlas: ImageAtlas { width: 2048, height: 2048 },
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    pub fn device_lost(&self) -> bool {
        self.device_lost
    }

    pub fn recreate(&mut self) -> Result<(), GpuError> {
        self.device_lost = false;
        self.glyph_atlas = GlyphAtlas::new(1024, 1024);
        Ok(())
    }

    pub fn begin_frame(&mut self) -> Result<FrameEncoder, GpuError> {
        if self.device_lost {
            return Err(GpuError::DeviceLost);
        }
        Ok(FrameEncoder {
            draws: 0,
            cleared: false,
        })
    }
}

pub struct FrameEncoder {
    pub draws: u32,
    pub cleared: bool,
}

impl FrameEncoder {
    pub fn clear(&mut self, _color: Color) {
        self.cleared = true;
    }

    pub fn draw_text(&mut self, runs: &[GpuGlyphRun]) {
        self.draws += runs.len() as u32;
    }

    pub fn draw_rects(&mut self, rects: &[GpuRect]) {
        self.draws += rects.len() as u32;
    }

    pub fn draw_images(&mut self, imgs: &[GpuImageDraw]) {
        self.draws += imgs.len() as u32;
    }

    pub fn draw_paths(&mut self, paths: &[GpuPath]) {
        self.draws += paths.len() as u32;
    }

    pub fn end(self) -> Result<(), GpuError> {
        Ok(())
    }
}

#[derive(Clone, Debug, Default)]
pub struct GpuDebugOptions {
    pub show_tile_bounds: bool,
    pub show_atlas: bool,
    pub log_timings: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_frame() {
        let mut ctx = GpuContext::headless(800, 600).unwrap();
        let mut enc = ctx.begin_frame().unwrap();
        enc.clear(Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        });
        enc.draw_rects(&[GpuRect {
            x: 0.0,
            y: 0.0,
            w: 10.0,
            h: 10.0,
            color: Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        }]);
        enc.end().unwrap();
    }
}
