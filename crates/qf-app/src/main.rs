//! Quillforge CLI — exercises the editor controller (primary user workflow).

use qf_app::EditorController;
use qf_blocks::BlockBody;
use qf_core::{ParentRef, Version};
use qf_export::{CoreExporter, ExportFormat, ExportRequest, Exporter};
use qf_input::KeyEvent;
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
    println!("  quillforge demo [path.qfdoc]   Run editor + package demo");
    println!("  quillforge version             Print version");
    println!("  quillforge help                Show this help");
    println!();
    println!("Law: AGENTS Constitution/AGENTS.md + WORKFLOW.md");
    println!("Architecture: Input → Command → Journal → Kernel → Layout");
}

fn run_demo(path: Option<String>) -> Result<(), String> {
    let work = PathBuf::from("target");
    std::fs::create_dir_all(&work).map_err(|e| e.to_string())?;
    let journal = work.join("demo-session.log");
    let _ = std::fs::remove_file(&journal);

    let mut editor = EditorController::create_default(&journal).map_err(|e| e.to_string())?;
    editor
        .ensure_editable_paragraph()
        .map_err(|e| e.to_string())?;

    for ch in "Hello, Quillforge.".chars() {
        editor
            .handle_key(KeyEvent::Char(ch))
            .map_err(|e| e.to_string())?;
    }

    editor.handle_key(KeyEvent::Enter).map_err(|e| e.to_string())?;
    editor
        .handle_text("Line two")
        .map_err(|e| e.to_string())?;

    // Unknown block via session for preservation demo
    editor
        .session
        .commit_command(qf_core::Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 2,
            body_type: "com.example.custom/v1".into(),
            body_payload: vec![0xca, 0xfe, 0xba, 0xbe],
        })
        .map_err(|e| e.to_string())?;
    editor
        .handle_key(KeyEvent::Undo)
        .map_err(|e| e.to_string())?;
    editor
        .handle_key(KeyEvent::Redo)
        .map_err(|e| e.to_string())?;

    let text = editor.focused_text().unwrap_or_default();
    println!("focused text: {text}");
    println!("op_seq: {}", editor.op_seq().0);
    println!(
        "layout pages: {} hash: {:02x?}",
        editor.layout_tree.pages.len(),
        &editor.layout_tree.content_hash().0[..4]
    );

    let unknown = editor
        .session
        .state()
        .document
        .blocks
        .values()
        .find_map(|b| match &b.body {
            BlockBody::Unknown(u) => Some(u),
            _ => None,
        });
    if let Some(u) = unknown {
        println!(
            "unknown block preserved: type={} bytes={}",
            u.original_type,
            u.original_payload.len()
        );
    }

    let exporter = CoreExporter;
    let out = exporter
        .export(
            editor.session.state(),
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
    editor.save(&pkg_path).map_err(|e| e.to_string())?;
    println!("saved package: {}", pkg_path.display());

    let journal2 = work.join("demo-reopen.log");
    let _ = std::fs::remove_file(&journal2);
    let mut gen = qf_core::SeqIdGen::default();
    let (session, notes) = DocumentSession::open_package(
        &pkg_path,
        &journal2,
        qf_core::ActorId::new(&mut gen),
        qf_core::SessionId::new(&mut gen),
        Box::new(gen),
    )
    .map_err(|e| e.to_string())?;
    println!(
        "reopened ok; blocks={} notes={notes:?}",
        session.state().document.blocks.len()
    );
    println!("demo complete — editor + canonical path verified");
    Ok(())
}
