# Layout Engine Specification

**Document ID:** QF-LAY-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Goals

- Deterministic pagination and reflow  
- Pure function of `(CanonicalState, LayoutConfig, FontVersion)`  
- Never mutates canonical state  
- Supports dirty-range incremental layout  

---

## 2. Determinism rules

| Input | Effect |
|-------|--------|
| Document snapshot | Content |
| LayoutConfig | Page size, margins, default styles |
| Font set version + metrics | Line breaking |
| Locale for hyphenation | Optional; must be pinned in config |
| Device pixel ratio | **Not** input to layout logical coords |

**Layout coordinates are logical units (points, 1/72 inch).** Pixel snapping is render-stage only.

**Forbidden nondeterminism:** hash map iteration order, system default font substitution without pin, timestamps, threads racing into shared buffers without ordered merge.

---

## 3. Layout config

```rust
pub struct LayoutConfig {
    pub page: PageSpec,
    pub margins: Margins,
    pub default_font: FontRef,
    pub default_size_pt: f32, // binary-stable via ordered ops; store as fixed-point internally
    pub line_spacing: LineSpacing,
    pub hyphenation: HyphenationMode,
    pub locale: String,
    pub version: u32, // config schema
}

pub struct PageSpec {
    pub width_pt: Fixed32,  // fixed point
    pub height_pt: Fixed32,
}

// Decision: use fixed-point Fixed32 (1/64 pt) for layout math to improve determinism.
pub struct Fixed32(i32);
```

**Decision:** Fixed-point layout arithmetic (1/64 pt) for positions and breaks. Justification: cross-platform float variance risks determinism priority.

---

## 4. Output model

```rust
pub struct LayoutTree {
    pub state_gen: OpSeq,
    pub config_hash: Hash256,
    pub pages: Vec<PageFrame>,
}

pub struct PageFrame {
    pub index: u32,
    pub frames: Vec<FragmentFrame>,
}

pub struct FragmentFrame {
    pub block_id: BlockId,
    pub kind: FragmentKind,
    pub bounds: RectFixed,
    pub glyphs: Vec<GlyphRun>, // for text
}
```

---

## 5. Incremental reflow

```rust
pub struct LayoutEngine {
    cache: LayoutCache,
}

impl LayoutEngine {
    pub fn full_layout(&mut self, state: &CanonicalState, cfg: &LayoutConfig) -> LayoutTree;
    pub fn reflow_dirty(
        &mut self,
        state: &CanonicalState,
        cfg: &LayoutConfig,
        dirty: DirtySet,
    ) -> LayoutTree;
}
```

Dirty tracking:

- From `KernelEvent` map to block ids  
- Blocks after a height-changing edit in flow may invalidate  
- Use dependency intervals: edit at block B invalidates from B through end of section or until stable break point  

---

## 6. Block layout behaviors

| Block | Behavior |
|-------|----------|
| Paragraph | Line break with font metrics |
| Heading | Style scale by level |
| Quote | Indent + optional bar |
| Code | Monospace; overflow wrap or horizontal clip per config |
| List/Checklist | Hanging indent markers |
| Table | Grid algorithm; deterministic column widths |
| Image/Video | Intrinsic size scaled to content width |
| Equation | Box metrics from engine; fallback height if unavailable |
| Diagram | Precomputed or placeholder size |
| PageBreak | Force new page |
| SectionBreak | Section style reset |
| Unknown | Fixed placeholder min height |

---

## 7. Pagination

- Greedy page packing with keep-with-next for headings + first line  
- Orphans/widows: configurable min lines  
- Tables: row-level break allowed; cell content reflows  

---

## 8. Testing

- Golden layout fixtures: state + config → page count + frame hashes  
- Cross-OS CI on Windows/macOS/Linux with pinned fonts in repo  
- Incremental reflow equals full layout hash  
- Fuzz random text for no panics / monotonic y positions  

---

*End of layout specification.*
