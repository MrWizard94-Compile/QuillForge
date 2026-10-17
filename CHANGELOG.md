# Changelog

All notable changes to Quillforge are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Changed

- **WPAI Constitution adoption:** project guidance, CLI help, and CI now use
  the accepted canonical GitHub source instead of a vendored law tree.
  Project-specific product/architecture law is retained. Earlier governance
  entries below are history, not instructions to load the retired copies.
- **Declared formatting gate:** apply Rust 1.85.0 rustfmt to the published
  baseline in a separate mechanical commit; no unrelated local product edits
  or unpublished commits are included.
- **Cross-platform MSRV:** constrain the home/Wayland lock cohort to compatible
  patches after Linux CI exposed dependencies requiring newer Rust. Retain
  Rust 1.85.0 and all OS/test gates; repeat Windows checks for the changed cohort.

### Added

- **Full open/export matrix:** open `qfdoc|docx|md|html|txt`; export `pdf|docx|md|html|txt` (+ save qfdoc); CLI `open` / `export-all`; GUI Export tab wired (`docs/41-open-export-matrix.md`). PDF remains export-only.
- **Optional ONNX graph runtime:** Cargo feature `qf-search/onnx` adds pure-Rust `OnnxGraphRuntime` (QFEMB1 weight-table matmul + hash-bound fallback); default CI stays ORT-free (`docs/39-neural-onnx-pack.md`).
- **DOCX TrueType font embed:** `export_docx_with_assets_and_font` writes `word/fonts/*.ttf` + `fontTable.xml`; editor DOCX uses pin font (`docs/40-font-packs.md`).
- **Product mark:** `brand/assets/quillforge-product.png` embedded in the app; command-bar logo, insert image/logo, document-scaled PNG helper (`PRODUCT_MARK_PNG`).
- **Custom font packs:** `FontPack` + built-in `quill_mono_v1` TrueType pin; PDF embeds `/FontFile2`; editor PDF export uses pin for self-contained output (`docs/40-font-packs.md`).
- **Neural / ONNX embedding packs:** `neural_hash_v1` multi-scale offline pack, `onnx_v1` local model-file binding + pack JSON load/save, engine re-embed on pack swap (`docs/39-neural-onnx-pack.md`).
- **Live WS/WSS head notify:** concurrent session workers + hub live channels push unsolicited `Notify` frames to subscribers (`wait_notify`); mailbox `DrainNotify` still available (`docs/38-server-push-notify.md`).
- **Server-push head notify:** `Subscribe` / `DrainNotify` + hub mailbox fan-out on peer Push so clients need not poll Pull while idle (`docs/38-server-push-notify.md`).
- **DOCX DrawingML images:** `export_docx_with_assets` embeds PNG/JPEG as `word/media/*` with inline DrawingML; import restores package assets + Image blocks (`docs/37-docx-drawingml.md`).
- **Offline dense embeddings:** signed feature hashing pack `hash_ngram_v1` (tokens + char 3-grams, dim 64), `SearchMode::Dense`, hybrid chain, GUI Dense mode, `search-demo` pack JSON (`docs/36-dense-embeddings.md`).
- **PDF image XObjects + TLS PEM:** DeviceRGB image embedding in PDF export; production `TlsIdentity` PEM load (`docs/35-pdf-images-and-tls-pem.md`).
- **Long-lived WS/WSS sessions:** durable WebSocket and WSS sync session loops (`docs/30-websocket-sync.md`).
- **Multi-OS CI matrix:** Linux / Windows / macOS test + budgets, Ubuntu lint + constitution verify-pack, `ci-ok` aggregate (`docs/34-ci-matrix.md`).
- **Hybrid / semantic search:** token AND, phrase, TF–IDF cosine, hybrid merge; GUI search panel; `search-demo` (`docs/33-semantic-search.md`).
- **Rich media fidelity:** PNG decode/GUI paint, DOCX `w:tbl`, image lines in PDF export (`docs/32-rich-media.md`).
- **Browser sync path:** QFSNC1 over `ws://` / `wss://`, HTML client, HTTPS demo (`docs/30-websocket-sync.md`, `docs/31-browser-sync-client.md`).
- **Word-class product surface:** binding UX law (`docs/28-product-surface.md`), `ProductSurface` command-bar model, document-workspace theme, `quillforge surface` CLI (ADR-026).
- **Process-kill harness:** `qf-kill-harness` aborts after fsync commit; parent reopens journal (ADR-025).
- **Project status:** `docs/27-project-status.md` completion layers and residual roadmap.
- **Structured DOCX:** AST-driven export with Heading/Quote/Code styles and `styles.xml` (ADR-024).
- **RepoForge birth law:** LIFE-BIRTH-001 + ADOPT Option D for attaching this constitution pack.
- **TLS sync transport:** rustls TLS 1.3 adapter for QFSNC1 (`TlsSyncServer` / `TlsSyncClient`), shared wire codec, `tls-sync-demo` CLI, ADR-023.
- **AGENTS Constitution pack v5.0.0:** portable universal pack (`PACK.md`, `ADOPT.md`, `VERSION`), integrity tools, project pointer Level 4 entry, archive of pre-v4 monolith.
- **WPAI branding:** studio identity constants (`qf_core::brand`), forge-dark theme (`Theme::wpai_forge_dark`), CLI/version chrome, package `APP_VERSION` attribution, and in-repo `brand/` kit (voice + official marks).

## [0.1.0-foundation] — 2026-07-28

Foundation release: local-first document OS core is production-shaped, zero-warning, and CI-gated. A **WPAI Software** product of Wizard Productions AI Studio.

### Added

- **Canonical path:** Command → Operations → Journal → Kernel (dry-run before journal).
- **Package format:** `.qfdoc` ZIP+CBOR, atomic save with LKG, untrusted ZIP budgets.
- **Encryption:** QFENC1 Argon2id + XChaCha20-Poly1305 at-rest packages.
- **Recovery:** Torn journal, partial frames, handle-forget after fsync, LKG open, encrypted open.
- **Layout / render:** Deterministic fixed-point layout, golden fixtures, headless render.
- **Export:** Markdown, HTML, plain text, multi-page PDF 1.4, minimal OOXML DOCX.
- **Collab / sync:** RGA text CRDT, presence split, DurableSyncQueue, SyncHub, localhost TCP `QFSNC1` CBOR transport.
- **Plugins:** Capability host + Wasmtime guest fuel runtime.
- **AI:** Suggestion-only flow with content permission tokens.
- **Security:** ACL, audit redaction, package crypto.
- **Performance:** CI-gated `budget_*` tests (`docs/26-performance-budgets.md`).
- **Governance:** AGENTS Constitution v4.1 modular pack + `verify-pack.ps1`.
- **CLI:** `quillforge demo`, `sync-demo`, `version`.

### Known deferred (non-blocking for foundation tag)

- WebSocket / browser transport (TLS + TCP QFSNC1 ship).
- Full OOXML tables/media/images and custom fonts (structured DOCX styles ship).
- ~~OS process-spawn kill harness~~ closed (abort harness + synthetic suite).

### Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace budget_ -- --nocapture
cargo run -p qf-app -- demo
cargo run -p qf-app -- sync-demo
```

[0.1.0-foundation]: https://github.com/MrWizard94-Compile/QuillForge/releases/tag/v0.1.0-foundation
