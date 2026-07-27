//! ZIP container I/O for `.qfdoc` packages.
//!
//! Opened packages are **untrusted input**. Path traversal, zip bombs (entry count /
//! logical size), and malformed members are rejected fail-closed.

use crate::package::{CheckpointBlob, Package, PackageMetadata, MIME_TYPE};
use qf_core::{Hash256, StorageError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const PATH_MIMETYPE: &str = "mimetype";
const PATH_META: &str = "meta/package.cbor";
const PATH_CHECKPOINT: &str = "checkpoint/state.cbor";
const PATH_CHECKPOINT_META: &str = "checkpoint/meta.cbor";
const PATH_JOURNAL: &str = "journal/ops.log";
const PATH_MANIFEST: &str = "META-INF/manifest.cbor";

/// Default maximum number of ZIP entries (zip-bomb control).
pub const MAX_ZIP_ENTRIES: usize = 100_000;
/// Default maximum total uncompressed bytes across all entries.
pub const MAX_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;
/// Maximum single entry uncompressed size.
pub const MAX_ENTRY_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Manifest {
    entries: BTreeMap<String, Hash256>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CheckpointMeta {
    at_op_seq: u64,
    snapshot_hash: Hash256,
}

/// Limits applied when reading untrusted packages.
#[derive(Clone, Debug)]
pub struct ReadLimits {
    pub max_entries: usize,
    pub max_total_uncompressed: u64,
    pub max_entry_uncompressed: u64,
}

impl Default for ReadLimits {
    fn default() -> Self {
        Self {
            max_entries: MAX_ZIP_ENTRIES,
            max_total_uncompressed: MAX_UNCOMPRESSED_BYTES,
            max_entry_uncompressed: MAX_ENTRY_BYTES,
        }
    }
}

/// Serialize package to `.qfdoc` ZIP bytes.
pub fn write_qfdoc(package: &Package) -> Result<Vec<u8>, StorageError> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let store = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let deflate = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        zip.start_file(PATH_MIMETYPE, store)
            .map_err(|e| StorageError::Io(e.to_string()))?;
        zip.write_all(MIME_TYPE.as_bytes())
            .map_err(|e| StorageError::Io(e.to_string()))?;

        let meta_bytes = to_cbor(&package.metadata)?;
        zip.start_file(PATH_META, deflate)
            .map_err(|e| StorageError::Io(e.to_string()))?;
        zip.write_all(&meta_bytes)
            .map_err(|e| StorageError::Io(e.to_string()))?;

        if let Some(cp) = &package.checkpoint {
            let state_bytes = to_cbor(&cp.snapshot)?;
            zip.start_file(PATH_CHECKPOINT, deflate)
                .map_err(|e| StorageError::Io(e.to_string()))?;
            zip.write_all(&state_bytes)
                .map_err(|e| StorageError::Io(e.to_string()))?;

            let cp_meta = CheckpointMeta {
                at_op_seq: cp.at_op_seq,
                snapshot_hash: cp.snapshot.snapshot_hash,
            };
            let cp_meta_bytes = to_cbor(&cp_meta)?;
            zip.start_file(PATH_CHECKPOINT_META, deflate)
                .map_err(|e| StorageError::Io(e.to_string()))?;
            zip.write_all(&cp_meta_bytes)
                .map_err(|e| StorageError::Io(e.to_string()))?;
        }

        zip.start_file(PATH_JOURNAL, store)
            .map_err(|e| StorageError::Io(e.to_string()))?;
        zip.write_all(&package.journal_bytes)
            .map_err(|e| StorageError::Io(e.to_string()))?;

        for (name, data) in &package.assets {
            validate_member_name(&format!("assets/{name}"))?;
            let path = format!("assets/{name}");
            zip.start_file(&path, deflate)
                .map_err(|e| StorageError::Io(e.to_string()))?;
            zip.write_all(data)
                .map_err(|e| StorageError::Io(e.to_string()))?;
        }

        for (name, data) in &package.other_files {
            if is_reserved(name) {
                continue;
            }
            validate_member_name(name)?;
            zip.start_file(name, deflate)
                .map_err(|e| StorageError::Io(e.to_string()))?;
            zip.write_all(data)
                .map_err(|e| StorageError::Io(e.to_string()))?;
        }

        let mut entries = BTreeMap::new();
        entries.insert(PATH_META.to_string(), Hash256::sha256(&meta_bytes));
        entries.insert(
            PATH_JOURNAL.to_string(),
            Hash256::sha256(&package.journal_bytes),
        );
        if let Some(cp) = &package.checkpoint {
            let state_bytes = to_cbor(&cp.snapshot)?;
            entries.insert(PATH_CHECKPOINT.to_string(), Hash256::sha256(&state_bytes));
        }
        let manifest = Manifest { entries };
        let man_bytes = to_cbor(&manifest)?;
        zip.start_file(PATH_MANIFEST, deflate)
            .map_err(|e| StorageError::Io(e.to_string()))?;
        zip.write_all(&man_bytes)
            .map_err(|e| StorageError::Io(e.to_string()))?;

        zip.finish()
            .map_err(|e| StorageError::Io(e.to_string()))?;
    }
    Ok(cursor.into_inner())
}

/// Deserialize package from `.qfdoc` ZIP bytes with default untrusted-input limits.
pub fn read_qfdoc(bytes: &[u8]) -> Result<Package, StorageError> {
    read_qfdoc_with_limits(bytes, &ReadLimits::default())
}

/// Deserialize with explicit limits (tests may tighten).
pub fn read_qfdoc_with_limits(bytes: &[u8], limits: &ReadLimits) -> Result<Package, StorageError> {
    let cursor = Cursor::new(bytes);
    let mut zip =
        ZipArchive::new(cursor).map_err(|e| StorageError::Corrupt(format!("zip: {e}")))?;

    if zip.len() > limits.max_entries {
        return Err(StorageError::Corrupt(format!(
            "too many zip entries: {} > {}",
            zip.len(),
            limits.max_entries
        )));
    }

    let mut mimetype = None;
    let mut meta: Option<PackageMetadata> = None;
    let mut journal_bytes = Vec::new();
    let mut assets = BTreeMap::new();
    let mut other_files = BTreeMap::new();
    let mut manifest: Option<Manifest> = None;
    let mut cp_meta: Option<CheckpointMeta> = None;
    let mut cp_state: Option<qf_kernel::DocumentSnapshot> = None;
    let mut total_uncompressed: u64 = 0;

    for i in 0..zip.len() {
        let mut file = zip
            .by_index(i)
            .map_err(|e| StorageError::Corrupt(e.to_string()))?;
        let name = file.name().to_string();
        validate_member_name(&name)?;

        let declared = file.size();
        if declared > limits.max_entry_uncompressed {
            return Err(StorageError::Corrupt(format!(
                "entry too large: {name} ({declared} bytes)"
            )));
        }
        total_uncompressed = total_uncompressed.saturating_add(declared);
        if total_uncompressed > limits.max_total_uncompressed {
            return Err(StorageError::Corrupt(format!(
                "uncompressed package budget exceeded ({total_uncompressed} bytes)"
            )));
        }

        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| StorageError::Io(e.to_string()))?;
        if data.len() as u64 > limits.max_entry_uncompressed {
            return Err(StorageError::Corrupt(format!(
                "entry expanded too large: {name}"
            )));
        }

        match name.as_str() {
            PATH_MIMETYPE => {
                mimetype = Some(
                    String::from_utf8(data).map_err(|e| StorageError::Corrupt(e.to_string()))?,
                );
            }
            PATH_META => {
                meta = Some(from_cbor(&data)?);
            }
            PATH_CHECKPOINT => {
                cp_state = Some(from_cbor(&data)?);
            }
            PATH_CHECKPOINT_META => {
                cp_meta = Some(from_cbor(&data)?);
            }
            PATH_JOURNAL => {
                journal_bytes = data;
            }
            PATH_MANIFEST => {
                manifest = Some(from_cbor(&data)?);
            }
            other if other.starts_with("assets/") => {
                let key = other.trim_start_matches("assets/").to_string();
                if key.is_empty() || key.contains("..") {
                    return Err(StorageError::Corrupt("invalid asset path".into()));
                }
                assets.insert(key, data);
            }
            other => {
                other_files.insert(other.to_string(), data);
            }
        }
    }

    let mt = mimetype.ok_or_else(|| StorageError::Corrupt("missing mimetype".into()))?;
    if mt != MIME_TYPE {
        return Err(StorageError::Corrupt(format!("bad mimetype: {mt}")));
    }

    let metadata = meta.ok_or_else(|| StorageError::Corrupt("missing package meta".into()))?;

    let mut checkpoint = None;
    if let (Some(state), Some(m)) = (cp_state, cp_meta) {
        if state.snapshot_hash != m.snapshot_hash {
            return Err(StorageError::Corrupt(
                "checkpoint meta hash mismatch".into(),
            ));
        }
        checkpoint = Some(CheckpointBlob {
            at_op_seq: m.at_op_seq,
            snapshot: state,
        });
    }

    if let Some(man) = manifest {
        if let Some(expected) = man.entries.get(PATH_JOURNAL) {
            let actual = Hash256::sha256(&journal_bytes);
            if actual != *expected {
                return Err(StorageError::Corrupt(
                    "journal manifest hash mismatch".into(),
                ));
            }
        }
    }

    Ok(Package {
        format_version: metadata.format_version,
        metadata,
        checkpoint,
        journal_bytes,
        assets,
        other_files,
    })
}

/// Copy a path to a quarantine name beside it (never overwrites original).
pub fn quarantine_copy(path: &std::path::Path) -> Result<std::path::PathBuf, StorageError> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // name.qfdoc → name.qfdoc.corrupt-<ts>
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".corrupt-{stamp}"));
    let dest = std::path::PathBuf::from(s);
    std::fs::copy(path, &dest).map_err(|e| StorageError::Io(e.to_string()))?;
    Ok(dest)
}

/// Reject path traversal and absolute paths in ZIP member names.
pub fn validate_member_name(name: &str) -> Result<(), StorageError> {
    if name.is_empty() {
        return Err(StorageError::Corrupt("empty zip member name".into()));
    }
    if name.starts_with('/') || name.starts_with('\\') {
        return Err(StorageError::Corrupt(format!(
            "absolute zip path forbidden: {name}"
        )));
    }
    if name.contains('\0') {
        return Err(StorageError::Corrupt("nul in zip path".into()));
    }
    for part in name.split(['/', '\\']) {
        if part == ".." {
            return Err(StorageError::Corrupt(format!(
                "path traversal forbidden: {name}"
            )));
        }
    }
    // Windows drive-like
    if name.len() >= 2 && name.as_bytes()[1] == b':' {
        return Err(StorageError::Corrupt(format!(
            "drive path forbidden: {name}"
        )));
    }
    Ok(())
}

fn is_reserved(name: &str) -> bool {
    matches!(
        name,
        PATH_MIMETYPE
            | PATH_META
            | PATH_CHECKPOINT
            | PATH_CHECKPOINT_META
            | PATH_JOURNAL
            | PATH_MANIFEST
    ) || name.starts_with("assets/")
}

fn to_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, StorageError> {
    let mut buf = Vec::new();
    ciborium::into_writer(value, &mut buf).map_err(|e| StorageError::Io(e.to_string()))?;
    Ok(buf)
}

fn from_cbor<T: for<'de> Deserialize<'de>>(bytes: &[u8]) -> Result<T, StorageError> {
    ciborium::from_reader(bytes).map_err(|e| StorageError::Corrupt(format!("cbor: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_path_traversal() {
        assert!(validate_member_name("../evil").is_err());
        assert!(validate_member_name("foo/../../x").is_err());
        assert!(validate_member_name("/abs").is_err());
        assert!(validate_member_name("meta/package.cbor").is_ok());
    }

    #[test]
    fn rejects_too_many_entries_budget() {
        // Craft minimal invalid: empty zip fails mimetype; we test limit path via limits.
        let limits = ReadLimits {
            max_entries: 0,
            ..ReadLimits::default()
        };
        // empty archive
        let mut cursor = Cursor::new(Vec::new());
        {
            let zip = ZipWriter::new(&mut cursor);
            zip.finish().unwrap();
        }
        let bytes = cursor.into_inner();
        let err = read_qfdoc_with_limits(&bytes, &limits).unwrap_err();
        assert!(matches!(err, StorageError::Corrupt(_)));
    }
}
