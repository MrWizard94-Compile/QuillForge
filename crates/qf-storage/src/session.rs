//! Document session: commit coordinator, undo/redo, save/open.
//!
//! # Commit protocol (normative)
//!
//! 1. Expand command → operations (or accept raw ops)
//! 2. Dry-run apply on a kernel clone
//! 3. Durable journal append + fsync per op
//! 4. Live kernel apply (must succeed if dry-run was pure)
//! 5. Record inverse batch for undo
//!
//! Journal is never truncated. Undo commits inverse ops as new journal records.

use crate::package::{OpenOptions, OpenOutcome, PackageStore, FORMAT_VERSION};
use qf_core::{
    next_seq, ActorId, Command, CommandError, CommitResult, IdGen, OpBatch, OpSeq, Operation,
    SeqIdGen, SessionId, Timestamp,
};
use qf_doc::{expand_command, Document};
use qf_journal::{make_record, Journal, RecordDraft};
use qf_kernel::{invert_ops, CanonicalState, EntityTables, Kernel, KernelEvent};
use std::path::{Path, PathBuf};

/// Operation-based undo/redo (not snapshot-based).
///
/// - `undo_stack`: inverse batches to apply when undoing
/// - `forward_stack`: original forward batches (parallel to undo_stack) for redo
/// - `redo_stack`: forward batches available after undo
#[derive(Debug, Default)]
pub struct UndoService {
    undo_stack: Vec<OpBatch>,
    forward_stack: Vec<OpBatch>,
    redo_stack: Vec<OpBatch>,
}

impl UndoService {
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    fn on_edit(&mut self, forward: Vec<Operation>, inverses: Vec<Operation>) {
        if forward.is_empty() {
            return;
        }
        self.undo_stack.push(OpBatch { ops: inverses });
        self.forward_stack.push(OpBatch { ops: forward });
        self.redo_stack.clear();
    }

    /// After a successful redo apply, record undo without clearing remaining redo.
    fn on_redo_applied(&mut self, forward: Vec<Operation>, inverses: Vec<Operation>) {
        if forward.is_empty() {
            return;
        }
        self.undo_stack.push(OpBatch { ops: inverses });
        self.forward_stack.push(OpBatch { ops: forward });
    }

    fn pop_undo(&mut self) -> Option<(OpBatch, OpBatch)> {
        let inv = self.undo_stack.pop()?;
        let fwd = self.forward_stack.pop()?;
        Some((inv, fwd))
    }

    fn push_redo(&mut self, forward: OpBatch) {
        self.redo_stack.push(forward);
    }

    fn pop_redo(&mut self) -> Option<OpBatch> {
        self.redo_stack.pop()
    }
}

#[derive(Debug)]
pub struct CommitOutcome {
    pub result: CommitResult,
    pub events: Vec<KernelEvent>,
}

/// Live editing session bound to a journal file and optional package path.
pub struct DocumentSession {
    pub kernel: Kernel,
    pub journal: Journal,
    pub package_path: Option<PathBuf>,
    pub actor: ActorId,
    pub session: SessionId,
    pub ids: Box<dyn IdGen + Send>,
    pub undo: UndoService,
    pub store: PackageStore,
    pub assets: std::collections::BTreeMap<String, Vec<u8>>,
    pub other_files: std::collections::BTreeMap<String, Vec<u8>>,
    writable: bool,
}

impl DocumentSession {
    /// Create a new empty document with a journal sidecar path.
    pub fn create(
        journal_path: &Path,
        actor: ActorId,
        session: SessionId,
        mut ids: Box<dyn IdGen + Send>,
    ) -> Result<Self, CommandError> {
        let doc_id = qf_core::DocumentId::new(ids.as_mut());
        let root = qf_core::SectionId::new(ids.as_mut());
        let document = Document::empty(doc_id, root, actor, Timestamp { unix_ms: 0 });
        let state = CanonicalState {
            document_id: doc_id,
            package_format_version: FORMAT_VERSION,
            op_seq: OpSeq::ZERO,
            document,
            entities: EntityTables::default(),
        };
        let journal = Journal::open_or_create(journal_path)
            .map_err(|e| CommandError::Journal(e.to_string()))?;
        Ok(Self {
            kernel: Kernel::new(state),
            journal,
            package_path: None,
            actor,
            session,
            ids,
            undo: UndoService::default(),
            store: PackageStore::default(),
            assets: Default::default(),
            other_files: Default::default(),
            writable: true,
        })
    }

    pub fn create_default(journal_path: &Path) -> Result<Self, CommandError> {
        let mut gen = SeqIdGen::default();
        let actor = ActorId::new(&mut gen);
        let session = SessionId::new(&mut gen);
        Self::create(journal_path, actor, session, Box::new(gen))
    }

    /// Open a `.qfdoc` package. Working journal is restored from package bytes.
    ///
    /// Package always stores the full journal so `journal.last_seq == kernel.op_seq`.
    pub fn open_package(
        package_path: &Path,
        journal_path: &Path,
        actor: ActorId,
        session: SessionId,
        ids: Box<dyn IdGen + Send>,
    ) -> Result<(Self, Vec<String>), CommandError> {
        let store = PackageStore::default();
        let outcome = store
            .open_path(package_path, OpenOptions::v1())
            .map_err(|e| CommandError::Internal(e.to_string()))?;

        let (kernel, journal_bytes, notes) = match outcome {
            OpenOutcome::Healthy {
                kernel,
                journal_bytes,
                notes,
                ..
            }
            | OpenOutcome::RecoveredFromLkg {
                kernel,
                journal_bytes,
                notes,
                ..
            } => (kernel, journal_bytes, notes),
            OpenOutcome::NeedsMigration { from, to } => {
                return Err(CommandError::Internal(format!(
                    "needs migration {from} -> {to}"
                )));
            }
            OpenOutcome::Corrupt { message, .. } => {
                return Err(CommandError::Internal(format!("corrupt: {message}")));
            }
        };

        let journal = Journal::from_bytes(journal_path, &journal_bytes)
            .map_err(|e| CommandError::Journal(e.to_string()))?;

        if journal.last_seq() != kernel.state().op_seq {
            return Err(CommandError::Internal(format!(
                "journal/kernel seq desync: journal={} kernel={}",
                journal.last_seq().0,
                kernel.state().op_seq.0
            )));
        }

        Ok((
            Self {
                kernel,
                journal,
                package_path: Some(package_path.to_path_buf()),
                actor,
                session,
                ids,
                undo: UndoService::default(),
                store,
                assets: Default::default(),
                other_files: Default::default(),
                writable: true,
            },
            notes,
        ))
    }

    pub fn state(&self) -> &CanonicalState {
        self.kernel.state()
    }

    /// Commit raw operations. Records undo inverses.
    pub fn commit_ops(&mut self, ops: Vec<Operation>) -> Result<CommitOutcome, CommandError> {
        let state_before = self.kernel.state().clone();
        let out = self.durable_apply(ops.clone())?;
        let inv =
            invert_ops(&state_before, &ops).map_err(|e| CommandError::Internal(e.to_string()))?;
        self.undo.on_edit(ops, inv);
        Ok(out)
    }

    /// Expand and commit a user command.
    pub fn commit_command(&mut self, command: Command) -> Result<CommitOutcome, CommandError> {
        match command {
            Command::Undo { steps } => self.undo_steps(steps),
            Command::Redo { steps } => self.redo_steps(steps),
            other => {
                let ops = expand_command(&self.kernel.state().document, self.ids.as_mut(), &other)?;
                self.commit_ops(ops)
            }
        }
    }

    fn durable_apply(&mut self, ops: Vec<Operation>) -> Result<CommitOutcome, CommandError> {
        if !self.writable {
            return Err(CommandError::NotWritable);
        }
        if ops.is_empty() {
            return Err(CommandError::Invalid("empty op batch".into()));
        }

        self.kernel
            .dry_run(&ops)
            .map_err(|e| CommandError::DryRun(e.to_string()))?;

        let mut events = Vec::new();
        let mut first_seq = None;
        let mut last_seq = OpSeq::ZERO;

        for op in &ops {
            let seq = next_seq(self.journal.last_seq());
            let rec = make_record(RecordDraft {
                prev_hash: self.journal.last_hash(),
                seq,
                op_id: qf_core::OpId::new(self.ids.as_mut()),
                document_id: self.kernel.state().document_id,
                actor_id: self.actor,
                session_id: self.session,
                committed_at: Timestamp { unix_ms: 0 },
                payload: op.clone(),
            });
            self.journal
                .append_batch(std::slice::from_ref(&rec))
                .map_err(|e| CommandError::Journal(e.to_string()))?;
            let ev = self.kernel.apply(op).map_err(|e| {
                CommandError::Internal(format!("CRITICAL: apply failed after journal write: {e}"))
            })?;
            if self.kernel.state().op_seq != seq {
                return Err(CommandError::Internal(format!(
                    "op_seq mismatch after apply: kernel={} journal={}",
                    self.kernel.state().op_seq.0,
                    seq.0
                )));
            }
            events.extend(ev);
            if first_seq.is_none() {
                first_seq = Some(seq);
            }
            last_seq = seq;
        }

        Ok(CommitOutcome {
            result: CommitResult {
                first_seq: first_seq.unwrap_or(last_seq),
                last_seq,
            },
            events,
        })
    }

    fn undo_steps(&mut self, steps: u32) -> Result<CommitOutcome, CommandError> {
        if steps == 0 {
            return Err(CommandError::Invalid("undo steps == 0".into()));
        }
        let mut all_events = Vec::new();
        let mut first = None;
        let mut last = OpSeq::ZERO;
        for _ in 0..steps {
            let (inv, forward) = self.undo.pop_undo().ok_or(CommandError::NothingToUndo)?;
            let out = self.durable_apply(inv.ops)?;
            self.undo.push_redo(forward);
            if first.is_none() {
                first = Some(out.result.first_seq);
            }
            last = out.result.last_seq;
            all_events.extend(out.events);
        }
        Ok(CommitOutcome {
            result: CommitResult {
                first_seq: first.unwrap_or(last),
                last_seq: last,
            },
            events: all_events,
        })
    }

    fn redo_steps(&mut self, steps: u32) -> Result<CommitOutcome, CommandError> {
        if steps == 0 {
            return Err(CommandError::Invalid("redo steps == 0".into()));
        }
        let mut all_events = Vec::new();
        let mut first = None;
        let mut last = OpSeq::ZERO;
        for _ in 0..steps {
            let forward = self.undo.pop_redo().ok_or(CommandError::NothingToRedo)?;
            let state_before = self.kernel.state().clone();
            let out = self.durable_apply(forward.ops.clone())?;
            let inv = invert_ops(&state_before, &forward.ops)
                .map_err(|e| CommandError::Internal(e.to_string()))?;
            self.undo.on_redo_applied(forward.ops, inv);
            if first.is_none() {
                first = Some(out.result.first_seq);
            }
            last = out.result.last_seq;
            all_events.extend(out.events);
        }
        Ok(CommitOutcome {
            result: CommitResult {
                first_seq: first.unwrap_or(last),
                last_seq: last,
            },
            events: all_events,
        })
    }

    /// Atomic save to `.qfdoc` with full journal + checkpoint.
    pub fn save(&mut self, path: &Path) -> Result<(), CommandError> {
        let journal_bytes = self
            .journal
            .to_bytes()
            .map_err(|e| CommandError::Journal(e.to_string()))?;
        let package = self.store.package_from_state(
            self.kernel.state(),
            journal_bytes,
            self.assets.clone(),
            self.other_files.clone(),
        );
        self.store
            .save_package(path, &package)
            .map_err(|e| CommandError::Internal(e.to_string()))?;
        self.package_path = Some(path.to_path_buf());
        Ok(())
    }
}
