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

---

## ADR-011 — Competitive research synthesis (2026-07-27)

**Status:** Accepted  
**Decision:** Product strategy absorbs: Obsidian ownership, Logseq block atoms, Notion review roles, AFFiNE/Yjs presence split + CRDT, ProseMirror transactional discipline, Automerge history stance, Zotero citations, Typst determinism, Anytype encryption posture—without cloud-only lock-in or unrestricted native plugins.  
**Why:** Ten-project study (`docs/research/01-ten-projects.md`).  
**Consequence:** Engineering milestones M2+ prioritize package untrusted-input hardening, layout hashes, citation validation, search rebuild equivalence.

---

## ADR-012 — Untrusted package budgets

**Status:** Accepted  
**Decision:** ZIP open enforces max entries (100k), max total uncompressed (512 MiB), max entry (256 MiB), and rejects path traversal. Corrupt primaries may be quarantine-copied without overwrite.  
**Why:** Zip-bomb / path traversal class of attacks on local document apps.

---

## ADR-013 — QFENC1 at-rest package encryption

**Status:** Accepted  
**Decision:** Encrypted `.qfdoc` files use magic `QFENC1`, Argon2id KDF (m/t/p stored in header), XChaCha20-Poly1305 AEAD over the ZIP package bytes with AAD = magic. Open requires passphrase via `OpenOptions`.  
**Why:** Local-first documents may hold secrets; AEAD detects tampering; Argon2id resists offline passphrase attack better than a bare hash.  
**Consequence:** Random salt/nonce → ciphertext non-deterministic (plaintext seals remain deterministic). Tests use reduced Argon2 memory.

---

## ADR-014 — Minimal deterministic PDF export

**Status:** Accepted  
**Decision:** v1 PDF export is PDF 1.4, fixed Helvetica, US Letter, plain-text layout of export lines — no external fonts, no images. Same document state yields identical bytes.  
**Why:** Ship a real binary PDF without font subsetting complexity; full typography deferred.

---

## ADR-015 — Minimal deterministic OOXML DOCX export

**Status:** Accepted  
**Decision:** DOCX export is a minimal valid Office Open XML ZIP (`[Content_Types].xml`, `_rels/.rels`, `word/document.xml`) with fixed ZIP timestamps and deflate, one paragraph per plain-export line.  
**Why:** Interop with Word/LibreOffice without full style/theme model.  
**Consequence:** Rich formatting, images, and styles deferred.

---

## ADR-016 — Wasmtime guest plugins with multiplexed hostcalls

**Status:** Accepted  
**Decision:** Plugin guests execute under Wasmtime with fuel. Imports are `qf.host_call` / `scratch_len` / `scratch_copy` only; no FS/network/journal. Hostcalls re-enter `PluginHost` for KV, type registration, and proposals.  
**Why:** Spec isolation without component-model complexity in foundation v1.  
**Consequence:** Guests must be written/compiled to this ABI; WIT/component model deferred.

---

## ADR-017 — In-process SyncHub as foundation network server

**Status:** Accepted  
**Decision:** Multi-replica sync is provided by `qf_sync::SyncHub` implementing Hello / Pull / Push and a separate presence stream. Offline durability uses `DurableSyncQueue` with magic `QFSYNCQ1` and fsync atomic writes. Content still enters canonical state only via command/journal/kernel after merge.  
**Why:** Closes the network residual for local-first multi-device testing without requiring a production TLS deployment in foundation.  
**Consequence:** A thin HTTPS/WebSocket adapter can wrap the same protocol later; hub is the source of truth for logical protocol tests.

---

## ADR-018 — Synthetic power-loss matrix is binding for v1 durability

**Status:** Accepted  
**Decision:** Durability evidence is synthetic: journal fsync + handle-forget reopen, torn/partial frames, incomplete staging, LKG recovery, encrypted open, durable queue reopen. Full OS process-kill spawn harness is optional, not release-blocking.  
**Why:** Cross-OS process kill harnesses are flaky and expensive; fsync contracts + reopen tests prove the same invariants.

---

## ADR-019 — Localhost TCP framed transport (QFSNC1) over SyncHub

**Status:** Accepted  
**Decision:** Real OS sockets speak length-prefixed CBOR frames (`QFSNC1`) for Hello/Pull/Push. Payload is CBOR (not JSON) so TextCrdt maps with structured keys serialize. Max frame 16 MiB. One request/response per connection. TLS/WebSocket is a future adapter over the same logical protocol.  
**Why:** Closes the “network residual” with real concurrency over the network stack without enterprise TLS complexity in foundation.

---

## ADR-020 — CI-gated performance budgets (`budget_` tests)

**Status:** Accepted  
**Decision:** Performance budgets live in `docs/26-performance-budgets.md` and are enforced by tests named `budget_*` in layout, journal, export, sync, and security crates. CI and `cargo xtask check`/`perf` run `cargo test --workspace budget_`. Limits are generous debug-CI ceilings that still catch algorithmic regressions.  
**Why:** Closes D26 without flaky micro-benchmarks; operational load under wall-clock caps.

---

## ADR-021 — Multi-page deterministic PDF pagination

**Status:** Accepted  
**Decision:** PDF export paginates plain-export lines at 45 lines/page (US Letter, Helvetica 12, TL 14), up to 450 lines total. Shared font object; one content stream per page; byte-stable for identical input.  
**Why:** Long documents must not truncate silently at 60 lines; multi-page still avoids font subsetting complexity.

---

## ADR-022 — WPAI studio branding for Quillforge

**Status:** Accepted  
**Decision:** Quillforge is branded as a **WPAI** (Wizard Productions AI Studio / **W.P.A.I**) Software product. Product name stays Quillforge. Studio voice, thesis, and forge palette are codified in `qf_core::brand`, default UI theme `Theme::wpai_forge_dark`, and repository `brand/`. Package metadata and CLI display studio attribution.  
**Why:** Portfolio consistency with Wizard Productions AI Studio; transparent AI-assisted, human-directed positioning.
