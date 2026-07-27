//! `.qfdoc` package format, atomic writes, recovery, and migrations.
//!
//! See docs/18-storage-qfdoc.md and docs/06-recovery.md.

use qf_core::{
    DocumentId, Hash256, MigrationError, StorageError, Version,
};
use qf_kernel::{CanonicalState, DocumentSnapshot, Kernel};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Package in-memory representation used by migrations and open/save.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Package {
    pub format_version: Version,
    pub metadata: PackageMetadata,
    pub checkpoint: Option<CheckpointBlob>,
    pub journal_bytes: Vec<u8>,
    pub assets: BTreeMap<String, Vec<u8>>,
    /// Unrecognized members preserved verbatim.
    pub other_files: BTreeMap<String, Vec<u8>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub format_version: Version,
    pub document_id: DocumentId,
    pub created_at_unix_ms: i64,
    pub app_version_written: String,
    pub migration_history: Vec<MigrationRecord>,
    pub content_seal: Hash256,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MigrationRecord {
    pub from: Version,
    pub to: Version,
    pub at_unix_ms: i64,
    pub tool: String,
    pub input_hash: Hash256,
    pub output_hash: Hash256,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointBlob {
    pub at_op_seq: u64,
    pub snapshot: DocumentSnapshot,
}

/// Versioned package migration.
pub trait Migration: Send + Sync {
    fn from_version(&self) -> Version;
    fn to_version(&self) -> Version;
    fn migrate(&self, input: Package) -> Result<Package, MigrationError>;
}

pub struct MigrationRegistry {
    edges: Vec<Box<dyn Migration>>,
}

impl MigrationRegistry {
    pub fn new() -> Self {
        Self { edges: vec![] }
    }

    pub fn register(&mut self, m: Box<dyn Migration>) {
        self.edges.push(m);
    }

    pub fn plan(
        &self,
        from: Version,
        to: Version,
    ) -> Result<Vec<&dyn Migration>, MigrationError> {
        if from == to {
            return Ok(vec![]);
        }
        // Linear unique path search (v1: small graph).
        let mut path = Vec::new();
        let mut cur = from;
        while cur != to {
            let edge = self
                .edges
                .iter()
                .find(|e| e.from_version() == cur)
                .ok_or_else(|| MigrationError::NoPath {
                    from: from.to_string(),
                    to: to.to_string(),
                })?;
            let next = edge.to_version();
            path.push(edge.as_ref());
            cur = next;
            if path.len() > 64 {
                return Err(MigrationError::Failed("migration path too long".into()));
            }
        }
        Ok(path)
    }
}

impl Default for MigrationRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Migrator {
    pub registry: MigrationRegistry,
    pub target: Version,
}

impl Migrator {
    pub fn needs_migration(&self, from: Version) -> bool {
        from != self.target
    }

    pub fn migrate_package(&self, mut input: Package) -> Result<Package, MigrationError> {
        let plan = self.registry.plan(input.format_version, self.target)?;
        for m in plan {
            let before_unknown = count_unknown_hint(&input);
            input = m.migrate(input)?;
            if input.format_version != m.to_version() {
                return Err(MigrationError::Validation(
                    "migration did not set to_version".into(),
                ));
            }
            let after_unknown = count_unknown_hint(&input);
            if after_unknown < before_unknown {
                return Err(MigrationError::Validation(
                    "migration dropped unknown content".into(),
                ));
            }
        }
        Ok(input)
    }
}

fn count_unknown_hint(pkg: &Package) -> usize {
    // Foundation heuristic: count other_files + checkpoint presence.
    pkg.other_files.len()
        + pkg
            .checkpoint
            .as_ref()
            .map(|_| 1usize)
            .unwrap_or(0)
}

/// Atomic write: temp file → fsync → rename.
pub fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| StorageError::Io(e.to_string()))?;
    let staging = path.with_extension(format!(
        "tmp-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    {
        let mut f = fs::File::create(&staging).map_err(|e| StorageError::Io(e.to_string()))?;
        use std::io::Write;
        f.write_all(bytes).map_err(|e| StorageError::Io(e.to_string()))?;
        f.sync_all().map_err(|e| StorageError::Io(e.to_string()))?;
    }
    // LKG: if target exists, copy to .lkg before replace.
    if path.exists() {
        let lkg = lkg_path(path);
        let _ = fs::copy(path, &lkg);
    }
    fs::rename(&staging, path).map_err(|e| StorageError::Io(e.to_string()))?;
    Ok(())
}

pub fn lkg_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".lkg");
    PathBuf::from(s)
}

#[derive(Debug)]
pub enum OpenOutcome {
    Healthy { kernel: Kernel },
    Corrupt { message: String },
    NeedsMigration { from: Version, to: Version },
}

pub struct PackageStore {
    pub format_version: Version,
}

impl Default for PackageStore {
    fn default() -> Self {
        Self {
            format_version: Version::V1_0_0,
        }
    }
}

impl PackageStore {
    /// Save canonical snapshot as a simple foundation package (JSON wrapper).
    /// Production uses ZIP layout per storage spec; foundation proves atomic path.
    pub fn save_snapshot(&self, path: &Path, state: &CanonicalState) -> Result<(), StorageError> {
        let snap = DocumentSnapshot::from_state(state.clone());
        let pkg = Package {
            format_version: self.format_version,
            metadata: PackageMetadata {
                format_version: self.format_version,
                document_id: state.document_id,
                created_at_unix_ms: 0,
                app_version_written: "0.0.0".into(),
                migration_history: vec![],
                content_seal: snap.snapshot_hash,
            },
            checkpoint: Some(CheckpointBlob {
                at_op_seq: state.op_seq.0,
                snapshot: snap,
            }),
            journal_bytes: vec![],
            assets: BTreeMap::new(),
            other_files: BTreeMap::new(),
        };
        let bytes = serde_json::to_vec_pretty(&pkg).map_err(|e| StorageError::Io(e.to_string()))?;
        write_bytes_atomic(path, &bytes)
    }

    pub fn open_snapshot(&self, path: &Path) -> Result<OpenOutcome, StorageError> {
        let bytes = fs::read(path).map_err(|e| StorageError::Io(e.to_string()))?;
        let pkg: Package =
            serde_json::from_slice(&bytes).map_err(|e| StorageError::Corrupt(e.to_string()))?;
        if pkg.format_version > self.format_version {
            return Ok(OpenOutcome::NeedsMigration {
                from: pkg.format_version,
                to: self.format_version,
            });
        }
        if pkg.format_version < self.format_version {
            return Ok(OpenOutcome::NeedsMigration {
                from: pkg.format_version,
                to: self.format_version,
            });
        }
        let Some(cp) = pkg.checkpoint else {
            return Ok(OpenOutcome::Corrupt {
                message: "missing checkpoint".into(),
            });
        };
        let kernel = Kernel::from_snapshot(cp.snapshot)
            .map_err(|e| StorageError::Corrupt(e.to_string()))?;
        Ok(OpenOutcome::Healthy { kernel })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::{ActorId, IdGen, SectionId, SeqIdGen, Timestamp};
    use qf_doc::Document;
    use qf_kernel::EntityTables;
    use std::env::temp_dir;

    #[test]
    fn atomic_save_open_roundtrip() {
        let mut gen = SeqIdGen::default();
        let doc_id = DocumentId::new(&mut gen);
        let root = SectionId::new(&mut gen);
        let actor = ActorId::new(&mut gen);
        let document = Document::empty(doc_id, root, actor, Timestamp { unix_ms: 0 });
        let state = CanonicalState {
            document_id: doc_id,
            package_format_version: Version::V1_0_0,
            op_seq: qf_core::OpSeq::ZERO,
            document,
            entities: EntityTables::default(),
        };
        let path = temp_dir().join(format!("qf-pkg-{}.qfdoc", gen.new_uuid()));
        let store = PackageStore::default();
        store.save_snapshot(&path, &state).unwrap();
        match store.open_snapshot(&path).unwrap() {
            OpenOutcome::Healthy { kernel } => {
                assert_eq!(kernel.state().document_id, doc_id);
            }
            other => panic!("unexpected {other:?}"),
        }
        let _ = fs::remove_file(&path);
        let _ = fs::remove_file(lkg_path(&path));
    }
}
