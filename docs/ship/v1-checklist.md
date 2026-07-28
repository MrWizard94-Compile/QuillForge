# Quillforge v1 Ship Checklist (M10)

**Document ID:** QF-SHIP-001  
**SOP:** Procedure §9–10  
**Constitution:** AGENTS Section 0  

Use this checklist before tagging a v1 release. Evidence points to automated tests where available.

---

## Canonical core

| ID | Criterion | Evidence | Status |
|----|-----------|----------|--------|
| D1 | Command → Ops → Journal → Kernel | `DocumentSession`, e2e/canonical tests | Met |
| D2 | No out-of-band mutation | Crate boundaries; plugin/AI propose only | Met |
| D3 | Deterministic journal replay | Journal + open/save hash tests | Met |
| D4 | Op-based undo/redo | `edit_e2e`, session undo | Met |
| D5 | Dry-run before journal | `dry_run_failure_does_not_advance_journal` | Met |

## Durability & recovery

| ID | Criterion | Evidence | Status |
|----|-----------|----------|--------|
| D6 | Committed ops survive fsync | Journal fsync + `rec_commit_survives_handle_forget` | Met |
| D7 | Atomic package write | `atomic_write_creates_and_lkg` | Met |
| D8 | Open from journal + checkpoint | `create_edit_save_open_preserves_state` | Met |
| D9 | Derived rebuild after open | Layout/render rebuild from state | Met (search async optional) |
| D10 | Corrupt never clobbers LKG | quarantine + LKG open path | Met |

## Evolution & unknown

| ID | Criterion | Evidence | Status |
|----|-----------|----------|--------|
| D11 | Migrations | `Migration` trait + registry | Met (empty path at 1.0.0) |
| D12–D14 | Unknown blocks | unknown round-trip tests; plugin unload registry | Met |

## Product features

| ID | Criterion | Evidence | Status |
|----|-----------|----------|--------|
| D15 | Editing offline | EditorController e2e | Met |
| D16 | Layout | Golden fixtures | Met |
| D17 | Rendering | qf-render headless tests | Met (headless GPU abstraction) |
| D18 | Export | MD/HTML/TXT + multi-page PDF + structured DOCX styles | Met |
| D19 | Search | rebuild_equivalence | Met (token; semantic optional) |
| D20 | Collab | presence + CRDT + suggestions | Met (foundation) |
| D21 | Sync | durable queue + SyncHub + TCP + TLS (rustls) + presence | Met |
| D22 | Plugins | capability host + Wasmtime guest + command_path | Met |
| D23 | AI suggestions | accept_path + permission tests | Met |
| D24 | Security | ACL, audit, tokens, budgets, QFENC1 encrypt | Met |

## Quality

| ID | Criterion | Evidence | Status |
|----|-----------|----------|--------|
| D25 | Automated tests | `cargo test --workspace` | Met |
| D26 | Performance budgets | `docs/26-performance-budgets.md` + `budget_*` CI tests | Met |
| D27 | Risk register | docs/23 + residual notes | Accepted residuals documented |
| D28 | Docs match APIs | docs/ + DECISIONS + ship checklist | Met |

---

## Residual risks accepted for foundation v1 tag

1. ~~Process-kill matrix~~ **Closed (synthetic)** — handle-forget, partial frames, LKG, encrypted open, durable sync queue reopen; OS process spawn harness optional.  
2. ~~PDF/DOCX text stand-ins~~ **Closed** — minimal deterministic PDF 1.4 and OOXML DOCX.  
3. ~~Wasmtime not wired~~ **Closed** — Wasmtime guest runtime with fuel + hostcalls (`qf-plugin::wasm_runtime`).  
4. ~~Network sync server~~ **Closed** — SyncHub + TCP + **TLS (rustls)** QFSNC1; WebSocket WAN optional later.  
5. ~~Argon2id package encryption~~ **Closed** — QFENC1 (Argon2id + XChaCha20-Poly1305) save/open productized.

---

## Release commands

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace budget_ -- --nocapture
cargo run -p qf-app -- demo
cargo run -p qf-app -- sync-demo
cargo run -p qf-app -- version
# or: cargo xtask check
```

Tag: **`v0.1.0-foundation`** (foundation DoD met; marketing v1 may still add rich export styles / TLS WAN).

## Rollback

Previous git tag/commit on `main`; packages remain format `1.0.0`.
