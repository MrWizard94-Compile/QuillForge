# Recovery Specification

**Document ID:** QF-REC-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Hard requirements

| ID | Requirement |
|----|-------------|
| R1 | Every committed operation survives power loss |
| R2 | Every package write is atomic |
| R3 | Recovery reconstructs from **journal + last checkpoint + package metadata** |
| R4 | Derived caches are rebuilt after canonical recovery |
| R5 | Corruption MUST NEVER silently overwrite last-known-good state |

---

## 2. Recovery sources

| Source | Path (inside/ beside package) | Role |
|--------|-------------------------------|------|
| Package metadata | `meta/package.json` / CBOR | Format version, doc id, flags |
| Checkpoint | `checkpoint/state.cbor` | Fast baseline at `at_op_seq` |
| Journal | `journal/ops.log` | Ops after checkpoint |
| Manifest | `MANIFEST` | File list + hashes |
| LKG backup | `*.qfdoc.lkg` sibling file | Last successful open/save snapshot of package |
| Staging | `*.qfdoc.tmp` / `*.qfdoc.partial` | Incomplete writes; never treated as primary |

---

## 3. Atomic write strategy

### 3.1 Package replace algorithm

```
write_package_atomic(target_path, build_fn):
  1. staging = target_path + ".tmp-" + random
  2. Create staging directory/file container
  3. build_fn writes all members into staging
  4. Write MANIFEST with content hashes
  5. fsync all file data
  6. fsync staging directory
  7. Compute package seal hash; write SEAL
  8. fsync SEAL
  9. If target exists:
       copy/rename target → target.lkg (or retain previous .lkg)
       fsync
  10. atomic rename staging → target  (ReplaceFile/rename)
  11. fsync parent directory
  12. Delete obsolete temps only after success
```

**On Windows:** use `MoveFileEx` with `MOVEFILE_REPLACE_EXISTING` after ensuring destination backup policy. Prefer write-new + rename over in-place mutation.

### 3.2 Journal append algorithm

```
append_records(records):
  1. For each record: encode length-prefixed frame + crc32c
  2. write(all frames)
  3. fsync(journal_fd)
  4. Optionally write commit mark frame + fsync
  5. Return success
```

**Torn write detection:** If last frame CRC fails or length exceeds EOF, truncate to last good frame boundary during recovery (only trailing incomplete frame). Never truncate committed complete frames.

### 3.3 Checkpoint write algorithm

```
write_checkpoint(state):
  1. Write state to checkpoint/state.cbor.tmp
  2. fsync
  3. Write checkpoint/meta with at_op_seq + hash
  4. fsync
  5. rename tmp → state.cbor
  6. fsync dir
  7. Update package pointer only after verify load
```

---

## 4. Open / recovery procedure

```
open_document(path) -> Result<Session>:
  A. Locate primary package
  B. Verify MANIFEST/SEAL if present
  C. Read metadata (format version)
  D. If version > supported: error UnsupportedVersion
  E. If version < current: plan migration (see migration spec); may require user confirm
  F. Load checkpoint if valid
  G. Open journal; scan; verify hash chain from checkpoint
  H. Truncate only incomplete trailing frame if needed (log event)
  I. Replay ops after checkpoint into kernel
  J. If any step fails → recovery_failed path (section 6)
  K. Rebuild derived caches (layout/search) asynchronously
  L. Mark session healthy
```

### 4.1 Successful recovery definition

Canonical state equals fold(checkpoint, journal_ops). `op_seq` matches last journal record. Derived systems empty or rebuilding.

---

## 5. Crash scenarios and expected outcomes

| Scenario | Outcome |
|----------|---------|
| Power loss after journal fsync, before UI ack | On reopen, op present |
| Power loss mid-journal write | Trailing torn frame discarded; prior ops intact |
| Power loss mid-package staging | Staging ignored; primary unchanged |
| Power loss after staging complete, before rename | Primary unchanged; staging cleaned on next start |
| Power loss after rename to primary | New package opens |
| Power loss during LKG creation | Primary still valid; LKG may be missing |
| Kernel apply bug after journal | Recovery mode; rebuild from journal; if apply fails, quarantine |
| Bitrot in checkpoint | Discard checkpoint; replay from genesis or older checkpoint |
| Bitrot in journal middle | Fail integrity; do not open as healthy; offer LKG |

---

## 6. Corrupted package handling

**Policy: fail closed, preserve evidence.**

```
recovery_failed:
  1. Do NOT write over primary package
  2. Copy primary to quarantine: package.qfdoc.corrupt-<timestamp>
  3. Attempt open of sibling .lkg if present
  4. If LKG opens: present UI "Recovered from last known good"; primary remains quarantined
  5. If not: attempt salvage:
       - read journal only
       - read checkpoint only
       - partial export of recoverable CBOR
  6. User must explicitly choose salvage write to NEW path
  7. Never auto-replace primary with salvage
```

---

## 7. Last-known-good (LKG)

| Property | Rule |
|----------|------|
| When written | After successful open+verify and after successful full save |
| Retention | Keep 1 LKG by default (configurable 1..=3) |
| Location | Same directory as package: `name.qfdoc.lkg` |
| Encryption | Same policy as primary if at-rest encryption enabled |
| Auto-use | Only when primary fails integrity; never silently on success |

---

## 8. Derived cache rebuild

After canonical recovery:

1. Drop layout cache, render atlas pages, search index segments for that document.  
2. Emit `KernelEvent::DocumentHydrated`.  
3. Layout/search rebuild from snapshot + events.  
4. UI may show progressive readiness; editing waits until kernel ready (search may lag).

**Invariant:** Derived rebuild failure MUST NOT mark document corrupt if canonical is healthy.

---

## 9. Journal compaction recovery rules

Compaction creates new package generation:

1. Build new checkpoint at seq S.  
2. Build new journal empty or with only ops after policy (usually empty if checkpoint includes all).  
3. Atomic replace package.  
4. Retain previous generation as LKG until next successful open.

If crash during compaction: primary old package remains; temp discarded.

---

## 10. Recovery API

```rust
pub struct RecoveryService;

impl RecoveryService {
    pub fn open(path: &Path, opts: OpenOptions) -> Result<OpenOutcome, RecoveryError>;
    pub fn verify_integrity(path: &Path) -> IntegrityReport;
    pub fn salvage(path: &Path, out: &Path) -> Result<SalvageReport, RecoveryError>;
}

pub enum OpenOutcome {
    Healthy { session: DocumentSession },
    RecoveredFromJournal { session: DocumentSession, notes: Vec<RecoveryNote> },
    RecoveredFromLkg { session: DocumentSession, lkg_path: PathBuf },
    NeedsMigration { from: Version, to: Version },
    Corrupt { report: IntegrityReport, quarantine_path: Option<PathBuf> },
}
```

---

## 11. Acceptance tests (recovery)

| Test | Pass criteria |
|------|---------------|
| `powerloss_after_fsync` | Op durable |
| `powerloss_before_fsync` | Op absent; prior state intact |
| `torn_trailing_frame` | Truncate; open healthy |
| `atomic_save_mid_staging` | Old package opens |
| `corrupt_checkpoint` | Replay from genesis/older |
| `corrupt_mid_journal` | No silent open; quarantine |
| `lkg_fallback` | Opens LKG; primary not overwritten |
| `derived_rebuild` | Search/layout rebuild post open |

---

*End of recovery specification.*
