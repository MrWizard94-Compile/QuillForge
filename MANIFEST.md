# Delivery MANIFEST — SOP rename

**Title:** Adopt SOP.md as process law (replaces WORKFLOW.md)  
**Date:** 2026-07-27  
**Law:** `AGENTS Constitution/AGENTS.md` + `AGENTS Constitution/SOP.md`

## Summary

Recognized rename of WORKFLOW.md → SOP.md. Updated all project pointers. Cleaned SOP structure (roles, procedure 1–10, quality controls) while preserving stages and adding Quillforge baseline + AGENTS gate linkage.

## Verify

```bash
# Docs only — optional full suite:
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Self-audit

References updated; no broken WORKFLOW.md links in entrypoints.

## Next

Continue green-light milestones (M9 plugin host) under SOP procedure step 7+.
