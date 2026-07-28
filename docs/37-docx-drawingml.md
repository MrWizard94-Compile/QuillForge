# DOCX DrawingML Image Export

**Document ID:** QF-DOCX-DRAW-001  
**Status:** Foundation  
**Law:** Deterministic package bytes when inputs fixed  

---

## Behavior

| Piece | Detail |
|-------|--------|
| API | `qf_export::export_docx_with_assets(state, sources, cfg, assets)` |
| Media | `word/media/imageN.png` or `.jpeg` (sniff magic) |
| Markup | Inline `w:drawing` / `wp:inline` / `pic:pic` / `a:blip r:embed` |
| Rels | `rId1` styles; `rId2+` image relationships |
| Size | EMU at 96 DPI (`9525` EMU/px); max content width 6.5in |
| Fallback | Caption `[image: alt W×H]` when asset missing or unknown format |
| Editor export | `export_docx_bytes` passes package assets |
| Import | `import_docx_blocks` → `DocxImportBlock::Image` + media bytes |
| Editor import | Stores package assets + journaled `Image` block |

Deterministic: fixed ZIP timestamps + stable part order.

---

## Verify

```bash
cargo test -p qf-export docx_embeds_drawingml -- --nocapture
cargo test -p qf-export import_drawingml -- --nocapture
cargo test -p qf-app docx_image_drawingml -- --nocapture
```

---

## Out of scope

- EMF/WMF/SVG  
- Floating anchors / wrap layouts  
- Custom font embedding  
