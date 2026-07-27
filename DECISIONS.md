# Architecture Decisions (ADR log)

Living record of decisions that bind implementers. Complements `docs/` specs.

---

## ADR-001 — Canonical mutation path only

**Status:** Accepted  
**Decision:** All canonical mutations flow `Command → Operations → Journal → Kernel`.  
**Why:** Data integrity, crash recovery, auditability, deterministic replay.  
**Consequence:** UI, plugins, AI, search, layout never write document state directly.

---

## ADR-002 — Dry-run before journal

**Status:** Accepted  
**Decision:** `DocumentSession` dry-runs ops on a kernel clone before any journal write.  
**Why:** Prevents journal poison (invalid ops that can never apply).  
**Consequence:** Slight CPU cost (clone + apply); correctness over convenience.

---

## ADR-003 — Operation-based undo

**Status:** Accepted  
**Decision:** Undo commits inverse ops as new journal records; stacks hold inverse/forward batches, never full snapshots.  
**Why:** Matches journal model; history remains auditable; no snapshot bloat.  
**Consequence:** Journal grows on undo; invert must be maintained for each op kind.

---

## ADR-004 — Full journal always stored in package

**Status:** Accepted  
**Decision:** Save writes checkpoint **and** complete journal bytes. Open loads checkpoint, replays only ops after `at_op_seq`, and restores the full journal for continued append.  
**Why:** Keeps `journal.last_seq == kernel.op_seq` without synthetic post-fold chain breaks.  
**Consequence:** Packages grow with history until a future compaction migration.

---

## ADR-005 — `.qfdoc` is ZIP + CBOR members

**Status:** Accepted  
**Decision:** Container is ZIP; metadata/checkpoint use CBOR; journal uses length-prefixed JSON frames with CRC32C.  
**Why:** ZIP inspectability; CBOR compact deterministic snapshots; JSON journal is debuggable and already fsync-proven.  
**Trade-off:** Dual encodings until a journal-frame migration to CBOR.

---

## ADR-006 — Atomic save with LKG

**Status:** Accepted  
**Decision:** Write staging file → fsync → copy previous primary to `.lkg` → replace primary. Never overwrite primary without LKG when a previous version exists.  
**Why:** Corruption and crash must not destroy last good state.  
**Consequence:** Extra disk space for LKG; Windows requires delete-before-rename.

---

## ADR-007 — Unknown content is opaque bytes

**Status:** Accepted  
**Decision:** Unregistered block types become `UnknownBlock { original_type, original_payload, metadata }` with byte-identical round trips.  
**Why:** Plugin lifecycle and forward compatibility.  
**Consequence:** No partial edit of unknown interiors without capability.

---

## ADR-008 — Kernel apply is pure

**Status:** Accepted  
**Decision:** Apply has no I/O, no RNG, no wall-clock branching. IDs allocated only at command expansion time.  
**Why:** Deterministic replay across machines.  
**Consequence:** Handlers must pre-generate all entity IDs.

---

## ADR-009 — WASM plugins only (v1)

**Status:** Accepted (spec; host stub present)  
**Decision:** No third-party native DLLs in v1.  
**Why:** Security priority over extensibility convenience.

---

## ADR-010 — Fixed-point layout (spec)

**Status:** Accepted for layout phase  
**Decision:** Layout uses Fixed32 (1/64 pt).  
**Why:** Cross-platform float variance threatens determinism.
