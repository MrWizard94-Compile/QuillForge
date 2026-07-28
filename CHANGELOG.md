# Changelog

All notable changes to Quillforge are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0-foundation] — 2026-07-28

Foundation release: local-first document OS core is production-shaped, zero-warning, and CI-gated.

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

- TLS/WebSocket WAN transport adapter (loopback TCP + logical hub ship).
- Rich PDF/DOCX typography (fonts, images, styles).
- OS process-spawn kill harness (synthetic power-loss matrix is binding).

### Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace budget_ -- --nocapture
cargo run -p qf-app -- demo
cargo run -p qf-app -- sync-demo
```

[0.1.0-foundation]: https://github.com/MrWizard94-Compile/QuillForge/releases/tag/v0.1.0-foundation
