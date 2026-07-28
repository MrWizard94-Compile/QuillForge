# Changelog

All notable changes to Quillforge are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

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
