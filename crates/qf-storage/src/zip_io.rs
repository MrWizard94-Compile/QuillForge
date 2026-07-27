//! ZIP container I/O for `.qfdoc` packages.

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

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Manifest {
    entries: BTreeMap<String, Hash256>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct CheckpointMeta {
    at_op_seq: u64,
    snapshot_hash: Hash256,
}

/// Serialize package to `.qfdoc` ZIP bytes.
pub fn write_qfdoc(package: &Package) -> Result<Vec<u8>, StorageError> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut zip = ZipWriter::new(&mut cursor);
        let store = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        let deflate = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

        // mimetype must be first and uncompressed (ODF convention).
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
            let path = format!("assets/{name}");
            zip.start_file(&path, deflate)
                .map_err(|e| StorageError::Io(e.to_string()))?;
            zip.write_all(data)
                .map_err(|e| StorageError::Io(e.to_string()))?;
        }

        for (name, data) in &package.other_files {
            // Never overwrite reserved paths.
            if is_reserved(name) {
                continue;
            }
            zip.start_file(name, deflate)
                .map_err(|e| StorageError::Io(e.to_string()))?;
            zip.write_all(data)
                .map_err(|e| StorageError::Io(e.to_string()))?;
        }

        // Manifest of content hashes for known members.
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

/// Deserialize package from `.qfdoc` ZIP bytes. Preserves unknown members.
pub fn read_qfdoc(bytes: &[u8]) -> Result<Package, StorageError> {
    let cursor = Cursor::new(bytes);
    let mut zip =
        ZipArchive::new(cursor).map_err(|e| StorageError::Corrupt(format!("zip: {e}")))?;

    let mut mimetype = None;
    let mut meta: Option<PackageMetadata> = None;
    let mut checkpoint: Option<CheckpointBlob> = None;
    let mut journal_bytes = Vec::new();
    let mut assets = BTreeMap::new();
    let mut other_files = BTreeMap::new();
    let mut manifest: Option<Manifest> = None;
    let mut cp_meta: Option<CheckpointMeta> = None;
    let mut cp_state: Option<qf_kernel::DocumentSnapshot> = None;

    for i in 0..zip.len() {
        let mut file = zip
            .by_index(i)
            .map_err(|e| StorageError::Corrupt(e.to_string()))?;
        let name = file.name().to_string();
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| StorageError::Io(e.to_string()))?;

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
                assets.insert(key, data);
            }
            other => {
                other_files.insert(other.to_string(), data);
            }
        }
    }

    if let Some(mt) = &mimetype {
        if mt != MIME_TYPE {
            return Err(StorageError::Corrupt(format!("bad mimetype: {mt}")));
        }
    } else {
        return Err(StorageError::Corrupt("missing mimetype".into()));
    }

    let metadata = meta.ok_or_else(|| StorageError::Corrupt("missing package meta".into()))?;

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

    // Optional manifest verification for known paths.
    if let Some(man) = manifest {
        if let Some(expected) = man.entries.get(PATH_JOURNAL) {
            let actual = Hash256::sha256(&journal_bytes);
            if actual != *expected {
                return Err(StorageError::Corrupt("journal manifest hash mismatch".into()));
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

