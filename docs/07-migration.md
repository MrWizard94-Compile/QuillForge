# Migration Specification

**Document ID:** QF-MIG-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Principles

1. Package format versions evolve; v1 is not forever.  
2. Migrations are explicit, versioned, testable, and reversible only via recorded inverse when provided.  
3. Unknown content MUST survive migrations (see unknown-content).  
4. Migration NEVER runs in-place on the only copy without LKG/backup.  
5. Prefer additive schema changes; destructive changes require multi-step migrations.

---

## 2. Version model

```rust
pub struct Version {
    pub major: u16,
    pub minor: u16,
    pub patch: u16,
}

// Package format version is independent from app semver.
// Example: package format 1.0.0, 1.1.0, 2.0.0
```

Stored in package metadata: `format_version`.

**Compatibility:**

| Case | Behavior |
|------|----------|
| `format == app_supported` | Open normally |
| `format < app` | Migrate along path to app version |
| `format > app` | Refuse open; require newer app |
| Missing version | Treat as unsupported/corrupt |

---

## 3. Required trait

```rust
pub trait Migration: Send + Sync {
    fn from_version(&self) -> Version;
    fn to_version(&self) -> Version;
    fn migrate(&self, input: Package) -> Result<Package, MigrationError>;
}
```

Optional capability:

```rust
pub trait ReversibleMigration: Migration {
    fn reverse(&self, input: Package) -> Result<Package, MigrationError>;
}
```

v1 does not require reverse for all migrations; where absent, document that downgrade is unsupported.

---

## 4. Migration graph

Migrations form a directed path of edges `from → to` with no cycles.

```rust
pub struct MigrationRegistry {
    edges: Vec<Arc<dyn Migration>>,
}

impl MigrationRegistry {
    pub fn plan(&self, from: Version, to: Version) -> Result<Vec<Arc<dyn Migration>>, MigrationError>;
}
```

**Rules:**

- `plan` finds a unique linear path (registry authoring ensures uniqueness).  
- If gap exists → `MigrationError::NoPath`.  
- Skipping edges is forbidden; always apply sequential migrations.

---

## 5. Package type for migration

```rust
pub struct Package {
    pub format_version: Version,
    pub metadata: PackageMetadata,
    pub checkpoint: Option<CheckpointBlob>,
    pub journal: JournalBlob,
    pub assets: BTreeMap<String, Vec<u8>>,
    pub other_files: BTreeMap<String, Vec<u8>>, // unknown members preserved
    pub manifest: Manifest,
}
```

**M1:** Migrations MUST copy unknown `other_files` entries unchanged unless a file is explicitly owned by a migrated subsystem.  
**M2:** Migrations MUST preserve `UnknownBlock` payloads.  
**M3:** Migrations MAY rewrite known structures; MUST update `format_version` to `to_version`.

---

## 6. Migration execution procedure

```
migrate_open(path, target_version):
  1. Integrity check package
  2. Ensure LKG exists (create if missing by copying primary)
  3. Load Package into memory or streaming builder
  4. plan(from, target)
  5. For each migration edge:
       a. input_hash = hash(package)
       b. package = migration.migrate(package)?
       c. validate package invariants
       d. assert package.format_version == edge.to
  6. Write to staging via atomic package write
  7. Verify open+replay on staging
  8. Atomic rename staging → primary
  9. Record migration audit event (versions, hashes) in package meta
```

**On failure at any step:** leave primary untouched; report error; keep staging for diagnostics if safe.

---

## 7. What migrations may and may not do

| Allowed | Forbidden |
|---------|-----------|
| Rewrite CBOR schema of known types | Drop unknown blocks |
| Split/merge known fields | Reorder journal ops |
| Introduce new required fields with defaults | Change op_seq of historical ops |
| Re-encode journal records to new schema **with 1:1 semantic map** | Invent new content ops silently |
| Move assets paths with manifest update | Decrypt without keys |

### Journal migration policy

**Preferred:** Keep historical journal bytes stable; only migrate checkpoint snapshot + set version. New ops use new schema.

**When journal record schema must change:** Provide `JournalRecordMigration` that maps each old record to new record **semantically equivalent**; verify hash-chain rebuild; this is a major migration and requires extensive tests.

---

## 8. Initial migrations (v1 baseline)

At first release, format version is `1.0.0` with empty migration path to itself.

Registry includes:

| ID | From | To | Purpose |
|----|------|----|---------|
| `noop_identity` | 1.0.0 | 1.0.0 | Test harness only |

Future example (documented now for process):

| ID | From | To | Purpose |
|----|------|----|---------|
| `m_1_0_0__1_1_0` | 1.0.0 | 1.1.0 | Add optional document meta field `language` |
| `m_1_1_0__2_0_0` | 1.1.0 | 2.0.0 | Block body enum tag rename with alias map |

---

## 9. Validation after migration

```rust
pub fn validate_migrated(pkg: &Package) -> Result<(), MigrationError> {
    // format version matches
    // manifest hashes
    // checkpoint load
    // journal decode
    // kernel replay
    // unknown blocks count >= pre-migration unknown count
    // asset hashes stable unless migration declared transforms
}
```

**Hard gate:** unknown block byte payloads identical pre/post unless migration explicitly targets that type (and type was known).

---

## 10. User experience

- Automatic migrate for minor/patch when safe and fast.  
- Major version: confirm dialog with backup note.  
- Progress UI for large packages.  
- Never auto-delete LKG after migrate until next successful open cycle completes.

---

## 11. Testing requirements

| Test | Detail |
|------|--------|
| Fixture matrix | Real packages at each version |
| Path plan | All pairs from→to through registry |
| Idempotence | Migrating already-at-target is no-op |
| Unknown preserve | Fixtures with UnknownBlock |
| Failure inject | Mid-migrate crash leaves primary intact |
| Property | Random documents survive additive migrations |

---

## 12. API surface (`qf-storage` / `qf-core`)

```rust
pub struct Migrator {
    registry: MigrationRegistry,
}

impl Migrator {
    pub fn needs_migration(&self, from: Version, to: Version) -> bool;
    pub fn migrate_package(&self, input: Package, to: Version) -> Result<Package, MigrationError>;
    pub fn migrate_file(&self, path: &Path, to: Version) -> Result<(), MigrationError>;
}
```

---

*End of migration specification.*
