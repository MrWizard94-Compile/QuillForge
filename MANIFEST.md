# Delivery MANIFEST — Phase 1–2 Canonical Path

**Title:** Production-ready command → journal → kernel → `.qfdoc` session  
**Date:** 2026-07-27  
**Constitution:** AGENTS.md v2.0.0 (law)

## One-sentence summary

Quillforge now has a complete, tested, zero-warning durable editing session: dry-run commit, fsync journal, operation-based undo/redo, ZIP `.qfdoc` packages with LKG atomic saves, unknown-block byte preservation, and open/replay recovery.

## Files changed / added (purpose)

| Path | Purpose |
|------|---------|
| `Cargo.toml` | Workspace deps: ciborium, zip, tempfile |
| `Cargo.lock` | Locked dependency graph |
| `MANIFEST.md` | This delivery package |
| `DECISIONS.md` | Architecture decision record |
| `README.md` | Verification commands updated |
| `crates/qf-core/**` | CommitResult, next_seq, error taxonomy |
| `crates/qf-journal/**` | Durable journal, torn-frame truncate, RecordDraft |
| `crates/qf-kernel/**` | Pure apply, invert_ops, CBOR state hash, body codec |
| `crates/qf-doc/**` | AST + expand_command editing expansion |
| `crates/qf-storage/**` | DocumentSession, ZIP .qfdoc, atomic LKG, migration, recovery |
| `crates/qf-storage/tests/canonical_path.rs` | Phase 1–2 integration gates |
| `crates/qf-app/**` | CLI demo on DocumentSession |
| `crates/qf-telemetry/**` | Zero-warning telemetry defaults |

## Suggested commit message

```
Implement durable DocumentSession and .qfdoc package path.

Complete Phase 1–2: commit coordinator with dry-run, fsync journal,
operation-based undo/redo, ZIP packages with atomic LKG saves,
unknown-block preservation, and open/replay recovery. All tests and
clippy -D warnings pass under AGENTS.md Section 0.
```

## How to verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p qf-app -- demo
```

**Expected:**

- Clippy: clean exit  
- Tests: all pass (including 7 `canonical_path` integration tests)  
- Demo: prints preserved unknown payload, saves/reopens `.qfdoc`

## Known remaining risks / deliberate trade-offs

| Item | Status |
|------|--------|
| CBOR for journal frames | JSON frames in v0 journal; CBOR for package checkpoint/meta (deterministic). Journal can migrate later. |
| Nested block parents | Rejected with clear error; table cell nesting post-Phase-2 |
| Power-loss kill injection | Torn-frame + fsync contract tested; full process-kill harness is next hardening slice |
| Encryption at rest | Spec complete; not enabled in this slice |

## Self-audit (Section 0)

1. Completeness: session commit path + package I/O + undo + unknown + tests — **yes**  
2. Dependency-first: zip/ciborium/tempfile wired — **yes**  
3. Zero warnings: clippy `-D warnings` — **yes**  
4. Tests exist & pass — **yes**  
5. Docs synchronized: DECISIONS + MANIFEST + crate rustdocs — **yes**  
6. Security: no secrets; package path validation fail-closed — **yes**  
7. Performance: commit is serial single-doc; no gratuitous allocs on hot path beyond required clones for dry-run — **yes**  
8. Version fidelity: Rust 1.85, workspace lockfile — **yes**  
9. Full files delivered — **yes**  
10. Resource/cognitive packaging: MANIFEST review path — **yes**  
11. Determinism: SeqIdGen tests; content hash CBOR — **yes**  
12. IP/invention: N/A beyond product architecture already documented — **yes**  
13. Multi-agent: single delivery — **yes**  
14. Review packaging — **this file**  
15. Self-audit recorded — **yes**

## What the human should do next (priority)

1. Review `DECISIONS.md` and `crates/qf-storage/src/session.rs`  
2. Run verification commands above  
3. Optionally tag `phase-1-2-canonical`  
4. Next engineering slice: layout engine (Phase 3) or recovery kill-injection harness  

## Review path (optimal order)

1. `DECISIONS.md`  
2. `crates/qf-storage/src/session.rs`  
3. `crates/qf-journal/src/lib.rs`  
4. `crates/qf-kernel/src/lib.rs`  
5. `crates/qf-storage/src/zip_io.rs` + `package.rs`  
6. `crates/qf-storage/tests/canonical_path.rs`  
