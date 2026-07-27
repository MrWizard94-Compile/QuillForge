//! Quillforge application entry.
//!
//! Wires the canonical mutation path:
//! `Command → Operations → Journal → Kernel → Derived State`

use qf_blocks::BlockBody;
use qf_core::{
    Command, ParentRef, TextOffset, TextTarget, Version,
};
use qf_export::{CoreExporter, ExportFormat, ExportRequest, Exporter};
use qf_storage::DocumentSession;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".into());

    match cmd.as_str() {
        "demo" => match run_demo(args.next()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        "version" => {
            println!("quillforge {}", env!("CARGO_PKG_VERSION"));
            println!("package format {}", Version::V1_0_0);
            ExitCode::SUCCESS
        }
        "help" | "-h" | "--help" => {
            print_help();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown command: {other}");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    println!("Quillforge — local-first document OS");
    println!();
    println!("Usage:");
    println!("  quillforge demo [path.qfdoc]   Run canonical-path demo");
    println!("  quillforge version             Print version");
    println!("  quillforge help                Show this help");
    println!();
    println!("Architecture: Command → Operations → Journal → Kernel → Derived");
}

fn run_demo(path: Option<String>) -> Result<(), String> {
    let work = PathBuf::from("target");
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let journal = work.join("demo-session.log");
    let _ = std::fs::remove_file(&journal);

    let mut session =
        DocumentSession::create_default(&journal).map_err(|e| e.to_string())?;

    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: vec![],
        })
        .map_err(|e| e.to_string())?;

    let block_id = *session
        .state()
        .document
        .blocks
        .keys()
        .next()
        .ok_or("missing block")?;

    let commit = session
        .commit_command(Command::InsertText {
            target: TextTarget {
                block_id,
                span_id: None,
                offset: TextOffset(0),
            },
            text: "Hello, Quillforge.".into(),
        })
        .map_err(|e| e.to_string())?;

    let payload = vec![0xca, 0xfe, 0xba, 0xbe];
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 1,
            body_type: "com.example.custom/v1".into(),
            body_payload: payload.clone(),
        })
        .map_err(|e| e.to_string())?;

    // Undo/redo smoke
    session
        .commit_command(Command::Undo { steps: 1 })
        .map_err(|e| e.to_string())?;
    session
        .commit_command(Command::Redo { steps: 1 })
        .map_err(|e| e.to_string())?;

    let text = session
        .state()
        .document
        .project_text(block_id)
        .unwrap_or_default();
    println!("canonical text: {text}");
    println!("op_seq: {}", commit.result.last_seq.0);

    let unknown = session
        .state()
        .document
        .blocks
        .values()
        .find_map(|b| match &b.body {
            BlockBody::Unknown(u) => Some(u),
            _ => None,
        })
        .ok_or("expected unknown block")?;
    if unknown.original_payload != payload {
        return Err("unknown payload mutated".into());
    }
    println!(
        "unknown block preserved: type={} bytes={}",
        unknown.original_type,
        unknown.original_payload.len()
    );

    let exporter = CoreExporter;
    let out = exporter
        .export(
            session.state(),
            &ExportRequest {
                format: ExportFormat::Markdown,
                config: Default::default(),
            },
        )
        .map_err(|e| e.to_string())?;
    println!("export markdown:\n{}", String::from_utf8_lossy(&out.bytes));

    let pkg_path = path
        .map(PathBuf::from)
        .unwrap_or_else(|| work.join("demo.qfdoc"));
    session.save(&pkg_path).map_err(|e| e.to_string())?;
    println!("saved package: {}", pkg_path.display());

    let journal2 = work.join("demo-reopen.log");
    let _ = std::fs::remove_file(&journal2);
    let mut gen = qf_core::SeqIdGen::default();
    let actor = qf_core::ActorId::new(&mut gen);
    let sess = qf_core::SessionId::new(&mut gen);
    let (opened, notes) = DocumentSession::open_package(
        &pkg_path,
        &journal2,
        actor,
        sess,
        Box::new(gen),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "reopened ok; blocks={} notes={notes:?}",
        opened.state().document.blocks.len()
    );
    println!("demo complete — canonical path verified");
    Ok(())
}
