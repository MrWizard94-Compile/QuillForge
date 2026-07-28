//! Quillforge CLI (WPAI Software) — editor controller and demos.

use qf_app::{run_gui, EditorController, ProductSurface, ShellSession};
use qf_blocks::BlockBody;
use qf_core::{
    cli_banner, ActorId, DocumentId, Hash256, Operation, ParentRef, SeqIdGen, SessionId, Version,
    FOUNDER_CREDIT, HUMAN_DIRECTED_BADGE, PRODUCT_DIVISION, PRODUCT_NAME, STUDIO_DISCLOSURE,
    STUDIO_LEGAL, STUDIO_SHORT, STUDIO_SITE, STUDIO_TAGLINE, STUDIO_THESIS,
};
use qf_export::{CoreExporter, ExportFormat, ExportRequest, Exporter};
use qf_input::KeyEvent;
use qf_storage::DocumentSession;
use qf_sync::{
    Hello, PullRequest, PushRequest, SyncHub, TcpSyncClient, TcpSyncServer, TlsIdentity,
    TlsSyncClient, TlsSyncServer,
};
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
        "sync-demo" => match run_sync_demo() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        "tls-sync-demo" => match run_tls_sync_demo() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        "version" => {
            println!("{}", cli_banner());
            println!(
                "{PRODUCT_NAME} {} · package format {}",
                env!("CARGO_PKG_VERSION"),
                Version::V1_0_0
            );
            println!(
                "{STUDIO_LEGAL} ({STUDIO_SHORT}) · {PRODUCT_NAME} is a {PRODUCT_DIVISION} product"
            );
            println!("{HUMAN_DIRECTED_BADGE}");
            println!("Studio: {STUDIO_SITE}");
            println!("Founder: {FOUNDER_CREDIT}");
            ExitCode::SUCCESS
        }
        "surface" => {
            print_product_surface();
            ExitCode::SUCCESS
        }
        "review-demo" => match run_review_demo() {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        "import-docx" => {
            let path = args.next();
            let name = args.next().unwrap_or_else(|| "Imported".into());
            match run_import_docx(path, name) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        },
        "edit" | "gui" => {
            let work = PathBuf::from("target/quill-gui");
            let name = args.next().unwrap_or_else(|| "Document".into());
            match run_gui(work, &name) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
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

fn run_import_docx(path: Option<String>, name: String) -> Result<(), String> {
    let Some(path) = path else {
        return Err("usage: quillforge import-docx <file.docx> [name]".into());
    };
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(format!("file not found: {}", path.display()));
    }
    let work = PathBuf::from("target/quill-import");
    let session =
        ShellSession::open_from_docx(&work, &name, &path).map_err(|e| e.to_string())?;
    println!("imported: {}", path.display());
    println!("package:  {}", session.package_path.display());
    println!("blocks:   {}", session.editor.page_count()); // at least pages
    println!("words:    {}", session.editor.word_count());
    println!("plain preview:\n{}", session.editor.document_plain());
    println!("import-docx complete — DOCX → .qfdoc via journal");
    Ok(())
}

fn run_review_demo() -> Result<(), String> {
    let work = PathBuf::from("target/quill-review-demo");
    let mut session = ShellSession::open_or_create(&work, "ReviewDemo").map_err(|e| e.to_string())?;
    session.draft = "Word-class review loop starts here.".into();
    session.apply_draft().map_err(|e| e.to_string())?;
    let before = session.draft.clone();
    println!("before: {before}");
    session.review.instruction = "annotate".into();
    let id = session.propose_suggestion().map_err(|e| e.to_string())?;
    println!(
        "pending after propose: {}",
        session.surface.status.pending_suggestions
    );
    session.select_suggestion(id);
    session.accept_selected().map_err(|e| e.to_string())?;
    println!("after accept: {}", session.draft);
    assert_ne!(session.draft, before);

    session.draft = session.editor.focused_text().unwrap_or_default();
    let mid = session.draft.clone();
    let id2 = session.propose_suggestion().map_err(|e| e.to_string())?;
    session.select_suggestion(id2);
    session.reject_selected().map_err(|e| e.to_string())?;
    assert_eq!(session.editor.focused_text().unwrap_or_default(), mid);
    println!("after reject: text unchanged ({mid})");
    println!("review-demo complete — accept/reject journal path verified");
    Ok(())
}

fn print_product_surface() {
    println!("{}", ProductSurface::category_line());
    let surface = ProductSurface::word_class_default("Untitled");
    println!("Window: {}", surface.window_title);
    println!("View:   {:?} (default page/print layout)", surface.view_mode);
    println!("Tab:    {}", surface.active_tab.label());
    println!("Status: {}", surface.status.summary_line());
    println!();
    println!("Command bar tabs:");
    for tab in qf_app::CommandBarTab::all() {
        println!("  · {}", tab.label());
    }
    println!();
    println!("Active tab groups ({}):", surface.active_tab.label());
    for g in surface.active_groups() {
        let actions: Vec<_> = g.actions.iter().map(|a| a.label.as_str()).collect();
        println!("  [{}] {}", g.label, actions.join(", "));
    }
    println!();
    println!("UX law: docs/28-product-surface.md");
    println!("No Microsoft trademarks in chrome.");
}

fn print_help() {
    println!("{}", cli_banner());
    println!("{}", ProductSurface::category_line());
    println!("{STUDIO_LEGAL} — {STUDIO_TAGLINE}");
    println!("{STUDIO_THESIS}");
    println!("{HUMAN_DIRECTED_BADGE}");
    println!();
    println!("Usage:");
    println!("  quillforge demo [path.qfdoc]   Run editor + package demo");
    println!("  quillforge surface             Show Word-class product chrome model");
    println!("  quillforge edit [name]         Open Word-class GUI shell");
    println!("  quillforge review-demo         Propose / accept / reject suggestions");
    println!("  quillforge import-docx <file> [name]  Import .docx → .qfdoc");
    println!("  quillforge sync-demo           Localhost TCP multi-client sync demo");
    println!("  quillforge tls-sync-demo       Localhost TLS (rustls) sync demo");
    println!("  quillforge version             Print version and WPAI identity");
    println!("  quillforge help                Show this help");
    println!();
    println!("Law: AGENTS Constitution/AGENTS.md + SOP.md");
    println!("Architecture: Input → Command → Journal → Kernel → Layout");
    println!("Product surface: Word-class editor UX (no Microsoft branding)");
    println!("Brand: brand/ (WPAI forge visual + voice)");
    println!();
    println!("{STUDIO_DISCLOSURE}");
}

fn run_tls_sync_demo() -> Result<(), String> {
    let identity = TlsIdentity::generate_dev().map_err(|e| e.to_string())?;
    let server =
        TlsSyncServer::start(SyncHub::new(), identity.clone()).map_err(|e| e.to_string())?;
    let addr = server.addr();
    println!("TLS sync hub (QFSNC1 over rustls) listening on {addr}");

    let mut gen = SeqIdGen::default();
    let doc = DocumentId::new(&mut gen);
    let client = TlsSyncClient::connect(addr, &identity).map_err(|e| e.to_string())?;
    let ack = client
        .hello(Hello {
            client_id: ActorId::new(&mut gen),
            session_id: SessionId::new(&mut gen),
            document_id: doc,
            have_seq: 0,
        })
        .map_err(|e| e.to_string())?;
    let ops = vec![Operation::Noop {
        reason: qf_core::NoopReason::Other("tls-sync-demo".into()),
    }];
    let payload_hash = {
        let mut buf = Vec::new();
        for op in &ops {
            let s = format!("{op:?}");
            buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
            buf.extend_from_slice(s.as_bytes());
        }
        Hash256::sha256(&buf)
    };
    let push = client
        .push(PushRequest {
            token: ack.token.clone(),
            first_seq: 1,
            last_seq: 1,
            ops,
            crdt: None,
            payload_hash,
        })
        .map_err(|e| e.to_string())?;
    println!("TLS client pushed; head={}", push.accepted_head_seq);
    let pull = client
        .pull(PullRequest {
            token: ack.token,
            have_seq: 0,
        })
        .map_err(|e| e.to_string())?;
    println!(
        "TLS client pulled; head={} ops={}",
        pull.server_head_seq,
        pull.ops.len()
    );
    server.stop();
    println!("tls-sync-demo complete — QFSNC1 over TLS 1.3 verified");
    Ok(())
}

fn run_sync_demo() -> Result<(), String> {
    let server = TcpSyncServer::start(SyncHub::new()).map_err(|e| e.to_string())?;
    let addr = server.addr();
    println!("sync hub listening on {addr}");

    let mut gen = SeqIdGen::default();
    let doc = DocumentId::new(&mut gen);
    let client_a = TcpSyncClient::connect(addr);
    let client_b = TcpSyncClient::connect(addr);

    let ack_a = client_a
        .hello(Hello {
            client_id: ActorId::new(&mut gen),
            session_id: SessionId::new(&mut gen),
            document_id: doc,
            have_seq: 0,
        })
        .map_err(|e| e.to_string())?;
    let ops = vec![Operation::Noop {
        reason: qf_core::NoopReason::Other("sync-demo".into()),
    }];
    let payload_hash = {
        let mut buf = Vec::new();
        for op in &ops {
            let s = format!("{op:?}");
            buf.extend_from_slice(&(s.len() as u32).to_le_bytes());
            buf.extend_from_slice(s.as_bytes());
        }
        Hash256::sha256(&buf)
    };
    let push = client_a
        .push(PushRequest {
            token: ack_a.token,
            first_seq: 1,
            last_seq: 1,
            ops,
            crdt: None,
            payload_hash,
        })
        .map_err(|e| e.to_string())?;
    println!("client A pushed; head={}", push.accepted_head_seq);

    let ack_b = client_b
        .hello(Hello {
            client_id: ActorId::new(&mut gen),
            session_id: SessionId::new(&mut gen),
            document_id: doc,
            have_seq: 0,
        })
        .map_err(|e| e.to_string())?;
    let pull = client_b
        .pull(PullRequest {
            token: ack_b.token,
            have_seq: 0,
        })
        .map_err(|e| e.to_string())?;
    println!(
        "client B pulled; head={} ops={}",
        pull.server_head_seq,
        pull.ops.len()
    );
    server.stop();
    println!("sync-demo complete — TCP framed protocol verified");
    Ok(())
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

    for ch in "Hello, Quillforge — WPAI.".chars() {
        editor
            .handle_key(KeyEvent::Char(ch))
            .map_err(|e| e.to_string())?;
    }

    editor
        .handle_key(KeyEvent::Enter)
        .map_err(|e| e.to_string())?;
    editor.handle_text("Line two").map_err(|e| e.to_string())?;

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
    let md = exporter
        .export(
            editor.session.state(),
            &ExportRequest {
                format: ExportFormat::Markdown,
                config: Default::default(),
            },
        )
        .map_err(|e| e.to_string())?;
    println!("export markdown:\n{}", String::from_utf8_lossy(&md.bytes));

    let pdf = exporter
        .export(
            editor.session.state(),
            &ExportRequest {
                format: ExportFormat::Pdf,
                config: Default::default(),
            },
        )
        .map_err(|e| e.to_string())?;
    println!("export pdf: {} bytes (PDF-1.4)", pdf.bytes.len());

    let docx = exporter
        .export(
            editor.session.state(),
            &ExportRequest {
                format: ExportFormat::Docx,
                config: Default::default(),
            },
        )
        .map_err(|e| e.to_string())?;
    println!("export docx: {} bytes (OOXML)", docx.bytes.len());
    let docx_path = work.join("demo.docx");
    std::fs::write(&docx_path, &docx.bytes).map_err(|e| e.to_string())?;
    println!("wrote docx: {}", docx_path.display());

    let pkg_path = path
        .map(PathBuf::from)
        .unwrap_or_else(|| work.join("demo.qfdoc"));
    editor.save(&pkg_path).map_err(|e| e.to_string())?;
    println!("saved package: {}", pkg_path.display());

    let enc_path = work.join("demo-encrypted.qfdoc");
    editor
        .save_encrypted(&enc_path, "demo-passphrase")
        .map_err(|e| e.to_string())?;
    println!("saved encrypted package: {}", enc_path.display());

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

    let journal3 = work.join("demo-reopen-enc.log");
    let _ = std::fs::remove_file(&journal3);
    let mut gen = qf_core::SeqIdGen::default();
    let (enc_session, enc_notes) = DocumentSession::open_package_with_passphrase(
        &enc_path,
        &journal3,
        qf_core::ActorId::new(&mut gen),
        qf_core::SessionId::new(&mut gen),
        Box::new(gen),
        "demo-passphrase",
    )
    .map_err(|e| e.to_string())?;
    println!(
        "reopened encrypted ok; blocks={} notes={enc_notes:?}",
        enc_session.state().document.blocks.len()
    );
    println!("demo complete — editor + export + encrypt path verified");
    Ok(())
}
