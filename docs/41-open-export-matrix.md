# Open / Export Format Matrix

**Document ID:** QF-IO-001  
**Status:** Foundation  
**Law:** Local-first interchange; journaled apply for all imports  

---

## Supported

| Format | Open / import | Export | Notes |
|--------|---------------|--------|-------|
| **`.qfdoc`** | Yes (package open) | Yes (`save`) | Canonical package |
| **`.docx`** | Yes | Yes | DrawingML images + pin font embed |
| **`.md` / `.markdown`** | Yes | Yes | Headings, quote, code, `---` page break |
| **`.html` / `.htm`** | Yes | Yes | Lightweight tag scan import |
| **`.txt`** | Yes | Yes | Blank-line paragraphs |
| **`.pdf`** | **No** (export-only) | Yes | Layout-aligned PDF 1.4 + pin font |

---

## Surfaces

| Surface | Entry |
|---------|--------|
| Shell | `open_from_path`, `import_path`, `export_as`, `export_all` |
| Editor | `import_any_path`, `export_by_extension` |
| CLI | `quillforge open <file> [name]`, `quillforge export-all [name]` |
| GUI | Export tab: PDF, DOCX, Markdown, HTML, Plain text, All formats |
| Import | Place `import.docx` / `import.md` / `import.html` / `import.txt` beside package |

---

## Verify

```bash
cargo test -p qf-export text_import -- --nocapture
cargo test -p qf-app export_all_formats -- --nocapture
cargo test -p qf-app open_html -- --nocapture
cargo run -p qf-app -- export-all
```

---

## Out of scope

- PDF import / text extraction  
- RTF, ODT, EPUB  
- Full HTML/CSS fidelity  
