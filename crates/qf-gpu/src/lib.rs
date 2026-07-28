//! Document-oriented GPU abstraction — **software (CPU) framebuffer backend**.
//!
//! # Scope (honest)
//!
//! This crate ships a complete **headless software** backend: real RGBA8 pixels,
//! rect fills, path strokes, glyph-cell text, and image blit from an image atlas.
//! It is used for layout→composition validation and tests without a window.
//!
//! **Not claimed:** a native `wgpu` / Vulkan / Metal device path. That remains
//! roadmap (`docs/21-roadmap.md`). Do not document this crate as “wgpu-backed.”
//!
//! # Modules (implemented)
//!
//! - **context** — `GpuContext` + device-loss recovery  
//! - **atlas** — `GlyphAtlas` grid + `ImageAtlas` texture slots  
//! - **encoder** — clear / rect / path / text / image draws into a framebuffer  
//! - **debug** — `GpuDebugOptions`

use thiserror::Error;

#[derive(Debug, Error)]
pub enum GpuError {
    #[error("device lost")]
    DeviceLost,
    #[error("init failed: {0}")]
    Init(String),
    #[error("out of memory")]
    OutOfMemory,
    #[error("texture not found: {0}")]
    MissingTexture(u32),
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

impl Color {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub fn to_rgba8(self) -> [u8; 4] {
        [
            (self.r.clamp(0.0, 1.0) * 255.0) as u8,
            (self.g.clamp(0.0, 1.0) * 255.0) as u8,
            (self.b.clamp(0.0, 1.0) * 255.0) as u8,
            (self.a.clamp(0.0, 1.0) * 255.0) as u8,
        ]
    }
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

/// Fixed-size glyph atlas (grid occupancy tracking for software text).
pub struct GlyphAtlas {
    pub width: u32,
    pub height: u32,
    /// Occupied cell count (tests / diagnostics).
    pub cells_used: u32,
}

impl GlyphAtlas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            cells_used: 0,
        }
    }

    /// Reserve a 8×8 cell; returns false if atlas is full.
    pub fn alloc_cell(&mut self) -> bool {
        let max = (self.width / 8).saturating_mul(self.height / 8);
        if self.cells_used >= max {
            return false;
        }
        self.cells_used += 1;
        true
    }
}

/// Image texture atlas: id → RGBA8 pixels + dimensions.
pub struct ImageAtlas {
    pub width: u32,
    pub height: u32,
    textures: std::collections::BTreeMap<u32, (u32, u32, Vec<u8>)>,
    next_id: u32,
}

impl ImageAtlas {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            textures: std::collections::BTreeMap::new(),
            next_id: 1,
        }
    }

    /// Upload RGBA8 image; returns texture id.
    pub fn upload_rgba(&mut self, w: u32, h: u32, rgba: Vec<u8>) -> Result<u32, GpuError> {
        let need = (w as usize).saturating_mul(h as usize).saturating_mul(4);
        if rgba.len() < need {
            return Err(GpuError::Init("image rgba too short".into()));
        }
        if w > self.width || h > self.height {
            return Err(GpuError::OutOfMemory);
        }
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        self.textures.insert(id, (w, h, rgba));
        Ok(id)
    }

    pub fn get(&self, id: u32) -> Option<(u32, u32, &[u8])> {
        self.textures
            .get(&id)
            .map(|(w, h, px)| (*w, *h, px.as_slice()))
    }
}

/// Software GPU context with a live RGBA8 framebuffer.
pub struct GpuContext {
    pub width: u32,
    pub height: u32,
    pub device_lost: bool,
    pub glyph_atlas: GlyphAtlas,
    pub image_atlas: ImageAtlas,
    /// Row-major RGBA8 framebuffer (owned pixels).
    framebuffer: Vec<u8>,
}

impl GpuContext {
    pub fn headless(width: u32, height: u32) -> Result<Self, GpuError> {
        let width = width.max(1);
        let height = height.max(1);
        let pixels = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or(GpuError::OutOfMemory)?;
        Ok(Self {
            width,
            height,
            device_lost: false,
            glyph_atlas: GlyphAtlas::new(1024, 1024),
            image_atlas: ImageAtlas::new(2048, 2048),
            framebuffer: vec![0u8; pixels],
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        self.width = width;
        self.height = height;
        let n = (width as usize) * (height as usize) * 4;
        self.framebuffer.resize(n, 0);
    }

    pub fn device_lost(&self) -> bool {
        self.device_lost
    }

    pub fn recreate(&mut self) -> Result<(), GpuError> {
        self.device_lost = false;
        self.glyph_atlas = GlyphAtlas::new(1024, 1024);
        let n = (self.width as usize) * (self.height as usize) * 4;
        self.framebuffer = vec![0u8; n];
        Ok(())
    }

    /// Simulate device loss (tests / recovery path).
    pub fn force_device_lost(&mut self) {
        self.device_lost = true;
    }

    pub fn framebuffer_rgba(&self) -> &[u8] {
        &self.framebuffer
    }

    pub fn begin_frame(&mut self) -> Result<FrameEncoder<'_>, GpuError> {
        if self.device_lost {
            return Err(GpuError::DeviceLost);
        }
        Ok(FrameEncoder {
            width: self.width,
            height: self.height,
            framebuffer: &mut self.framebuffer,
            image_atlas: &self.image_atlas,
            draws: 0,
            cleared: false,
        })
    }
}

/// Frame encoder writing into the parent context framebuffer.
pub struct FrameEncoder<'a> {
    width: u32,
    height: u32,
    framebuffer: &'a mut [u8],
    image_atlas: &'a ImageAtlas,
    pub draws: u32,
    pub cleared: bool,
}

impl FrameEncoder<'_> {
    pub fn clear(&mut self, color: Color) {
        let px = color.to_rgba8();
        for chunk in self.framebuffer.chunks_exact_mut(4) {
            chunk.copy_from_slice(&px);
        }
        self.cleared = true;
    }

    pub fn draw_text(&mut self, runs: &[GpuGlyphRun]) {
        for run in runs {
            // Software “glyph”: 6×8 solid cell per codepoint, advancing on x.
            let mut x = run.x;
            for g in &run.glyphs {
                let shade = ((*g as u8).wrapping_mul(17)).max(32);
                self.fill_rect_px(
                    x as i32,
                    run.y as i32,
                    6,
                    8,
                    [shade, shade, shade.saturating_add(20), 255],
                );
                x += 6.0;
                self.draws += 1;
            }
        }
    }

    pub fn draw_rects(&mut self, rects: &[GpuRect]) {
        for r in rects {
            let c = r.color.to_rgba8();
            self.fill_rect_px(r.x as i32, r.y as i32, r.w.ceil() as i32, r.h.ceil() as i32, c);
            self.draws += 1;
        }
    }

    pub fn draw_images(&mut self, imgs: &[GpuImageDraw]) {
        for img in imgs {
            if let Some((tw, th, src)) = self.image_atlas.get(img.texture_id) {
                self.blit_scaled(
                    src,
                    tw,
                    th,
                    img.x as i32,
                    img.y as i32,
                    img.w.ceil() as i32,
                    img.h.ceil() as i32,
                );
                self.draws += 1;
            }
        }
    }

    pub fn draw_paths(&mut self, paths: &[GpuPath]) {
        for path in paths {
            let c = path.color.to_rgba8();
            for w in path.points.windows(2) {
                self.draw_line(w[0].0 as i32, w[0].1 as i32, w[1].0 as i32, w[1].1 as i32, c);
            }
            self.draws += 1;
        }
    }

    pub fn end(self) -> Result<(), GpuError> {
        Ok(())
    }

    fn fill_rect_px(&mut self, x: i32, y: i32, w: i32, h: i32, rgba: [u8; 4]) {
        let w = w.max(0);
        let h = h.max(0);
        let x1 = x.saturating_add(w);
        let y1 = y.saturating_add(h);
        for py in y.max(0)..y1.min(self.height as i32) {
            for px in x.max(0)..x1.min(self.width as i32) {
                self.put_pixel(px as u32, py as u32, rgba);
            }
        }
    }

    fn put_pixel(&mut self, x: u32, y: u32, rgba: [u8; 4]) {
        if x >= self.width || y >= self.height {
            return;
        }
        let i = ((y as usize) * (self.width as usize) + (x as usize)) * 4;
        if i + 3 >= self.framebuffer.len() {
            return;
        }
        // Src-over alpha blend when src alpha < 255.
        let sa = rgba[3] as u32;
        if sa == 0 {
            return;
        }
        if sa == 255 {
            self.framebuffer[i..i + 4].copy_from_slice(&rgba);
            return;
        }
        let inv = 255 - sa;
        for c in 0..3 {
            let d = self.framebuffer[i + c] as u32;
            let s = rgba[c] as u32;
            self.framebuffer[i + c] = ((s * sa + d * inv) / 255) as u8;
        }
        self.framebuffer[i + 3] = 255;
    }

    fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, rgba: [u8; 4]) {
        let mut x0 = x0;
        let mut y0 = y0;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if x0 >= 0 && y0 >= 0 {
                self.put_pixel(x0 as u32, y0 as u32, rgba);
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    fn blit_scaled(
        &mut self,
        src: &[u8],
        sw: u32,
        sh: u32,
        dx: i32,
        dy: i32,
        dw: i32,
        dh: i32,
    ) {
        if sw == 0 || sh == 0 || dw <= 0 || dh <= 0 {
            return;
        }
        for row in 0..dh {
            for col in 0..dw {
                let sx = ((col as u32) * sw) / (dw as u32);
                let sy = ((row as u32) * sh) / (dh as u32);
                let si = ((sy as usize) * (sw as usize) + (sx as usize)) * 4;
                if si + 3 >= src.len() {
                    continue;
                }
                let rgba = [src[si], src[si + 1], src[si + 2], src[si + 3]];
                let px = dx + col;
                let py = dy + row;
                if px >= 0 && py >= 0 {
                    self.put_pixel(px as u32, py as u32, rgba);
                }
            }
        }
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
    fn headless_frame_writes_pixels() {
        let mut ctx = GpuContext::headless(32, 32).unwrap();
        {
            let mut enc = ctx.begin_frame().unwrap();
            enc.clear(Color::WHITE);
            enc.draw_rects(&[GpuRect {
                x: 2.0,
                y: 2.0,
                w: 4.0,
                h: 4.0,
                color: Color::BLACK,
            }]);
            assert!(enc.cleared);
            assert_eq!(enc.draws, 1);
            enc.end().unwrap();
        }
        let fb = ctx.framebuffer_rgba();
        // Corner remains white.
        assert_eq!(&fb[0..4], &[255, 255, 255, 255]);
        // Pixel (2,2) is black.
        let i = (2 * 32 + 2) * 4;
        assert_eq!(&fb[i..i + 3], &[0, 0, 0]);
    }

    #[test]
    fn image_atlas_blit() {
        let mut ctx = GpuContext::headless(16, 16).unwrap();
        let id = ctx
            .image_atlas
            .upload_rgba(2, 2, vec![255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255])
            .unwrap();
        {
            let mut enc = ctx.begin_frame().unwrap();
            enc.clear(Color::WHITE);
            enc.draw_images(&[GpuImageDraw {
                texture_id: id,
                x: 0.0,
                y: 0.0,
                w: 4.0,
                h: 4.0,
            }]);
            enc.end().unwrap();
        }
        assert_eq!(ctx.framebuffer_rgba()[0], 255);
        assert_eq!(ctx.framebuffer_rgba()[1], 0);
    }

    #[test]
    fn device_lost_blocks_frame() {
        let mut ctx = GpuContext::headless(8, 8).unwrap();
        ctx.force_device_lost();
        assert!(ctx.begin_frame().is_err());
        ctx.recreate().unwrap();
        assert!(ctx.begin_frame().is_ok());
    }

    #[test]
    fn path_and_text_draw() {
        let mut ctx = GpuContext::headless(64, 64).unwrap();
        let mut enc = ctx.begin_frame().unwrap();
        enc.clear(Color::WHITE);
        enc.draw_paths(&[GpuPath {
            points: vec![(1.0, 1.0), (20.0, 20.0), (40.0, 5.0)],
            color: Color::BLACK,
        }]);
        enc.draw_text(&[GpuGlyphRun {
            glyphs: vec![b'A' as u32, b'B' as u32],
            x: 8.0,
            y: 8.0,
        }]);
        assert!(enc.draws >= 2);
        enc.end().unwrap();
    }
}
