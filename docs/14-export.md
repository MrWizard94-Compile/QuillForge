# Export Specification

**Document ID:** QF-EXP-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Goals

- Deterministic exports for fixed inputs  
- Fidelity appropriate per format  
- Unknown content never silently dropped  
- Plugin transform hooks capability-gated  

---

## 2. Pipeline

```
CanonicalState
  → ExportIR (normalized intermediate)
  → FormatWriter (md|html|pdf|txt|docx)
  → optional PluginHooks
  → bytes + ExportManifest
```

```rust
pub struct ExportRequest {
    pub format: ExportFormat,
    pub config: ExportConfig,
    pub range: Option<ExportRange>, // whole doc default
}

pub enum ExportFormat {
    Markdown,
    Html,
    Pdf,
    PlainText,
    Docx,
}

pub trait Exporter: Send + Sync {
    fn export(&self, state: &CanonicalState, req: &ExportRequest) -> Result<ExportOutput>;
}

pub struct ExportOutput {
    pub bytes: Vec<u8>,
    pub manifest: ExportManifest, // content hash, generator version, config hash
}
```

---

## 3. Determinism

Same `(state_gen snapshot bytes, ExportConfig, font set for PDF, plugin set versions)` → same `bytes`.

| Input pinned | Why |
|--------------|-----|
| ExportConfig | Styles, paper size |
| Generator version | Code path |
| Font subset IDs | PDF |
| Time | MUST NOT embed wall clock unless config `include_timestamps` |

---

## 4. Format rules (summary)

### Markdown

- AT + headings + lists + tables (GFM)  
- Unknown → HTML comment with type + base64 payload  
- Images → relative paths or embedded per config  

### HTML

- Semantic tags  
- Unknown → `data-qf-unknown-*` attributes  
- CSS optional external  

### PDF

- Uses layout engine for pagination fidelity  
- Embedded fonts subset  
- Unknown → placeholder box with type label  

### Plain text

- Structural newlines; marks stripped  
- Unknown → `[unknown:type]` marker  

### DOCX

- OOXML via pure Rust writer  
- Unknown → structured alternative or placeholder paragraph  

---

## 5. Plugin hooks

```rust
pub trait ExportHook {
    fn transform_ir(&self, ir: ExportIR) -> Result<ExportIR>;
}
```

Runs only with `export.transform` capability. Failures skip hook and log; core export continues unless policy is strict.

---

## 6. Fidelity tests

Golden files per format for fixture documents. PDF compared via content stream hash with pinned fonts. Unknown fixtures included.

---

*End of export specification.*
