# Delivery MANIFEST — Multi-page PDF + foundation release

**Title:** Multi-page PDF export and `v0.1.0-foundation` release packaging  
**Date:** 2026-07-28  
**Law:** AGENTS Constitution v4.1 + `operations/RELEASES.md` (REL-PACKAGE-001)

## Summary

1. **Multi-page PDF** — paginated deterministic PDF 1.4 (45 lines/page, max 450 lines).
2. **CHANGELOG.md** — Keep a Changelog entry for foundation release.
3. **README** — verify commands, law pointer, foundation tag note.
4. **Tag** — `v0.1.0-foundation` after suite green.

## Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo test --workspace budget_ -- --nocapture
cargo run -p qf-app -- demo
cargo run -p qf-app -- sync-demo
```

## Self-audit (CONST-GATE-001)

| # | Result |
|---|--------|
| Completeness | Pass |
| Zero warnings | Pass (pre-push) |
| Tests | Pass |
| Docs sync | Pass |
| Release packaging | Pass — CHANGELOG + tag |

## Deferred post-foundation

TLS/WebSocket WAN, rich DOCX/PDF typography, OS process-spawn kill harness.
