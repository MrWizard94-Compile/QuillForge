//! Append-only durable operation journal.
//!
//! # Durability contract
//!
//! `append_batch` returns `Ok` only after record bytes are written and
//! `File::sync_all` has completed. Callers may treat `Ok` as power-loss safe.
//!
//! # Frame format
//!
//! ```text
//! magic u32 LE = 0x51464A52 ("QFJR")
//! len   u32 LE
//! crc32c u32 LE  (of payload)
//! payload: JSON JournalRecord
//! ```
//!
//! Incomplete trailing frames are ignored on read (torn-write recovery).

use qf_core::{
    next_seq, ActorId, DocumentId, Hash256, JournalError, OpId, OpSeq, Operation, SessionId,
    Timestamp,
};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const FRAME_MAGIC: u32 = 0x5146_4A52; // "QFJR"
const HEADER_LEN: usize = 12;

/// One durable journal record.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JournalRecord {
    pub schema_version: u32,
    pub op_id: OpId,
    pub op_seq: OpSeq,
    pub document_id: DocumentId,
    pub actor_id: ActorId,
    pub session_id: SessionId,
    pub committed_at: Timestamp,
    pub parents: Vec<OpSeq>,
    pub content_hash: Hash256,
    pub prev_record_hash: Hash256,
    pub payload: Operation,
}

/// File-backed append-only journal.
pub struct Journal {
    path: PathBuf,
    file: File,
    last_seq: OpSeq,
    last_hash: Hash256,
    /// Byte length of valid frames (excludes torn tail).
    valid_len: u64,
}

impl Journal {
    /// Open an existing journal or create an empty one. Truncates torn trailing frames.
    pub fn open_or_create(path: &Path) -> Result<Self, JournalError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| JournalError::Io(e.to_string()))?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .map_err(|e| JournalError::Io(e.to_string()))?;

        let scan = scan_file(&mut file)?;
        if scan.valid_len < scan.file_len {
            file.set_len(scan.valid_len)
                .map_err(|e| JournalError::Io(e.to_string()))?;
            file.sync_all()
                .map_err(|e| JournalError::Io(e.to_string()))?;
        }
        file.seek(SeekFrom::End(0))
            .map_err(|e| JournalError::Io(e.to_string()))?;

        Ok(Self {
            path: path.to_path_buf(),
            file,
            last_seq: scan.last_seq,
            last_hash: scan.last_hash,
            valid_len: scan.valid_len,
        })
    }

    /// Create a journal from raw frame bytes (e.g. extracted from a package).
    pub fn from_bytes(path: &Path, bytes: &[u8]) -> Result<Self, JournalError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| JournalError::Io(e.to_string()))?;
        }
        {
            let mut f = File::create(path).map_err(|e| JournalError::Io(e.to_string()))?;
            f.write_all(bytes)
                .map_err(|e| JournalError::Io(e.to_string()))?;
            f.sync_all().map_err(|e| JournalError::Io(e.to_string()))?;
        }
        Self::open_or_create(path)
    }

    pub fn last_seq(&self) -> OpSeq {
        self.last_seq
    }

    pub fn last_hash(&self) -> Hash256 {
        self.last_hash
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn valid_len(&self) -> u64 {
        self.valid_len
    }

    /// Append records with consecutive sequences. Fsync before returning Ok.
    pub fn append_batch(&mut self, records: &[JournalRecord]) -> Result<(), JournalError> {
        if records.is_empty() {
            return Ok(());
        }
        for rec in records {
            let expected = next_seq(self.last_seq);
            if rec.op_seq != expected {
                return Err(JournalError::Corrupt(format!(
                    "expected seq {}, got {}",
                    expected.0, rec.op_seq.0
                )));
            }
            if rec.prev_record_hash != self.last_hash {
                return Err(JournalError::HashChain(rec.op_seq.0));
            }
            let bytes = encode_frame(rec)?;
            self.file
                .write_all(&bytes)
                .map_err(|e| JournalError::Io(e.to_string()))?;
            self.last_seq = rec.op_seq;
            self.last_hash = frame_hash(&bytes);
            self.valid_len += bytes.len() as u64;
        }
        self.file
            .sync_all()
            .map_err(|e| JournalError::Io(e.to_string()))?;
        Ok(())
    }

    /// All records with `op_seq > from_seq`, in order.
    pub fn iter_from(&self, from_seq: OpSeq) -> Result<Vec<JournalRecord>, JournalError> {
        let mut file = File::open(&self.path).map_err(|e| JournalError::Io(e.to_string()))?;
        let all = read_all_records(&mut file)?.records;
        Ok(all.into_iter().filter(|r| r.op_seq > from_seq).collect())
    }

    pub fn all_records(&self) -> Result<Vec<JournalRecord>, JournalError> {
        self.iter_from(OpSeq::ZERO)
    }

    /// Raw valid frame bytes for packaging.
    pub fn to_bytes(&self) -> Result<Vec<u8>, JournalError> {
        let mut file = File::open(&self.path).map_err(|e| JournalError::Io(e.to_string()))?;
        let mut data = Vec::new();
        file.read_to_end(&mut data)
            .map_err(|e| JournalError::Io(e.to_string()))?;
        let scan = parse_bytes(&data)?;
        data.truncate(scan.valid_len as usize);
        Ok(data)
    }

    pub fn verify_chain(&self) -> Result<(), JournalError> {
        let mut file = File::open(&self.path).map_err(|e| JournalError::Io(e.to_string()))?;
        let mut prev = Hash256::ZERO;
        let mut expected = OpSeq::ZERO;
        for rec in read_all_records(&mut file)?.records {
            if rec.prev_record_hash != prev {
                return Err(JournalError::HashChain(rec.op_seq.0));
            }
            let next = next_seq(expected);
            if rec.op_seq != next {
                return Err(JournalError::Corrupt(format!(
                    "seq gap: expected {}, got {}",
                    next.0, rec.op_seq.0
                )));
            }
            expected = rec.op_seq;
            let bytes = encode_frame(&rec)?;
            prev = frame_hash(&bytes);
        }
        Ok(())
    }
}

/// Inputs for constructing a chained journal record.
pub struct RecordDraft {
    pub prev_hash: Hash256,
    pub seq: OpSeq,
    pub op_id: OpId,
    pub document_id: DocumentId,
    pub actor_id: ActorId,
    pub session_id: SessionId,
    pub committed_at: Timestamp,
    pub payload: Operation,
}

/// Build a record with correct chain linkage for the given previous hash.
pub fn make_record(draft: RecordDraft) -> JournalRecord {
    let content_hash = hash_operation(&draft.payload);
    JournalRecord {
        schema_version: 1,
        op_id: draft.op_id,
        op_seq: draft.seq,
        document_id: draft.document_id,
        actor_id: draft.actor_id,
        session_id: draft.session_id,
        committed_at: draft.committed_at,
        parents: if draft.seq.0 <= 1 {
            vec![]
        } else {
            vec![OpSeq(draft.seq.0 - 1)]
        },
        content_hash,
        prev_record_hash: draft.prev_hash,
        payload: draft.payload,
    }
}

/// Stable content hash for an operation payload.
pub fn hash_operation(op: &Operation) -> Hash256 {
    match serde_json::to_vec(op) {
        Ok(bytes) => Hash256::sha256(&bytes),
        Err(_) => Hash256::sha256(format!("{op:?}").as_bytes()),
    }
}

pub fn encode_frame(rec: &JournalRecord) -> Result<Vec<u8>, JournalError> {
    let payload = serde_json::to_vec(rec).map_err(|e| JournalError::Corrupt(e.to_string()))?;
    let mut out = Vec::with_capacity(HEADER_LEN + payload.len());
    out.extend_from_slice(&FRAME_MAGIC.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    let crc = crc32c::crc32c(&payload);
    out.extend_from_slice(&crc.to_le_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

fn frame_hash(frame: &[u8]) -> Hash256 {
    Hash256::sha256(frame)
}

struct ScanResult {
    last_seq: OpSeq,
    last_hash: Hash256,
    valid_len: u64,
    file_len: u64,
    records: Vec<JournalRecord>,
}

fn scan_file(file: &mut File) -> Result<ScanResult, JournalError> {
    let mut data = Vec::new();
    file.seek(SeekFrom::Start(0))
        .map_err(|e| JournalError::Io(e.to_string()))?;
    file.read_to_end(&mut data)
        .map_err(|e| JournalError::Io(e.to_string()))?;
    let mut scan = parse_bytes(&data)?;
    scan.file_len = data.len() as u64;
    Ok(scan)
}

fn read_all_records(file: &mut File) -> Result<ScanResult, JournalError> {
    scan_file(file)
}

fn parse_bytes(data: &[u8]) -> Result<ScanResult, JournalError> {
    let mut records = Vec::new();
    let mut i = 0usize;
    let mut last_seq = OpSeq::ZERO;
    let mut last_hash = Hash256::ZERO;
    let mut valid_len = 0u64;

    while i + HEADER_LEN <= data.len() {
        let magic = u32::from_le_bytes(data[i..i + 4].try_into().unwrap());
        if magic != FRAME_MAGIC {
            if records.is_empty() && i == 0 {
                return Err(JournalError::Corrupt(format!("bad magic at {i}")));
            }
            // Stop at first bad magic after valid content (treat as torn/corrupt tail).
            break;
        }
        let len = u32::from_le_bytes(data[i + 4..i + 8].try_into().unwrap()) as usize;
        let crc = u32::from_le_bytes(data[i + 8..i + 12].try_into().unwrap());
        let start = i + HEADER_LEN;
        let end = start.saturating_add(len);
        if end > data.len() {
            break; // torn
        }
        let payload = &data[start..end];
        if crc32c::crc32c(payload) != crc {
            break; // torn / corrupt trailing
        }
        let rec: JournalRecord =
            serde_json::from_slice(payload).map_err(|e| JournalError::Corrupt(e.to_string()))?;
        let frame = &data[i..end];
        last_seq = rec.op_seq;
        last_hash = frame_hash(frame);
        records.push(rec);
        valid_len = end as u64;
        i = end;
    }

    Ok(ScanResult {
        last_seq,
        last_hash,
        valid_len,
        file_len: data.len() as u64,
        records,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::{BlockId, SeqIdGen, TextOffset};
    use std::time::{Duration, Instant};

    /// PB-JOURNAL-001 — append + fsync 500 records under CI budget.
    #[test]
    fn budget_append_500_under_limit() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("budget.log");
        let mut gen = SeqIdGen::default();
        let mut j = Journal::open_or_create(&path).unwrap();
        let doc = DocumentId::new(&mut gen);
        let actor = ActorId::new(&mut gen);
        let session = SessionId::new(&mut gen);
        let start = Instant::now();
        for i in 1..=500u64 {
            let rec = make_record(RecordDraft {
                prev_hash: j.last_hash(),
                seq: OpSeq(i),
                op_id: OpId::new(&mut gen),
                document_id: doc,
                actor_id: actor,
                session_id: session,
                committed_at: Timestamp { unix_ms: i as i64 },
                payload: Operation::Noop {
                    reason: qf_core::NoopReason::Other(format!("b{i}")),
                },
            });
            j.append_batch(&[rec]).unwrap();
        }
        let elapsed = start.elapsed();
        assert_eq!(j.last_seq(), OpSeq(500));
        j.verify_chain().unwrap();
        assert!(
            elapsed < Duration::from_secs(10),
            "PB-JOURNAL-001 exceeded: {elapsed:?}"
        );
    }

    #[test]
    fn append_and_replay() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ops.log");
        let mut gen = SeqIdGen::default();

        let mut j = Journal::open_or_create(&path).unwrap();
        let doc = DocumentId::new(&mut gen);
        let actor = ActorId::new(&mut gen);
        let session = SessionId::new(&mut gen);
        let op = Operation::InsertText {
            block_id: BlockId::new(&mut gen),
            at: TextOffset(0),
            text: "hi".into(),
        };
        let rec = make_record(RecordDraft {
            prev_hash: Hash256::ZERO,
            seq: OpSeq(1),
            op_id: OpId::new(&mut gen),
            document_id: doc,
            actor_id: actor,
            session_id: session,
            committed_at: Timestamp { unix_ms: 0 },
            payload: op,
        });
        j.append_batch(&[rec]).unwrap();
        assert_eq!(j.last_seq(), OpSeq(1));
        j.verify_chain().unwrap();
        assert_eq!(j.iter_from(OpSeq::ZERO).unwrap().len(), 1);
    }

    #[test]
    fn torn_trailing_frame_truncated() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("torn.log");
        let mut gen = SeqIdGen::default();
        let mut j = Journal::open_or_create(&path).unwrap();
        let rec = make_record(RecordDraft {
            prev_hash: Hash256::ZERO,
            seq: OpSeq(1),
            op_id: OpId::new(&mut gen),
            document_id: DocumentId::new(&mut gen),
            actor_id: ActorId::new(&mut gen),
            session_id: SessionId::new(&mut gen),
            committed_at: Timestamp { unix_ms: 0 },
            payload: Operation::Noop {
                reason: qf_core::NoopReason::Other("t".into()),
            },
        });
        j.append_batch(&[rec]).unwrap();
        let good_len = j.valid_len();
        drop(j);

        // Append garbage torn tail.
        {
            use std::io::Write;
            let mut f = OpenOptions::new().append(true).open(&path).unwrap();
            f.write_all(&[0x51, 0x46, 0x4A, 0x52, 0xFF, 0xFF, 0x00, 0x00])
                .unwrap();
        }

        let j2 = Journal::open_or_create(&path).unwrap();
        assert_eq!(j2.last_seq(), OpSeq(1));
        assert_eq!(j2.valid_len(), good_len);
        j2.verify_chain().unwrap();
    }

    #[test]
    fn bytes_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("rt.log");
        let mut gen = SeqIdGen::default();
        let mut j = Journal::open_or_create(&path).unwrap();
        let rec = make_record(RecordDraft {
            prev_hash: Hash256::ZERO,
            seq: OpSeq(1),
            op_id: OpId::new(&mut gen),
            document_id: DocumentId::new(&mut gen),
            actor_id: ActorId::new(&mut gen),
            session_id: SessionId::new(&mut gen),
            committed_at: Timestamp { unix_ms: 1 },
            payload: Operation::Noop {
                reason: qf_core::NoopReason::IdempotentRemote,
            },
        });
        j.append_batch(&[rec]).unwrap();
        let bytes = j.to_bytes().unwrap();
        let path2 = dir.path().join("rt2.log");
        let j2 = Journal::from_bytes(&path2, &bytes).unwrap();
        assert_eq!(j2.last_seq(), OpSeq(1));
        assert_eq!(j2.all_records().unwrap().len(), 1);
    }
}
