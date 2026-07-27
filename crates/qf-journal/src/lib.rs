//! Append-only durable operation journal.
//!
//! Durability contract: `append_batch` returns `Ok` only after data is written and
//! `sync_all` has completed on the journal file.

use qf_core::{
    ActorId, DocumentId, Hash256, JournalError, OpId, OpSeq, Operation, SessionId, Timestamp,
};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const FRAME_MAGIC: u32 = 0x5146_4A52; // "QFJR"

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

pub struct Journal {
    path: PathBuf,
    file: File,
    last_seq: OpSeq,
    last_hash: Hash256,
}

impl Journal {
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

        let (last_seq, last_hash) = scan_tail(&mut file)?;
        file.seek(SeekFrom::End(0))
            .map_err(|e| JournalError::Io(e.to_string()))?;

        Ok(Self {
            path: path.to_path_buf(),
            file,
            last_seq,
            last_hash,
        })
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

    /// Append records with consecutive sequences. Fsync before returning Ok.
    pub fn append_batch(&mut self, records: &[JournalRecord]) -> Result<(), JournalError> {
        for rec in records {
            if rec.op_seq != self.last_seq.next() && !(self.last_seq == OpSeq::ZERO && rec.op_seq == OpSeq(1))
            {
                // Allow first op as seq 1 from ZERO.
                if !(self.last_seq == OpSeq::ZERO && rec.op_seq.0 == 1) {
                    return Err(JournalError::Corrupt(format!(
                        "expected seq {}, got {}",
                        self.last_seq.next().0,
                        rec.op_seq.0
                    )));
                }
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
        }
        self.file
            .sync_all()
            .map_err(|e| JournalError::Io(e.to_string()))?;
        Ok(())
    }

    pub fn iter_from(&self, from_seq: OpSeq) -> Result<Vec<JournalRecord>, JournalError> {
        let mut file = File::open(&self.path).map_err(|e| JournalError::Io(e.to_string()))?;
        let all = read_all_records(&mut file)?;
        Ok(all
            .into_iter()
            .filter(|r| r.op_seq > from_seq)
            .collect())
    }

    pub fn verify_chain(&self) -> Result<(), JournalError> {
        let mut file = File::open(&self.path).map_err(|e| JournalError::Io(e.to_string()))?;
        let mut prev = Hash256::ZERO;
        let mut expected = OpSeq::ZERO;
        for rec in read_all_records(&mut file)? {
            if rec.prev_record_hash != prev {
                return Err(JournalError::HashChain(rec.op_seq.0));
            }
            let next = if expected == OpSeq::ZERO {
                OpSeq(1)
            } else {
                expected.next()
            };
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

fn encode_frame(rec: &JournalRecord) -> Result<Vec<u8>, JournalError> {
    let payload = serde_json::to_vec(rec).map_err(|e| JournalError::Corrupt(e.to_string()))?;
    let mut out = Vec::with_capacity(4 + 4 + 4 + payload.len());
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

fn scan_tail(file: &mut File) -> Result<(OpSeq, Hash256), JournalError> {
    let records = read_all_records(file)?;
    if let Some(last) = records.last() {
        let bytes = encode_frame(last)?;
        Ok((last.op_seq, frame_hash(&bytes)))
    } else {
        Ok((OpSeq::ZERO, Hash256::ZERO))
    }
}

fn read_all_records(file: &mut File) -> Result<Vec<JournalRecord>, JournalError> {
    file.seek(SeekFrom::Start(0))
        .map_err(|e| JournalError::Io(e.to_string()))?;
    let mut data = Vec::new();
    file.read_to_end(&mut data)
        .map_err(|e| JournalError::Io(e.to_string()))?;

    let mut records = Vec::new();
    let mut i = 0usize;
    while i + 12 <= data.len() {
        let magic = u32::from_le_bytes(data[i..i + 4].try_into().unwrap());
        if magic != FRAME_MAGIC {
            return Err(JournalError::Corrupt(format!("bad magic at {i}")));
        }
        let len = u32::from_le_bytes(data[i + 4..i + 8].try_into().unwrap()) as usize;
        let crc = u32::from_le_bytes(data[i + 8..i + 12].try_into().unwrap());
        let start = i + 12;
        let end = start + len;
        if end > data.len() {
            // Torn trailing frame — stop without error (caller may truncate).
            break;
        }
        let payload = &data[start..end];
        if crc32c::crc32c(payload) != crc {
            // Torn/corrupt trailing — stop.
            break;
        }
        let rec: JournalRecord =
            serde_json::from_slice(payload).map_err(|e| JournalError::Corrupt(e.to_string()))?;
        records.push(rec);
        i = end;
    }
    Ok(records)
}

/// Helper to build a record with correct chain linkage.
pub fn make_record(
    prev_hash: Hash256,
    seq: OpSeq,
    op_id: OpId,
    document_id: DocumentId,
    actor_id: ActorId,
    session_id: SessionId,
    payload: Operation,
) -> JournalRecord {
    let content_hash = Hash256::sha256(format!("{payload:?}").as_bytes());
    JournalRecord {
        schema_version: 1,
        op_id,
        op_seq: seq,
        document_id,
        actor_id,
        session_id,
        committed_at: Timestamp { unix_ms: 0 },
        parents: if seq.0 <= 1 {
            vec![]
        } else {
            vec![OpSeq(seq.0 - 1)]
        },
        content_hash,
        prev_record_hash: prev_hash,
        payload,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::{BlockId, IdGen, SeqIdGen, TextOffset};
    use std::env::temp_dir;

    #[test]
    fn append_and_replay() {
        let mut gen = SeqIdGen::default();
        let path = temp_dir().join(format!("qf-journal-{}.log", gen.new_uuid()));
        let _ = std::fs::remove_file(&path);

        let mut j = Journal::open_or_create(&path).unwrap();
        let doc = DocumentId::new(&mut gen);
        let actor = ActorId::new(&mut gen);
        let session = SessionId::new(&mut gen);
        let op = Operation::InsertText {
            block_id: BlockId::new(&mut gen),
            at: TextOffset(0),
            text: "hi".into(),
        };
        let rec = make_record(
            Hash256::ZERO,
            OpSeq(1),
            OpId::new(&mut gen),
            doc,
            actor,
            session,
            op,
        );
        j.append_batch(&[rec]).unwrap();
        assert_eq!(j.last_seq(), OpSeq(1));
        j.verify_chain().unwrap();
        let all = j.iter_from(OpSeq::ZERO).unwrap();
        assert_eq!(all.len(), 1);

        let _ = std::fs::remove_file(&path);
    }
}
