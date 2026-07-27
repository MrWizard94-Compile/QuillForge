//! Integration tests for the canonical mutation path, recovery, and unknown content.
//!
//! These tests are release gates for Phase 1–2.

use qf_blocks::BlockBody;
use qf_core::{
    ActorId, BlockBodyBytes, BlockId, Command, ParentRef, SeqIdGen, SessionId, TextOffset,
    TextTarget,
};
use qf_storage::{lkg_path, DocumentSession};
use std::fs;

#[test]
fn create_edit_save_open_preserves_state() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("work.log");
    let package = dir.path().join("doc.qfdoc");

    let mut session = DocumentSession::create_default(&journal).unwrap();
    let out = session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: vec![],
        })
        .unwrap();
    assert_eq!(out.result.last_seq.0, 1);

    let block_id = *session
        .state()
        .document
        .blocks
        .keys()
        .next()
        .expect("block");

    session
        .commit_command(Command::InsertText {
            target: TextTarget {
                block_id,
                span_id: None,
                offset: TextOffset(0),
            },
            text: "Hello, Quillforge.".into(),
        })
        .unwrap();

    let hash = session.kernel.content_hash();
    session.save(&package).unwrap();

    let journal2 = dir.path().join("work2.log");
    let mut gen = SeqIdGen::default();
    let actor = ActorId::new(&mut gen);
    let sess = SessionId::new(&mut gen);
    let (session2, _notes) =
        DocumentSession::open_package(&package, &journal2, actor, sess, Box::new(gen)).unwrap();

    assert_eq!(session2.kernel.content_hash(), hash);
    assert_eq!(
        session2.state().document.project_text(block_id).unwrap(),
        "Hello, Quillforge."
    );
}

#[test]
fn unknown_block_round_trip_byte_identity() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("u.log");
    let package = dir.path().join("u.qfdoc");
    let mut session = DocumentSession::create_default(&journal).unwrap();

    let payload = vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0xff, 0x10];
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "com.example.custom/v1".into(),
            body_payload: payload.clone(),
        })
        .unwrap();

    // Adjacent paragraph edit must not touch unknown payload.
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: vec![],
        })
        .unwrap();

    let unknown_id = session
        .state()
        .document
        .blocks
        .iter()
        .find_map(|(id, b)| match &b.body {
            BlockBody::Unknown(_) => Some(*id),
            _ => None,
        })
        .unwrap();

    session.save(&package).unwrap();

    let mut gen = SeqIdGen::default();
    let (session2, _) = DocumentSession::open_package(
        &package,
        &dir.path().join("u2.log"),
        ActorId::new(&mut gen),
        SessionId::new(&mut gen),
        Box::new(gen),
    )
    .unwrap();

    match &session2
        .state()
        .document
        .blocks
        .get(&unknown_id)
        .unwrap()
        .body
    {
        BlockBody::Unknown(u) => {
            assert_eq!(u.original_type, "com.example.custom/v1");
            assert_eq!(u.original_payload, payload);
        }
        _ => panic!("expected unknown"),
    }
}

#[test]
fn undo_redo_restores_text() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = DocumentSession::create_default(&dir.path().join("ur.log")).unwrap();

    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: vec![],
        })
        .unwrap();
    let block_id = *session.state().document.blocks.keys().next().unwrap();
    session
        .commit_command(Command::InsertText {
            target: TextTarget {
                block_id,
                span_id: None,
                offset: TextOffset(0),
            },
            text: "abc".into(),
        })
        .unwrap();
    assert_eq!(
        session.state().document.project_text(block_id).unwrap(),
        "abc"
    );

    session.commit_command(Command::Undo { steps: 1 }).unwrap();
    assert_eq!(session.state().document.project_text(block_id).unwrap(), "");

    session.commit_command(Command::Redo { steps: 1 }).unwrap();
    assert_eq!(
        session.state().document.project_text(block_id).unwrap(),
        "abc"
    );
}

#[test]
fn atomic_save_creates_lkg() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("a.log");
    let package = dir.path().join("a.qfdoc");
    let mut session = DocumentSession::create_default(&journal).unwrap();
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: b"one".to_vec(),
        })
        .unwrap();
    session.save(&package).unwrap();
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 1,
            body_type: "paragraph".into(),
            body_payload: b"two".to_vec(),
        })
        .unwrap();
    session.save(&package).unwrap();
    assert!(lkg_path(&package).exists());
    assert!(fs::metadata(&package).unwrap().len() > 0);
}

#[test]
fn dry_run_failure_does_not_advance_journal() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = DocumentSession::create_default(&dir.path().join("d.log")).unwrap();
    let before = session.journal.last_seq();
    // Insert text without a block → expand fails before journal.
    let mut gen = SeqIdGen::default();
    let err = session
        .commit_command(Command::InsertText {
            target: TextTarget {
                block_id: BlockId::new(&mut gen),
                span_id: None,
                offset: TextOffset(0),
            },
            text: "nope".into(),
        })
        .unwrap_err();
    assert!(matches!(err, qf_core::CommandError::InvalidTarget));
    assert_eq!(session.journal.last_seq(), before);
}

#[test]
fn journal_replay_matches_live_hash() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("r.log");
    let package = dir.path().join("r.qfdoc");
    let mut session = DocumentSession::create_default(&journal).unwrap();
    let block_id = BlockId::new(session.ids.as_mut());
    session
        .commit_ops(vec![qf_core::Operation::InsertBlock {
            block_id,
            parent: ParentRef::DocumentRoot,
            index: 0,
            body: BlockBodyBytes {
                type_name: "paragraph".into(),
                payload: b"replay".to_vec(),
            },
        }])
        .unwrap();
    let live_hash = session.kernel.content_hash();
    session.save(&package).unwrap();

    let mut gen = SeqIdGen::default();
    let (opened, _) = DocumentSession::open_package(
        &package,
        &dir.path().join("r2.log"),
        ActorId::new(&mut gen),
        SessionId::new(&mut gen),
        Box::new(gen),
    )
    .unwrap();
    assert_eq!(opened.kernel.content_hash(), live_hash);
}

#[test]
fn other_files_preserved_in_package() {
    use qf_storage::{read_qfdoc, write_qfdoc, PackageStore};
    let dir = tempfile::tempdir().unwrap();
    let mut session = DocumentSession::create_default(&dir.path().join("o.log")).unwrap();
    session
        .other_files
        .insert("extra/note.txt".into(), b"keepme".to_vec());
    let pkg_path = dir.path().join("o.qfdoc");
    session.save(&pkg_path).unwrap();
    let bytes = fs::read(&pkg_path).unwrap();
    let pkg = read_qfdoc(&bytes).unwrap();
    assert_eq!(
        pkg.other_files.get("extra/note.txt").map(|v| v.as_slice()),
        Some(b"keepme".as_slice())
    );
    let _ = PackageStore::default();
    let _ = write_qfdoc(&pkg).unwrap();
}

#[test]
fn rejects_zip_path_traversal_names() {
    use qf_storage::validate_member_name;
    assert!(validate_member_name("ok/path.txt").is_ok());
    assert!(validate_member_name("../secret").is_err());
}

#[test]
fn quarantine_copy_preserves_original() {
    use qf_storage::quarantine_copy;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doc.qfdoc");
    fs::write(&path, b"primary").unwrap();
    let q = quarantine_copy(&path).unwrap();
    assert!(q.exists());
    assert_eq!(fs::read(&path).unwrap(), b"primary");
    assert_eq!(fs::read(&q).unwrap(), b"primary");
    assert_ne!(q, path);
}
