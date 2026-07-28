# Render Scheduler Specification

**Document ID:** QF-RND-001  
**Version:** 1.1.0  
**Status:** Binding  

---

## 1. Role

Consume `LayoutTree` + theme + viewport → GPU draw schedule via `qf-gpu`. **Derived only** — never reads/writes journal or mutates kernel state.

---

## 2. Pipeline (foundation, as shipped)

```
LayoutTree → DirtyTracker → viewport cull (+ overscan) → tile ids → qf-gpu software encoder
```

| Component | Foundation behavior |
|-----------|---------------------|
| `DirtyTracker` | Full invalidate on layout/theme; per-tile set for last frame |
| Viewport cull | Fragments outside expanded viewport are skipped |
| Tile ids | `TileId { page, x, y }` over configurable tile size (default 512) |
| Composer | Colored rects per `FragmentKind` into software framebuffer |
| Tile pixel cache | **Not shipped** — roadmap (LRU tile bitmaps) |

---

## 3. Dirty tracking

```rust
pub struct DirtyTracker {
    pub dirty_tiles: HashSet<TileId>,
    pub full_invalidated: bool,
}
```

- Layout or theme change → full invalidate.  
- After a frame, `dirty_tiles` holds tiles touched by visible fragments.

---

## 4. Virtualization

```rust
pub struct TileId {
    pub page: u32,
    pub x: u32,
    pub y: u32,
}
```

- Only tiles intersecting viewport (+ overscan) are marked.  
- On GPU device loss: `recover_device` + full invalidate.

---

## 5. Composition order (foundation)

1. Clear page background (theme)  
2. Block/fragment rects by kind (text/heading/media/…)  
3. (Roadmap) glyph runs, images, overlays  

The interactive shell may composite via egui independently; this scheduler remains the pure derived path for tests and future native surfaces.

---

## 6. API (as implemented)

```rust
pub struct RenderScheduler {
    pub gpu: GpuContext,
    pub layout: Option<Arc<LayoutTree>>,
    pub theme: Theme,
    pub dirty: DirtyTracker,
    pub tile_size_px: u32,
    pub overscan_px: f32,
    pub last_stats: FrameStats,
}

impl RenderScheduler {
    pub fn set_layout(&mut self, layout: Arc<LayoutTree>);
    pub fn set_theme(&mut self, theme: Theme);
    pub fn recover_device(&mut self) -> Result<(), GpuError>;
    pub fn render_frame(&mut self, viewport: Viewport) -> Result<FrameStats, GpuError>;
}
```

`FrameStats` reports draw calls, fragments considered/drawn, tiles touched, device_lost.

---

## 7. Invariants

- Render never writes journal/kernel.  
- Hit-testing uses layout coords from the layout engine, not this scheduler’s pixels.

---

*End of render specification.*
