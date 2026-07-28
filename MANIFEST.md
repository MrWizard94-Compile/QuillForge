# Delivery MANIFEST — Structured DOCX + constitution birth law

**Title:** AST DOCX export; LIFE-BIRTH-001 RepoForge attach  
**Date:** 2026-07-28  
**Law:** AGENTS Constitution v5.0.0

## Summary

1. Constitution: LIFE-BIRTH-001, ADOPT Option D (RepoForge), registry + Level-4 pointer.  
2. DOCX: block-structured export (headings, quotes, code styles), styles.xml, deterministic ZIP.  
3. ADR-024.

## Verify

```bash
pwsh -File "AGENTS Constitution/tools/verify-pack.ps1"
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Self-audit

CONST-GATE-001 pass.
