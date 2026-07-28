//! In-memory package model and store open/save orchestration.

use crate::atomic::lkg_path;
use crate::migration::{MigrationRegistry, Migrator};
use crate::zip_io::{read_qfdoc, write_qfdoc};
use qf_core::OpSeq;
use qf_core::{DocumentId, Hash256, StorageError, Version};
use qf_journal::Journal;
use qf_kernel::{CanonicalState, DocumentSnapshot, Kernel};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const MIME_TYPE: &str = "application/vnd.quillforge.document";
pub const FORMAT_VERSION: Version = Version::V1_0_0;
/// Written into package metadata — WPAI Software product identity.
pub const APP_VERSION: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " (WPAI Software)"
);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Package {
    pub format_version: Version,
    pub metadata: PackageMetadata,
    pub checkpoint: Option<CheckpointBlob>,
    pub journal_bytes: Vec<u8>,
    pub assets: BTreeMap<String, Vec<u8>>,
    /// Unrecognized ZIP members preserved verbatim.
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

#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    pub target_version: Version,
    pub auto_migrate: bool,
    /// Passphrase when the on-disk file is `QFENC1` encrypted.
    pub passphrase: Option<String>,
}

impl OpenOptions {
    pub fn v1() -> Self {
        Self {
            target_version: FORMAT_VERSION,
            auto_migrate: true,
            passphrase: None,
        }
    }

    pub fn with_passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.passphrase = Some(passphrase.into());
        self
    }
}

#[derive(Debug)]
pub enum OpenOutcome {
    Healthy {
        kernel: Kernel,
        journal_bytes: Vec<u8>,
        package_path: PathBuf,
        notes: Vec<String>,
    },
    RecoveredFromLkg {
        kernel: Kernel,
        journal_bytes: Vec<u8>,
        lkg_path: PathBuf,
        notes: Vec<String>,
    },
    NeedsMigration {
        from: Version,
        to: Version,
    },
    Corrupt {
        message: String,
        quarantine_hint: Option<PathBuf>,
    },
}

pub struct PackageStore {
    pub format_version: Version,
    pub migrator: Migrator,
}

impl Default for PackageStore {
    fn default() -> Self {
        Self {
            format_version: FORMAT_VERSION,
            migrator: Migrator {
                registry: MigrationRegistry::new(),
                target: FORMAT_VERSION,
            },
        }
    }
}

impl PackageStore {
    pub fn with_migrator(migrator: Migrator) -> Self {
        Self {
            format_version: migrator.target,
            migrator,
        }
    }

    /// Build a package from live canonical state + journal bytes.
    pub fn package_from_state(
        &self,
        state: &CanonicalState,
        journal_bytes: Vec<u8>,
        assets: BTreeMap<String, Vec<u8>>,
        other_files: BTreeMap<String, Vec<u8>>,
    ) -> Package {
        let snap = DocumentSnapshot::from_state(state.clone());
        Package {
            format_version: self.format_version,
            metadata: PackageMetadata {
                format_version: self.format_version,
                document_id: state.document_id,
                created_at_unix_ms: state.document.meta.created_at.unix_ms,
                app_version_written: APP_VERSION.into(),
                migration_history: vec![],
                content_seal: snap.snapshot_hash,
            },
            checkpoint: Some(CheckpointBlob {
                at_op_seq: state.op_seq.0,
                snapshot: snap,
            }),
            journal_bytes,
            assets,
            other_files,
        }
    }

    pub fn save_package(&self, path: &Path, package: &Package) -> Result<(), StorageError> {
        let bytes = write_qfdoc(package)?;
        crate::atomic::write_bytes_atomic(path, &bytes)
    }

    /// Atomic save with Argon2id + XChaCha20-Poly1305 envelope (`.qfdoc` ciphertext).
    pub fn save_package_encrypted(
        &self,
        path: &Path,
        package: &Package,
        passphrase: &str,
    ) -> Result<(), StorageError> {
        let plain = write_qfdoc(package)?;
        let params = qf_security::KdfParams {
            // Production defaults; tests may wrap with custom encrypt API.
            m_kib: qf_security::DEFAULT_M_KIB,
            t_cost: qf_security::DEFAULT_T_COST,
            p_cost: qf_security::DEFAULT_P_COST,
        };
        let enc = qf_security::encrypt_package(passphrase, &plain, &params)
            .map_err(|e| StorageError::Encryption(e.to_string()))?;
        crate::atomic::write_bytes_atomic(path, &enc)
    }

    /// Save encrypted with explicit KDF params (tests use low memory).
    pub fn save_package_encrypted_with_params(
        &self,
        path: &Path,
        package: &Package,
        passphrase: &str,
        params: &qf_security::KdfParams,
    ) -> Result<(), StorageError> {
        let plain = write_qfdoc(package)?;
        let enc = qf_security::encrypt_package(passphrase, &plain, params)
            .map_err(|e| StorageError::Encryption(e.to_string()))?;
        crate::atomic::write_bytes_atomic(path, &enc)
    }

    pub fn open_path(&self, path: &Path, opts: OpenOptions) -> Result<OpenOutcome, StorageError> {
        match self.open_inner(path, &opts) {
            Ok(outcome) => Ok(outcome),
            Err(e) => {
                // Fail closed: try LKG without clobbering primary.
                let lkg = lkg_path(path);
                if lkg.exists() {
                    match self.open_inner(&lkg, &opts) {
                        Ok(OpenOutcome::Healthy {
                            kernel,
                            journal_bytes,
                            notes,
                            ..
                        }) => {
                            let mut notes = notes;
                            notes.push(format!(
                                "primary open failed ({e}); recovered from LKG {}",
                                lkg.display()
                            ));
                            Ok(OpenOutcome::RecoveredFromLkg {
                                kernel,
                                journal_bytes,
                                lkg_path: lkg,
                                notes,
                            })
                        }
                        Ok(other) => Ok(other),
                        Err(lkg_err) => Ok(OpenOutcome::Corrupt {
                            message: format!("primary: {e}; lkg: {lkg_err}"),
                            quarantine_hint: Some(path.to_path_buf()),
                        }),
                    }
                } else {
                    let q = crate::zip_io::quarantine_copy(path).ok();
                    Ok(OpenOutcome::Corrupt {
                        message: e.to_string(),
                        quarantine_hint: q.or_else(|| Some(path.to_path_buf())),
                    })
                }
            }
        }
    }

    fn open_inner(&self, path: &Path, opts: &OpenOptions) -> Result<OpenOutcome, StorageError> {
        let mut bytes = std::fs::read(path).map_err(|e| StorageError::Io(e.to_string()))?;
        if qf_security::is_encrypted_package(&bytes) {
            let pass = opts
                .passphrase
                .as_deref()
                .ok_or_else(|| StorageError::Encryption("passphrase required".into()))?;
            bytes = qf_security::decrypt_package(pass, &bytes)
                .map_err(|e| StorageError::Encryption(e.to_string()))?;
        }
        let mut package = read_qfdoc(&bytes)?;

        if package.format_version > opts.target_version {
            return Ok(OpenOutcome::NeedsMigration {
                from: package.format_version,
                to: opts.target_version,
            });
        }

        if package.format_version < opts.target_version {
            if !opts.auto_migrate {
                return Ok(OpenOutcome::NeedsMigration {
                    from: package.format_version,
                    to: opts.target_version,
                });
            }
            package = self
                .migrator
                .migrate_package(package)
                .map_err(|e| StorageError::Migration(e.to_string()))?;
        }

        let (kernel, notes) = reconstruct_kernel(&package)?;
        Ok(OpenOutcome::Healthy {
            kernel,
            journal_bytes: package.journal_bytes,
            package_path: path.to_path_buf(),
            notes,
        })
    }
}

/// Reconstruct kernel from checkpoint + journal bytes.
pub fn reconstruct_kernel(package: &Package) -> Result<(Kernel, Vec<String>), StorageError> {
    let mut notes = Vec::new();
    let checkpoint = package
        .checkpoint
        .as_ref()
        .ok_or_else(|| StorageError::Corrupt("missing checkpoint".into()))?;

    if !checkpoint.snapshot.verify_hash() {
        return Err(StorageError::Corrupt("checkpoint hash mismatch".into()));
    }

    let mut kernel = Kernel::from_snapshot_unverified(checkpoint.snapshot.clone());
    // Checkpoint already includes state at at_op_seq; journal should only hold
    // ops AFTER checkpoint. When saving we fold journal into checkpoint and
    // may store empty journal or only post-checkpoint ops.
    if !package.journal_bytes.is_empty() {
        // Parse journal frames in-memory via a unique temp file (avoids cross-test races).
        let tmp = tempfile::Builder::new()
            .prefix("qf-open-journal-")
            .tempdir()
            .map_err(|e| StorageError::Io(e.to_string()))?;
        let jpath = tmp.path().join("ops.log");
        let journal = Journal::from_bytes(&jpath, &package.journal_bytes)
            .map_err(|e| StorageError::Journal(e.to_string()))?;
        journal
            .verify_chain()
            .map_err(|e| StorageError::Journal(e.to_string()))?;

        let from = OpSeq(checkpoint.at_op_seq);
        let records = journal
            .iter_from(from)
            .map_err(|e| StorageError::Journal(e.to_string()))?;
        let n = records.len();
        for rec in records {
            kernel
                .apply(&rec.payload)
                .map_err(|e| StorageError::Apply(e.to_string()))?;
            if kernel.state().op_seq != rec.op_seq {
                return Err(StorageError::Corrupt(format!(
                    "seq mismatch after replay: kernel={} journal={}",
                    kernel.state().op_seq.0,
                    rec.op_seq.0
                )));
            }
        }
        if n > 0 {
            notes.push(format!(
                "replayed {n} journal ops after checkpoint seq {}",
                checkpoint.at_op_seq
            ));
        }
        // tmp dir dropped here
    }
    Ok((kernel, notes))
}
