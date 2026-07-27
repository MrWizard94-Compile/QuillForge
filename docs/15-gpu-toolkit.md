# GPU Toolkit Specification (`qf-gpu`)

**Document ID:** QF-GPU-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Purpose

Custom **native Rust GPU abstraction** for Quillforge rendering. Not a general game engine; optimized for document UI: text, vectors, images, effects, atlases.

**Decision:** Implement on **wgpu** as the portability layer, with `qf-gpu` owning higher-level document-oriented APIs. Direct Vulkan/Metal/DX12 code is not scattered across app crates.

---

## 2. Modules

| Module | Responsibility |
|--------|----------------|
| `backends` | Device/adapter selection, surface, frame lifecycle |
| `atlas` | Glyph and image atlas packing |
| `text` | Glyph raster upload, run drawing |
| `vector` | Paths, rects, rounded rects, lines |
| `image` | Texture upload, sampling |
| `effects` | Shadows, blur (limited), opacity layers |
| `debug` | Overlays, GPU timings, wireframes |

---

## 3. API shape

```rust
pub struct GpuContext { /* device, queue, backends */ }

impl GpuContext {
    pub fn new(window: &impl raw_window_handle::HasWindowHandle) -> Result<Self>;
    pub fn resize(&mut self, width: u32, height: u32);
    pub fn begin_frame(&mut self) -> Result<FrameEncoder>;
    pub fn device_lost(&self) -> bool;
    pub fn recreate(&mut self) -> Result<()>;
}

pub struct FrameEncoder { /* encoders, pass stack */ }

impl FrameEncoder {
    pub fn clear(&mut self, color: Color);
    pub fn draw_text(&mut self, runs: &[GpuGlyphRun]);
    pub fn draw_rects(&mut self, rects: &[GpuRect]);
    pub fn draw_images(&mut self, imgs: &[GpuImageDraw]);
    pub fn draw_paths(&mut self, paths: &[GpuPath]);
    pub fn push_opacity(&mut self, alpha: f32);
    pub fn pop_layer(&mut self);
    pub fn end(self) -> Result<()>;
}

pub struct GlyphAtlas { /* ... */ }
pub struct ImageAtlas { /* ... */ }
```

---

## 4. Backends

| Platform | wgpu backend preference |
|----------|-------------------------|
| Windows | DX12, Vulkan fallback |
| macOS | Metal |
| Linux | Vulkan, GL fallback |

Feature flags select optional backends. Headless tests use `wgpu` noop/software where available.

---

## 5. Atlas

- Shelf or skyline packer for glyphs  
- Growth strategy: double dimension up to max (4096) then multi-atlas  
- Eviction: LRU by tile/glyph usage  
- Threading: uploads on render thread in v1  

---

## 6. Text

- Glyph rasterization via `swash` or `cosmic-text` shaping at higher layer; `qf-gpu` receives bitmaps/SDF  
- **Decision:** Shape in `qf-layout` / text subsystem; GPU only blits glyphs  
- Support subpixel positioning at render stage only  

---

## 7. Vector

- AA rects and rounded rects via SDF or geometry  
- Paths tessellated CPU-side (lyon) for v1 simplicity/determinism of mesh  

---

## 8. Image

- RGBA8 / premultiplied  
- Mipmaps optional  
- Color space: sRGB textures  

---

## 9. Effects

v1 limited set:

- Opacity groups  
- Drop shadow (preblur kernel)  
- Rounded clip  

Full filter graph post-v1.

---

## 10. Debug

```rust
pub struct GpuDebugOptions {
    pub show_tile_bounds: bool,
    pub show_atlas: bool,
    pub log_timings: bool,
}
```

---

## 11. Device loss

On `device_lost`:

1. Notify render scheduler  
2. Drop all GPU resources  
3. Recreate device  
4. Rebuild atlases from CPU caches  
5. Full dirty invalidate  

---

## 12. Non-goals

- Compute-based layout  
- 3D scene graph  
- Shader hot reload in production  

---

*End of GPU toolkit specification.*
