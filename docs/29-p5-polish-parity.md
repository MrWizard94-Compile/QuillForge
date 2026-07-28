# P5 Polish Parity Set — Rulers, Headers/Footers, Images

**Document ID:** QF-UX-002  
**Status:** Binding product polish baseline  
**Brand:** WPAI only  

---

## Parity set (what “done” means)

| Feature | Product behavior | Implementation |
|---------|------------------|----------------|
| **Rulers** | Toggleable horizontal + vertical inch rulers in Page view | `ProductSurface.show_rulers` · View → Ruler · GUI paint |
| **Header** | Document-level header on each page canvas + PDF | Meta key `qf.page.header` via journaled `SetDocumentMeta` |
| **Footer** | Document-level footer on each page canvas + PDF | Meta key `qf.page.footer` via journaled `SetDocumentMeta` |
| **Images** | Insert image block with alt + size; layout Media frame; asset in package | `Image` block + `session.assets` placeholder PNG |

Out of scope for this baseline (later): live image decode/display, floating wrap, headers per section, full Word header XML round-trip.

---

## How to verify

```bash
cargo test -p qf-app page_chrome -- --nocapture
cargo run -p qf-app -- polish-demo
cargo run -p qf-app -- edit   # Layout tab · Ruler · Insert image
```

---

## Kernel law

Header/footer and image insert still use **Command → Journal → Kernel**. Layout remains derived.
