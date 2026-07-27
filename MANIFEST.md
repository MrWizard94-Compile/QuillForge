# Delivery MANIFEST — M8 collab CRDT + stream split

**Title:** Text CRDT convergence and presence/content stream separation  
**Date:** 2026-07-27  
**Law:** `AGENTS Constitution/AGENTS.md` + `WORKFLOW.md`

## Summary

M8 delivers offline concurrent text editing via an RGA-style `TextCrdt` (merge → kernel ops) and hard-separates ephemeral presence from durable content streams.

## Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Commits

1. `Implement RGA-style text CRDT for offline concurrent editing.`  
2. `Separate content and presence sync streams with CRDT merge.`  
3. Docs/lockfile update  

## Self-audit

Section 0 pass for M8: complete APIs, convergence tests, presence emits zero ops, clippy clean.

## Next

**M9** plugin WASM host capabilities (production host still capability-enforced stub → deepen hooks) or full remote batch through DocumentSession.
