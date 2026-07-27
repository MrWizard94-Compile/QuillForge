# Delivery MANIFEST — M6 export citations + M7 render

**Title:** Citation export markers and headless layout→GPU composition  
**Date:** 2026-07-27  
**Law:** `AGENTS Constitution/AGENTS.md` + `WORKFLOW.md`

## Summary

M6: deterministic markdown/HTML export with citation markers, source entity codec, and bibliography.  
M7: `RenderScheduler` composes layout fragments with viewport culling, tile tracking, kind colors, and device-loss recovery — never touches kernel/journal.

## Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Commits this turn

1. `Add citation markers and bibliography to deterministic export.`  
2. (this) render scheduler + docs  

## Self-audit

Section 0 pass for M6/M7: complete, zero warnings, tests green, docs updated.

## Next

**M8** — collab CRDT text mode / presence stream separation tests.
