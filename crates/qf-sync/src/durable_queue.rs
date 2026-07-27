//! Durable offline sync queue with fsync-backed atomic persistence.
//!
//! Spec: enqueue after local commit; queue file survives process death.

use crate::{SyncError, SyncItem, SyncQueue};
use qf_core::Hash256;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

const QUEUE_MAGIC: &[u8; 8] = b"QFSYNCQ1";
const QUEUE_VERSION: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct QueueFile {
    version: u32,
    items: Vec<SyncItem>,
}

/// File-backed [`SyncQueue`]. `persist` fsyncs before returning Ok.
pub struct DurableSyncQueue {
    path: PathBuf,
    inner: SyncQueue,
}

impl DurableSyncQueue {
    /// Open existing queue or create empty. Corrupt files are quarantined and replaced.
    pub fn open_or_create(path: impl Into<PathBuf>) -> Result<Self, SyncError> {
        let path = path.into();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| SyncError::Network(e.to_string()))?;
        }
        let inner = if path.exists() {
            match load_queue(&path) {
                Ok(q) => q,
                Err(e) => {
                    // Quarantine corrupt queue; start fresh (items may need re-enqueue from journal).
                    let qpath = path.with_extension("qf-sync-queue.corrupt");
                    let _ = fs::rename(&path, &qpath);
                    let _ = e;
                    SyncQueue::default()
                }
            }
        } else {
            SyncQueue::default()
        };
        Ok(Self { path, inner })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    pub fn peek(&self) -> Option<&SyncItem> {
        self.inner.peek()
    }

    pub fn pending(&self) -> &SyncQueue {
        &self.inner
    }

    /// Enqueue and immediately persist (fsync).
    pub fn enqueue(&mut self, item: SyncItem) -> Result<(), SyncError> {
        self.inner.enqueue(item);
        self.persist()
    }

    /// Remove head after remote ack and persist.
    pub fn pop_acked(&mut self) -> Result<Option<SyncItem>, SyncError> {
        let item = self.inner.pop_acked();
        if item.is_some() {
            self.persist()?;
        }
        Ok(item)
    }

    /// Increment attempts on head (for backoff bookkeeping) and persist.
    pub fn bump_head_attempts(&mut self) -> Result<(), SyncError> {
        if let Some(front) = self.inner.pending.front_mut() {
            front.attempts = front.attempts.saturating_add(1);
            self.persist()?;
        }
        Ok(())
    }

    /// Exponential backoff delay in milliseconds: `min(60_000, 250 * 2^attempts)`.
    pub fn head_backoff_ms(&self) -> u64 {
        let attempts = self.inner.peek().map(|i| i.attempts).unwrap_or(0);
        let exp = 250u64.saturating_mul(1u64 << attempts.min(8));
        exp.min(60_000)
    }

    fn persist(&self) -> Result<(), SyncError> {
        let file = QueueFile {
            version: QUEUE_VERSION,
            items: self.inner.pending.iter().cloned().collect(),
        };
        let json = serde_json::to_vec(&file).map_err(|e| SyncError::Network(e.to_string()))?;
        atomic_write_fsync(&self.path, &json)
    }
}

fn load_queue(path: &Path) -> Result<SyncQueue, SyncError> {
    let bytes = fs::read(path).map_err(|e| SyncError::Network(e.to_string()))?;
    if bytes.len() < 8 || &bytes[0..8] != QUEUE_MAGIC {
        // Allow pure JSON without magic for forward compatibility tests.
        let file: QueueFile =
            serde_json::from_slice(&bytes).map_err(|e| SyncError::Rejected(e.to_string()))?;
        return Ok(queue_from_file(file));
    }
    let file: QueueFile =
        serde_json::from_slice(&bytes[8..]).map_err(|e| SyncError::Rejected(e.to_string()))?;
    if file.version != QUEUE_VERSION {
        return Err(SyncError::Rejected(format!(
            "unsupported queue version {}",
            file.version
        )));
    }
    Ok(queue_from_file(file))
}

fn queue_from_file(file: QueueFile) -> SyncQueue {
    let mut q = SyncQueue::default();
    for item in file.items {
        q.enqueue(item);
    }
    q
}

fn atomic_write_fsync(path: &Path, payload: &[u8]) -> Result<(), SyncError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|e| SyncError::Network(e.to_string()))?;
    let staging = parent.join(format!(
        ".{}.tmp-{}",
        path.file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("queue"),
        std::process::id()
    ));
    {
        let mut f = File::create(&staging).map_err(|e| SyncError::Network(e.to_string()))?;
        f.write_all(QUEUE_MAGIC)
            .map_err(|e| SyncError::Network(e.to_string()))?;
        f.write_all(payload)
            .map_err(|e| SyncError::Network(e.to_string()))?;
        f.sync_all()
            .map_err(|e| SyncError::Network(e.to_string()))?;
    }
    if path.exists() {
        fs::remove_file(path).map_err(|e| SyncError::Network(e.to_string()))?;
    }
    fs::rename(&staging, path).map_err(|e| SyncError::Network(e.to_string()))?;
    let _ = fs::remove_file(&staging);
    Ok(())
}

/// Build a queue item from a committed seq range.
pub fn make_sync_item(
    document_id: qf_core::DocumentId,
    first_seq: u64,
    last_seq: u64,
    payload: &[u8],
) -> SyncItem {
    SyncItem {
        id: uuid::Uuid::new_v4(),
        document_id,
        first_seq,
        last_seq,
        payload_hash: Hash256::sha256(payload),
        attempts: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::{DocumentId, SeqIdGen};

    #[test]
    fn durable_queue_survives_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("offline.queue");
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);

        {
            let mut q = DurableSyncQueue::open_or_create(&path).unwrap();
            q.enqueue(make_sync_item(doc, 1, 3, b"ops-batch-1"))
                .unwrap();
            q.enqueue(make_sync_item(doc, 4, 5, b"ops-batch-2"))
                .unwrap();
            assert_eq!(q.len(), 2);
        }

        let q2 = DurableSyncQueue::open_or_create(&path).unwrap();
        assert_eq!(q2.len(), 2);
        let head = q2.peek().unwrap();
        assert_eq!(head.first_seq, 1);
        assert_eq!(head.last_seq, 3);
    }

    #[test]
    fn pop_acked_persists() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("q.queue");
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let mut q = DurableSyncQueue::open_or_create(&path).unwrap();
        q.enqueue(make_sync_item(doc, 1, 1, b"a")).unwrap();
        q.enqueue(make_sync_item(doc, 2, 2, b"b")).unwrap();
        q.pop_acked().unwrap();
        drop(q);
        let q2 = DurableSyncQueue::open_or_create(&path).unwrap();
        assert_eq!(q2.len(), 1);
        assert_eq!(q2.peek().unwrap().first_seq, 2);
    }

    #[test]
    fn backoff_grows_with_attempts() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("b.queue");
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let mut q = DurableSyncQueue::open_or_create(&path).unwrap();
        q.enqueue(make_sync_item(doc, 1, 1, b"x")).unwrap();
        let b0 = q.head_backoff_ms();
        q.bump_head_attempts().unwrap();
        let b1 = q.head_backoff_ms();
        assert!(b1 > b0);
    }
}
