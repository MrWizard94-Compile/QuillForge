# Render Scheduler Specification

**Document ID:** QF-RND-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Role

Consume `LayoutTree` + theme + viewport → GPU draw schedule. **Derived only.**

---

## 2. Components

```
LayoutTree → DirtyTracker → TileVirtualizer → Composer → qf-gpu backends
```

---

## 3. Dirty tracking

```rust
pub struct DirtyTracker {
    dirty_pages: RoaringBitmap,
    dirty_tiles: HashSet<TileId>,
    full_invalidated: bool,
}

impl DirtyTracker {
    pub fn on_layout_change(&mut self, old: &LayoutTree, new: &LayoutTree);
    pub fn on_theme_change(&mut self);
    pub fn on_viewport_change(&mut self, viewport: Rect);
}
```

- Compare page/fragment hashes where available.  
- Scroll without content change: no content tile redraw if cached.  

---

## 4. Virtualization

```rust
pub struct TileVirtualizer {
    tile_size_px: u32, // e.g. 512
}

pub struct TileId {
    pub page: u32,
    pub x: u32,
    pub y: u32,
}
```

- Only tiles intersecting viewport (+ overscan margin) are rasterized.  
- LRU cache of tiles with memory budget.  
- On GPU device loss: drop all tiles; rebuild.

---

## 5. Composition order

1. Page background  
2. Block backgrounds / callouts  
3. Text glyphs (atlas)  
4. Images  
5. Overlays: selection, carets, comments, presence  
6. Debug overlays (if enabled)  

Overlays may be immediate-mode each frame; content tiles cached.

---

## 6. Performance behavior

| Budget | Target |
|--------|--------|
| Steady scroll | 60 FPS on reference laptop |
| After keystroke | First paint of caret < 16ms; full tile refresh may lag 1 frame |
| Memory tiles | Configurable, default 256 MB |

Scheduler priorities:

1. Caret/selection  
2. Viewport center tiles  
3. Overscan  
4. Prefetch adjacent pages  

---

## 7. API

```rust
pub struct RenderScheduler {
    gpu: GpuContext,
    tiles: TileCache,
    dirty: DirtyTracker,
}

impl RenderScheduler {
    pub fn set_layout(&mut self, layout: Arc<LayoutTree>);
    pub fn set_theme(&mut self, theme: &Theme);
    pub fn render_frame(&mut self, viewport: Viewport, output: &mut FrameTarget) -> Result<()>;
}
```

---

## 8. Invariants

- Render never writes journal/kernel.  
- Frame with stale `state_gen` may display but must not be used for hit-testing without gen check.  
- Hit-testing uses layout coords + gen match.

---

*End of render specification.*
