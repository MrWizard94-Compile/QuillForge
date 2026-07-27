//! Crash/recovery style tests (REC family).
//!
//! These simulate power-loss and corruption without killing the process:
//! torn journal frames, incomplete staging packages, LKG fallback.

use qf_core::{Command, ParentRef};
use qf_journal::Journal;
use qf_storage::{lkg_path, write_bytes_atomic, DocumentSession, OpenOptions, PackageStore};
use std::fs::{self, OpenOptions as FsOpen};
use std::io::Write;

#[test]
fn rec_torn_journal_trailing_frame_truncates() {
    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("ops.log");
    let mut session = DocumentSession::create_default(&jpath).unwrap();
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: b"ok".to_vec(),
        })
        .unwrap();
    let good_seq = session.journal.last_seq();
    let good_len = session.journal.valid_len();
    drop(session);

    // Simulate torn write after durable commit.
    {
        let mut f = FsOpen::new().append(true).open(&jpath).unwrap();
        f.write_all(&[0x51, 0x46, 0x4A, 0x52, 0x10, 0x00, 0x00, 0x00, 0x00, 0x00])
            .unwrap();
    }

    let j2 = Journal::open_or_create(&jpath).unwrap();
    assert_eq!(j2.last_seq(), good_seq);
    assert_eq!(j2.valid_len(), good_len);
    j2.verify_chain().unwrap();
}

#[test]
fn rec_incomplete_staging_does_not_replace_primary() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("doc.qfdoc");
    write_bytes_atomic(&path, b"GOOD_PRIMARY").unwrap();

    // Leftover staging file from a killed save.
    fs::write(dir.path().join(".doc.qfdoc.tmp-dead"), b"PARTIAL").unwrap();

    assert_eq!(fs::read(&path).unwrap(), b"GOOD_PRIMARY");
    // New atomic write succeeds and refreshes LKG from GOOD_PRIMARY.
    write_bytes_atomic(&path, b"GOOD_V2").unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"GOOD_V2");
    assert_eq!(fs::read(lkg_path(&path)).unwrap(), b"GOOD_PRIMARY");
}

#[test]
fn rec_corrupt_primary_opens_lkg() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("w.log");
    let path = dir.path().join("doc.qfdoc");
    let mut session = DocumentSession::create_default(&journal).unwrap();
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: b"v1".to_vec(),
        })
        .unwrap();
    session.save(&path).unwrap();
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 1,
            body_type: "paragraph".into(),
            body_payload: b"v2".to_vec(),
        })
        .unwrap();
    session.save(&path).unwrap();
    assert!(lkg_path(&path).exists());

    // Corrupt primary
    fs::write(&path, b"NOT_A_ZIP").unwrap();

    let store = PackageStore::default();
    let outcome = store.open_path(&path, OpenOptions::v1()).unwrap();
    match outcome {
        qf_storage::OpenOutcome::RecoveredFromLkg { kernel, notes, .. } => {
            assert!(!kernel.state().document.blocks.is_empty());
            assert!(!notes.is_empty());
        }
        other => panic!("expected LKG recovery, got {other:?}"),
    }
}

#[test]
fn rec_encrypted_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("e.log");
    let path = dir.path().join("secret.qfdoc");
    let mut session = DocumentSession::create_default(&journal).unwrap();
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: b"classified".to_vec(),
        })
        .unwrap();
    let hash = session.kernel.content_hash();
    let pkg = session.store.package_from_state(
        session.state(),
        session.journal.to_bytes().unwrap(),
        Default::default(),
        Default::default(),
    );
    let params = qf_security::KdfParams {
        m_kib: 8,
        t_cost: 1,
        p_cost: 1,
    };
    session
        .store
        .save_package_encrypted_with_params(&path, &pkg, "s3cret!", &params)
        .unwrap();

    let raw = fs::read(&path).unwrap();
    assert!(qf_security::is_encrypted_package(&raw));

    let store = PackageStore::default();
    let outcome = store
        .open_path(&path, OpenOptions::v1().with_passphrase("s3cret!"))
        .unwrap();
    match outcome {
        qf_storage::OpenOutcome::Healthy { kernel, .. } => {
            assert_eq!(kernel.content_hash(), hash);
        }
        other => panic!("expected healthy encrypted open: {other:?}"),
    }

    let bad = store.open_path(&path, OpenOptions::v1().with_passphrase("wrong"));
    assert!(bad.is_ok()); // returns Corrupt outcome
    assert!(matches!(
        bad.unwrap(),
        qf_storage::OpenOutcome::Corrupt { .. }
    ));
}

/// Simulate process kill after successful commit: `append_batch`/`commit` returned Ok
/// (fsync done) but the Journal handle is never Dropped cleanly.
#[test]
fn rec_commit_survives_handle_forget() {
    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("kill.log");
    let mut session = DocumentSession::create_default(&jpath).unwrap();
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: b"durable".to_vec(),
        })
        .unwrap();
    let seq = session.journal.last_seq();
    // Process-kill simulation: leak the session without Drop cleanup.
    // Durability contract: append_batch already fsync'd before Ok.
    std::mem::forget(session);

    let j2 = Journal::open_or_create(&jpath).unwrap();
    assert_eq!(j2.last_seq(), seq);
    assert!(j2.last_seq().0 >= 1);
    j2.verify_chain().unwrap();
}

/// Mid-frame power loss: partial write of a new frame after a good commit.
#[test]
fn rec_partial_frame_after_good_commit_is_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let jpath = dir.path().join("partial.log");
    let mut session = DocumentSession::create_default(&jpath).unwrap();
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: b"good".to_vec(),
        })
        .unwrap();
    let good_seq = session.journal.last_seq();
    let good_len = session.journal.valid_len();
    drop(session);

    // Incomplete header only (power loss mid-write).
    {
        let mut f = FsOpen::new().append(true).open(&jpath).unwrap();
        f.write_all(&[0x51, 0x46]).unwrap(); // partial magic
        f.sync_all().unwrap();
    }

    let j2 = Journal::open_or_create(&jpath).unwrap();
    assert_eq!(j2.last_seq(), good_seq);
    assert_eq!(j2.valid_len(), good_len);
    j2.verify_chain().unwrap();
}
