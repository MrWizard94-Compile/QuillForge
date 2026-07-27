//! Quillforge application entry.
//!
//! Wires the canonical mutation path:
//! Command → Operations → Journal → Kernel → Derived State

use qf_core::{
    ActorId, BlockBodyBytes, BlockId, Command, CommandError, CommandHandler, DocumentId, OpSeq,
    Operation, ParentRef, SectionId, SeqIdGen, SessionId, TextOffset, Timestamp, Version,
};
use qf_doc::Document;
use qf_export::{CoreExporter, ExportFormat, ExportRequest, Exporter};
use qf_journal::{make_record, Journal};
use qf_kernel::{CanonicalState, EntityTables, Kernel};
use qf_storage::PackageStore;
use std::env;
use std::path::PathBuf;

/// Minimal editing handler for foundation demo.
struct DemoHandler {
    block_id: BlockId,
}

impl CommandHandler for DemoHandler {
    fn execute(&self, command: Command) -> Result<Vec<Operation>, CommandError> {
        match command {
            Command::InsertText { target, text } => Ok(vec![Operation::InsertText {
                block_id: target.block_id,
                at: target.offset,
                text,
            }]),
            Command::InsertBlock {
                parent,
                index,
                body_type,
                body_payload,
            } => Ok(vec![Operation::InsertBlock {
                block_id: self.block_id,
                parent,
                index,
                body: BlockBodyBytes {
                    type_name: body_type,
                    payload: body_payload,
                },
            }]),
            other => Err(CommandError::Handler(format!("unsupported: {other:?}"))),
        }
    }
}

/// Commit coordinator: dry-run → journal → apply.
struct CommitCoordinator {
    journal: Journal,
    kernel: Kernel,
    actor: ActorId,
    session: SessionId,
    gen: SeqIdGen,
}

impl CommitCoordinator {
    fn commit(&mut self, ops: Vec<Operation>) -> Result<OpSeq, CommandError> {
        // Dry-run on a snapshot kernel before any durable write.
        let mut dry = Kernel::from_snapshot(self.kernel.snapshot())
            .map_err(|e| CommandError::DryRun(e.to_string()))?;
        dry.apply_batch(&ops)
            .map_err(|e| CommandError::DryRun(e.to_string()))?;

        for op in &ops {
            let seq = if self.journal.last_seq() == OpSeq::ZERO {
                OpSeq(1)
            } else {
                self.journal.last_seq().next()
            };
            let rec = make_record(
                self.journal.last_hash(),
                seq,
                qf_core::OpId::new(&mut self.gen),
                self.kernel.state().document_id,
                self.actor,
                self.session,
                op.clone(),
            );
            self.journal
                .append_batch(std::slice::from_ref(&rec))
                .map_err(|e| CommandError::Journal(e.to_string()))?;
            self.kernel
                .apply(op)
                .map_err(|e| CommandError::Internal(format!("apply after journal: {e}")))?;
        }

        Ok(self.kernel.state().op_seq)
    }
}

fn main() {
    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".into());

    match cmd.as_str() {
        "demo" => run_demo(args.next()),
        "version" => {
            println!("quillforge 0.0.0 (foundation)");
            println!("package format {}", Version::V1_0_0);
        }
        _ => {
            println!("Quillforge — local-first document OS");
            println!();
            println!("Usage:");
            println!("  quillforge demo [path.qfdoc]   Run foundation pipeline demo");
            println!("  quillforge version             Print version");
            println!();
            println!("Architecture: Command → Operations → Journal → Kernel → Derived");
        }
    }
}

fn run_demo(path: Option<String>) {
    let mut gen = SeqIdGen::default();
    let doc_id = DocumentId::new(&mut gen);
    let root = SectionId::new(&mut gen);
    let actor = ActorId::new(&mut gen);
    let session = SessionId::new(&mut gen);
    let block_id = BlockId::new(&mut gen);

    let document = Document::empty(doc_id, root, actor, Timestamp { unix_ms: 0 });
    let state = CanonicalState {
        document_id: doc_id,
        package_format_version: Version::V1_0_0,
        op_seq: OpSeq::ZERO,
        document,
        entities: EntityTables::default(),
    };

    let journal_path = PathBuf::from("target/demo-journal.log");
    let _ = std::fs::remove_file(&journal_path);
    let journal = Journal::open_or_create(&journal_path).expect("journal");
    let kernel = Kernel::new(state);

    let handler = DemoHandler { block_id };

    let mut coord = CommitCoordinator {
        journal,
        kernel,
        actor,
        session,
        gen,
    };

    // Insert paragraph block.
    let ops = handler
        .execute(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: b"".to_vec(),
        })
        .unwrap();
    coord.commit(ops).expect("commit block");

    // Insert text.
    let ops = handler
        .execute(Command::InsertText {
            target: qf_core::TextTarget {
                block_id,
                span_id: None,
                offset: TextOffset(0),
            },
            text: "Hello, Quillforge.".into(),
        })
        .unwrap();
    let seq = coord.commit(ops).expect("commit text");

    // Unknown block preservation.
    let unknown_id = BlockId::new(&mut coord.gen);
    let payload = vec![0xca, 0xfe, 0xba, 0xbe];
    coord
        .commit(vec![Operation::InsertBlock {
            block_id: unknown_id,
            parent: ParentRef::DocumentRoot,
            index: 1,
            body: BlockBodyBytes {
                type_name: "com.example.custom/v1".into(),
                payload: payload.clone(),
            },
        }])
        .expect("unknown block");

    let text = coord
        .kernel
        .state()
        .document
        .project_text(block_id)
        .unwrap_or_default();
    println!("canonical text: {text}");
    println!("op_seq: {}", seq.0);

    match &coord.kernel.state().document.blocks.get(&unknown_id).unwrap().body {
        qf_blocks::BlockBody::Unknown(u) => {
            assert_eq!(u.original_payload, payload);
            println!(
                "unknown block preserved: type={} bytes={}",
                u.original_type,
                u.original_payload.len()
            );
        }
        _ => panic!("expected unknown block"),
    }

    // Export derived view.
    let exporter = CoreExporter;
    let out = exporter
        .export(
            coord.kernel.state(),
            &ExportRequest {
                format: ExportFormat::Markdown,
                config: Default::default(),
            },
        )
        .unwrap();
    println!("export markdown:\n{}", String::from_utf8_lossy(&out.bytes));

    // Atomic package save.
    let pkg_path = path
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/demo.qfdoc"));
    let store = PackageStore::default();
    store
        .save_snapshot(&pkg_path, coord.kernel.state())
        .expect("save");
    println!("saved package: {}", pkg_path.display());

    match store.open_snapshot(&pkg_path).unwrap() {
        qf_storage::OpenOutcome::Healthy { kernel } => {
            println!(
                "reopened ok; blocks={}",
                kernel.state().document.blocks.len()
            );
        }
        other => println!("reopen outcome: {other:?}"),
    }

    println!("demo complete — canonical path verified");
}
