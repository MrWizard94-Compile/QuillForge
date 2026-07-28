# PDF Image XObjects & Production TLS PEM

**Document ID:** QF-MEDIA-TLS-001  
**Status:** Foundation depth  

---

## PDF image XObjects

| Item | Detail |
|------|--------|
| API | `qf_export::export_pdf_with_assets(state, sources, cfg, Some(assets))` |
| Format | PDF 1.4 `/Subtype /Image` · `/DeviceRGB` · 8 bpc |
| Source | Package asset map (UUID → PNG/JPEG bytes), decoded via `image` |
| Placement | Layout Media frame bounds → PDF user space |
| Cap | Long edge ≤ 240 px (export budget) |
| Editor | `EditorController::export_pdf_bytes` passes `session.assets` |

Without assets, PDF remains text-only (prior behavior).

---

## Production TLS identity (PEM)

| API | Use |
|-----|-----|
| `TlsIdentity::generate_dev()` | Self-signed localhost demos |
| `TlsIdentity::certificate_pem()` / `private_key_pem()` | Export material |
| `TlsIdentity::from_pem(cert, key)` | Load PKCS#8 + cert chain from strings |
| `TlsIdentity::from_pem_files(cert_path, key_path)` | Load from disk |

Chain: multiple `BEGIN CERTIFICATE` blocks allowed; key must be `BEGIN PRIVATE KEY` (PKCS#8).

---

## Verify

```bash
cargo test -p qf-export pdf_embeds_image -- --nocapture
cargo test -p qf-sync tls_identity_pem -- --nocapture
cargo run -p qf-app -- polish-demo
```
