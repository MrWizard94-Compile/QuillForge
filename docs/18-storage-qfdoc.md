# Storage & `.qfdoc` Package Format Specification

**Document ID:** QF-STOR-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Package identity

- Extension: **`.qfdoc`**  
- MIME: `application/vnd.quillforge.document`  
- Container: **ZIP** (store or deflate), with normative internal layout  
- Endianness/encoding: UTF-8 paths; forward slashes  

**Decision:** ZIP for inspectability and tooling. Atomic replace updates whole package file.

---

## 2. Internal layout

```
document.qfdoc
├── mimetype                    # uncompressed first entry: application/vnd.quillforge.document
├── META-INF/
│   ├── manifest.xml            # or manifest.cbor — file list + hashes
│   └── signatures.xml          # optional
├── meta/
│   ├── package.cbor            # format_version, doc_id, encryption flags, migration history
│   └── document_meta.cbor      # title, authors, custom preserved
├── checkpoint/
│   ├── state.cbor              # CanonicalState snapshot
│   └── checkpoint_meta.cbor    # at_op_seq, snapshot_hash
├── journal/
│   └── ops.log                 # append-only framed records
├── assets/
│   └── **/*                    # binary assets by content-addressed name preferred
├── audit/
│   └── events.log              # optional
├── plugins/
│   └── **/*                    # optional embedded plugin data (not code execution from here alone)
└── extra/                      # unknown future members — preserved
```

---

## 3. Metadata

```rust
pub struct PackageMetadata {
    pub format_version: Version, // e.g. 1.0.0
    pub document_id: DocumentId,
    pub created_at: Timestamp,
    pub app_version_written: String,
    pub encryption: EncryptionInfo,
    pub migration_history: Vec<MigrationRecord>,
    pub content_seal: Hash256, // hash of canonical logical content
    pub custom: PreservedMap,
}

pub struct MigrationRecord {
    pub from: Version,
    pub to: Version,
    pub at: Timestamp,
    pub tool: String,
    pub input_hash: Hash256,
    pub output_hash: Hash256,
}
```

---

## 4. Document structure storage

- Checkpoint holds full `CanonicalState` CBOR.  
- Journal holds ops after checkpoint.  
- Open = load checkpoint + replay journal.  

---

## 5. History storage

- Primary history = journal.  
- Named revisions = entity table + optional snapshot blobs under `revisions/` (optional in v1).  
- Compaction rewrites checkpoint and truncates journal prefix after verification.

---

## 6. Asset storage

- Path: `assets/{asset_id_or_hash}.{ext}`  
- `AssetRef.content_hash` MUST match file bytes.  
- Dedup by hash allowed.  
- Large assets: still inside ZIP for v1; external attachments post-v1.

---

## 7. Versioning behavior

- `format_version` drives migration.  
- Readers MUST preserve `extra/**` and unknown meta keys.  
- Writers MUST NOT strip unknown ZIP members under `extra/`.

---

## 8. Integrity checks

| Check | When |
|-------|------|
| ZIP structure readable | Open |
| `mimetype` correct | Open |
| Manifest path list vs ZIP | Open |
| Per-file hashes | Open (full) / quick mode samples |
| Checkpoint hash | Open |
| Journal CRC + hash chain | Open |
| Asset hashes | On use + full verify |
| Optional signature | Enterprise policy |

```rust
pub struct ManifestEntry {
    pub path: String,
    pub size: u64,
    pub sha256: Hash256,
}
```

---

## 9. Atomic writes

See recovery spec. Writer builds new ZIP in temp file → fsync → rename replace → fsync dir → update LKG policy.

**Working journal during edit session:** MAY live as sidecar `document.qfdoc.journal` for performance, merged into package on save. Sidecar is durable with fsync; crash recovery reads package + sidecar.

```
Session durability model (chosen):
  - Live journal sidecar next to package during editing
  - Periodic checkpoint into sidecar or temp
  - Explicit Save folds sidecar into new .qfdoc atomically
  - Open recovers package checkpoint/journal + sidecar if newer
```

---

## 10. Unknown field preservation

- CBOR `PreservedMap` for meta  
- `extra/` ZIP members  
- `UnknownBlock` in checkpoint  

---

## 11. API

```rust
pub struct PackageStore;

impl PackageStore {
    pub fn create(path: &Path, meta: PackageMetadata) -> Result<DocumentSession>;
    pub fn open(path: &Path, opts: OpenOptions) -> Result<OpenOutcome>;
    pub fn save(&mut self, session: &DocumentSession) -> Result<()>;
    pub fn verify(path: &Path) -> Result<IntegrityReport>;
}
```

---

*End of storage specification.*
