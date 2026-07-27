//! Atomic file replace with last-known-good retention.

use qf_core::StorageError;
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Sibling last-known-good path: `name.qfdoc` → `name.qfdoc.lkg`.
pub fn lkg_path(path: &Path) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(".lkg");
    PathBuf::from(s)
}

/// Write bytes atomically: staging file → fsync → optional LKG copy → rename.
pub fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> Result<(), StorageError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| StorageError::Io(e.to_string()))?;

    let staging = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("package"),
        std::process::id()
    ));

    {
        let mut f = File::create(&staging).map_err(|e| StorageError::Io(e.to_string()))?;
        f.write_all(bytes)
            .map_err(|e| StorageError::Io(e.to_string()))?;
        f.sync_all()
            .map_err(|e| StorageError::Io(e.to_string()))?;
    }

    if path.exists() {
        let lkg = lkg_path(path);
        fs::copy(path, &lkg).map_err(|e| StorageError::Io(e.to_string()))?;
        // Best-effort fsync of LKG on platforms that support it.
        if let Ok(f) = File::open(&lkg) {
            let _ = f.sync_all();
        }
    }

    // On Windows, rename over existing may fail — remove target first after LKG.
    if path.exists() {
        fs::remove_file(path).map_err(|e| StorageError::Io(e.to_string()))?;
    }
    fs::rename(&staging, path).map_err(|e| StorageError::Io(e.to_string()))?;

    // Clean staging if rename left it (should not).
    let _ = fs::remove_file(&staging);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_creates_and_lkg() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.qfdoc");
        write_bytes_atomic(&path, b"v1").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"v1");
        write_bytes_atomic(&path, b"v2").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"v2");
        assert_eq!(fs::read(lkg_path(&path)).unwrap(), b"v1");
    }
}
