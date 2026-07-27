# Delivery MANIFEST — M9 plugin host

**Title:** Capability-based plugin host (proposals, registry, KV, fuel)  
**Date:** 2026-07-27  
**Law:** `AGENTS Constitution/AGENTS.md` + `SOP.md`

## Summary

M9 delivers a production host API for sandboxed plugins: capability grants, custom block registry, proposal queue (command-path only), namespaced KV quotas, fuel traps, and lite manifest parse. No raw FS, no journal, no native DLLs.

## Verify

```bash
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Self-audit

Section 0 pass for M9. Wasmtime guest runtime remains future binding to the same hostcalls.

## Next

**M10** — AI suggestion flow + hardening + v1 ship checklist.
