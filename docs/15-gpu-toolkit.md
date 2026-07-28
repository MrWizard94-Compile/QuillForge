# GPU Toolkit Specification (`qf-gpu`)

**Document ID:** QF-GPU-001  
**Version:** 1.1.0  
**Status:** Binding  

---

## 1. Purpose

Document-oriented GPU abstraction for Quillforge composition: text cells, vectors, images, atlases, and frame lifecycle.

### Shipped backend (foundation)

| Backend | Status |
|---------|--------|
| **Software (CPU) RGBA8 framebuffer** | **Shipped** — complete headless path |
| Native `wgpu` (DX12/Metal/Vulkan) | **Roadmap** — not claimed in this crate today |

`qf-gpu` **must not** be documented as “wgpu-backed” until a real device backend lands. See `docs/21-roadmap.md` Phase 4.

---

## 2. Modules (implemented)

| Module | Responsibility |
|--------|----------------|
| context | `GpuContext`, resize, device-loss, framebuffer |
| atlas | `GlyphAtlas` cell tracker; `ImageAtlas` RGBA upload by id |
| encoder | clear, rects, paths (Bresenham), glyph cells, image blit + alpha |
| debug | `GpuDebugOptions` (tile bounds / atlas / timings flags) |

Effects (blur/shadow layers) and native surface presentation are **out of foundation scope**.

---

## 3. API shape (as implemented)

```rust
pub struct GpuContext { /* software framebuffer + atlases */ }

impl GpuContext {
    pub fn headless(width: u32, height: u32) -> Result<Self, GpuError>;
    pub fn resize(&mut self, width: u32, height: u32);
    pub fn begin_frame(&mut self) -> Result<FrameEncoder<'_>, GpuError>;
    pub fn device_lost(&self) -> bool;
    pub fn recreate(&mut self) -> Result<(), GpuError>;
    pub fn framebuffer_rgba(&self) -> &[u8];
}

impl FrameEncoder<'_> {
    pub fn clear(&mut self, color: Color);
    pub fn draw_text(&mut self, runs: &[GpuGlyphRun]);
    pub fn draw_rects(&mut self, rects: &[GpuRect]);
    pub fn draw_images(&mut self, imgs: &[GpuImageDraw]);
    pub fn draw_paths(&mut self, paths: &[GpuPath]);
    pub fn end(self) -> Result<(), GpuError>;
}

pub struct GlyphAtlas { /* grid occupancy */ }
pub struct ImageAtlas { /* id → RGBA slots */ }
```

**Errors:** `GpuError::{DeviceLost, Init, OutOfMemory, MissingTexture, Other}`.

---

## 4. Product shell note

The Word-class desktop shell (`qf-app` / egui) paints layout fragments directly for interactivity. `qf-gpu` + `qf-render` remain the **derived composition / validation** path (tests, future native surface). Both must stay pure of journal/kernel mutation.

---

## 5. Roadmap (not shipped)

- `wgpu` device + swapchain presentation  
- Real glyph SDF/bitmap upload from shaped runs  
- Shelf/skyline atlas packer multi-atlas  
- Opacity layer stack  

---

*End of GPU toolkit specification.*
