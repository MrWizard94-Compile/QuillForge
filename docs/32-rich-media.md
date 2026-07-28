# Rich Media Fidelity

**Document ID:** QF-MEDIA-001  
**Status:** Foundation depth  
**Brand:** WPAI  

---

## Scope (this delivery)

| Capability | Behavior |
|------------|----------|
| **PNG decode** | `image` crate (png-only) → RGBA8 for GUI + size metadata |
| **Insert** | `insert_image_bytes` / path / demo brand PNG (ember gradient) |
| **GUI paint** | egui textures from decoded assets on Media layout frames |
| **PDF lines** | Layout `page_plain_lines` includes `[image: alt W×H]` and table text |
| **PDF images** | DeviceRGB XObjects from package assets (`export_pdf_with_assets`) |
| **DOCX tables** | Real `w:tbl` markup with cell projected text |
| **DOCX images** | Caption line with alt + pixel size (DrawingML embed later) |

---

## Verify

```bash
cargo test -p qf-app insert_demo_png -- --nocapture
cargo test -p qf-export docx_emits_real_table -- --nocapture
cargo run -p qf-app -- polish-demo
```

---

## Out of scope (next)

- JPEG/WebP features (feature-gate later)  
- DrawingML embedded images in DOCX  
- DeviceRGB image XObjects in PDF  
- TrueType font embedding  
