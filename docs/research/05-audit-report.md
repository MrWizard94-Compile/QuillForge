# Stages 8–9: Codebase Audit & Hardening Report

**Date:** 2026-07-27  
**Scope:** Full workspace after research program + M2 implementation  
**Auditor:** Grok Build (AGENTS.md §0)

---

## Functional correctness

| Check | Result |
|-------|--------|
| Command → journal → kernel path | Pass — `DocumentSession` |
| Dry-run before journal | Pass — tests |
| Undo/redo op-based | Pass — integration |
| Save/open state hash | Pass — integration |
| Unknown block bytes | Pass — integration |
| Layout determinism hash | Pass — unit |
| Search rebuild equivalence | Pass — integration |
| Citation validation | Pass — unit |

## Security review

| Check | Result | Notes |
|-------|--------|-------|
| Sacred path not bypassed | Pass | No public kernel mutators outside apply |
| Package path traversal | Pass | `validate_member_name` |
| Zip bomb budgets | Pass | Entry/size limits |
| Quarantine non-clobber | Pass | `quarantine_copy` |
| Plugin FS denied | Pass | `raw_filesystem_access_allowed == false` |
| AI permission gate | Pass | Expired token rejected |
| Secrets in repo | Pass | None found in delivery |
| Telemetry content | Pass | Default off; metric enums only |

## Dependency / supply chain

| Crate | Justification |
|-------|----------------|
| serde, uuid, sha2, thiserror | Core serialization/IDs/errors |
| crc32c | Journal frame integrity |
| ciborium | Deterministic package CBOR |
| zip | `.qfdoc` container |
| tempfile | Safe unique temp paths |

Lockfile committed. No new network-calling deps.

## Test coverage (quality signal)

- Workspace tests green under `cargo test --workspace`
- Clippy `-D warnings` green
- Gaps remaining (tracked, not blocking M2): process-kill power-loss harness, GPU device-loss CI, full CRDT collab suite

## Logging / observability

- Structured domain errors via `thiserror`
- Telemetry opt-in only
- Gap: production `tracing` spans on commit path (post-M2 polish)

## Performance / reliability

- Commit path serial per document (correct)
- Dry-run clones state (acceptable; document if large-doc cost rises)
- Layout O(blocks) pure

## Hardening applied this pass

1. ZIP member path validation  
2. Uncompressed size / entry count budgets  
3. Quarantine copy on corrupt open (when LKG missing)  
4. Layout content_hash + richer block metrics  
5. Citation locator validation  
6. Search rebuild equivalence test  

## Residual risks

| Risk | Severity | Plan |
|------|----------|------|
| Process-kill fsync proof on Windows | Med | M2.1 harness |
| Journal still JSON frames | Low | Future migration |
| Collab CRDT not implemented | Med | Milestone M8 |
| No rate limit on sync (N/A offline) | Low | When network lands |

## Verdict

**PASS for M0+M2 delivery.** Sacred invariants hold. No high-severity open findings.
