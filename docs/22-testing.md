# Testing Strategy & Acceptance Criteria

**Document ID:** QF-TEST-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Philosophy

- Prefer tests that enforce **sacred invariants** over superficial UI snapshots.  
- Every durability claim has a fault-injection test.  
- Determinism tests run on multi-OS CI.  
- Unknown-content and migration tests are release blockers.

---

## 2. Test layers

| Layer | Location | Focus |
|-------|----------|-------|
| Unit | `src/**` modules | Pure functions, apply, encode |
| Integration | `crates/*/tests`, `tests/` | Package open/save, commit path |
| Replay | `qf-kernel` / journal | Fold equality |
| Recovery | `qf-storage` | Power loss, torn writes |
| Migration | `qf-storage` | Version fixtures |
| Layout | `qf-layout` | Golden hashes |
| Export | `qf-export` | Golden bytes |
| Performance | `xtask` / benches | Budgets |
| Fuzz/property | `fuzz/` | Ops, CBOR, CRDT |
| Plugin sandbox | `qf-plugin` | Caps, traps |
| Security | `qf-security` | Crypto, ACL |

---

## 3. Required test suites

### 3.1 Unit

- Operation apply matrix per block type  
- Mark split/merge  
- Offset projection  
- Inverse op generation, including full block-body/metadata restoration and accepted-suggestion marker reversal
- PreservedMap round-trip  
- Capability checks  

### 3.2 Integration

- Create → edit → save → open → state hash equal  
- Multi-command undo/redo  
- Asset attach integrity  
- Sidecar journal merge on save  

### 3.3 Replay

- Random op streams (seeded)  
- Checkpoint + partial journal replay  
- Hash chain verification  

### 3.4 Recovery

| Test ID | Description |
|---------|-------------|
| REC-01 | Kill after journal fsync |
| REC-02 | Kill before fsync |
| REC-03 | Torn trailing frame |
| REC-04 | Mid-staging package write |
| REC-05 | Corrupt checkpoint |
| REC-06 | Corrupt mid-journal |
| REC-07 | LKG fallback no clobber |
| REC-08 | Derived rebuild after recovery |

### 3.5 Migration

- Path planning  
- Fixture packages per version  
- Unknown payload identity  
- Failure leaves primary intact  

### 3.6 Layout

- Golden pages for fixture docs  
- Incremental == full  
- Cross-OS hash (pinned fonts)  

### 3.7 Export

- MD/HTML/TXT/PDF/DOCX goldens  
- Unknown block markers present  

### 3.8 Performance

| Budget | Gate |
|--------|------|
| Single op apply p95 | < 16ms |
| Open 100MB cold | < 3s |
| Dirty reflow 50 pages p95 | < 50ms |
| Token search 10k blocks p95 | < 50ms |
| Scroll | 60 FPS sustained target |

### 3.9 Fuzz / property

- `proptest` on apply: invariants hold or clean error  
- CBOR unknown round-trip  
- CRDT merge commutativity for text ops  
- Journal frame parser  

### 3.10 Plugin sandbox

- Deny FS hostcalls  
- Deny missing capabilities  
- Trap isolation  
- Custom block remove/reinstall  

### 3.11 Security

- Wrong passphrase  
- Tamper AEAD  
- ACL matrix  
- AI without permission token  

---

## 4. Quality gates (merge/release)

| Gate | Requirement |
|------|-------------|
| Deterministic replay | State hash match on replay |
| Atomic save recovery | REC-04 pass |
| Unknown-block round trip | Byte identity |
| Migration correctness | All fixtures |
| Export fidelity | Goldens |
| Collaboration convergence | Two-replica tests |
| Performance budgets | Benches under limits |
| `cargo xtask check` | fmt, clippy -D warnings, test |

---

## 5. CI matrix

- OS: Windows, macOS, Linux  
- `cargo test --workspace`  
- Recovery tests enabled on all  
- GPU tests: headless where possible; optional GPU job  

---

## 6. Fixture policy

- Store under `fixtures/` with LICENSE notes  
- Corrupt fixtures clearly named  
- Never regenerate goldens without review  

---

## 7. Acceptance criteria mapping

| Feature | Acceptance |
|---------|------------|
| Canonical path | Static check + integration: no public kernel mutator outside coordinator |
| Journal | Durability tests |
| Recovery | REC-01..08 |
| Migration | Fixture matrix |
| Unknown | U1–U5 tests |
| Editing | Manual + scripted input |
| Undo | State hash restore |
| Layout/render | Goldens + smoke GPU |
| Export | Goldens |
| Search | Rebuild equivalence |
| Collab | Convergence |
| Plugins | Sandbox suite |
| AI | Permission suite |
| Security | Crypto + ACL suite |

---

*End of testing specification.*
