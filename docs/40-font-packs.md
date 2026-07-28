# Custom Font Packs

**Document ID:** QF-FONT-001  
**Status:** Foundation  
**Law:** Local-first, deterministic export — no system font substitution  

---

## Why

Layout and PDF export must be **byte-stable** across OS. Relying on “whatever Helvetica the viewer supplies” is fine for scaffolding; product fidelity needs **embedded** type.

---

## FontPack

| Field | Role |
|-------|------|
| `family` | Human family name |
| `postscript_name` | PDF `/BaseFont` |
| `algo` | `quill_mono_v1` (pin) or `truetype_v1` (file) |
| `ttf_bytes` | TrueType / OpenType payload |
| `sha256` | Content hash |

### Built-in pin: `quill_mono_v1`

Generated in-process monospaced TTF (ASCII boxes). Deterministic; no binary fixture required.

### Custom files

```rust
let pack = FontPack::load_ttf_path(path, "My Face")?;
// or
let pack = FontPack::from_ttf_bytes("My Face", bytes)?;
```

---

## Surfaces

| Surface | Behavior |
|---------|----------|
| PDF | `export_pdf_with_assets_and_font(..., Some(&pack))` → `/Subtype /TrueType` + `/FontFile2` |
| Editor PDF | Uses `FontPack::pin_default()` so export is self-contained |
| DOCX | `export_docx_with_assets_and_font` → `word/fonts/*.ttf` + `fontTable.xml` + styles family |
| Editor DOCX | Embeds pin font (`quill_mono_v1`) with DrawingML assets |
| API | `qf_export::{FontPack, quill_mono_v1, build_mono_ttf}` |

### DOCX package parts (when font supplied)

| Part | Role |
|------|------|
| `word/fonts/{PostScript}.ttf` | TrueType payload |
| `word/fontTable.xml` | `w:embedRegular` → font rel |
| `word/_rels/fontTable.xml.rels` | Relationship to font file |
| `word/styles.xml` | Normal style `w:rFonts` = pack family |

---

## Verify

```bash
cargo test -p qf-export fonts -- --nocapture
cargo test -p qf-export pdf_embeds_truetype -- --nocapture
cargo test -p qf-export docx_embeds_truetype -- --nocapture
cargo test -p qf-app print_layout_pdf -- --nocapture
```

---

## Out of scope

- Full OpenType CFF / variable fonts  
- Glyph subsetting for large CJK fonts  
- System font enumeration / fallback chains  
- Bold/italic face embedding (regular only)  
