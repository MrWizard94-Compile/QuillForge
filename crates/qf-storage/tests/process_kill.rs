//! OS process-kill durability (REC process harness).
//!
//! Spawns `qf-kill-harness`, which commits then aborts. Parent reopens journal
//! and asserts the committed operation is present and chain-valid.

use qf_journal::Journal;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn process_abort_after_commit_preserves_journal() {
    let dir = tempfile::tempdir().unwrap();
    let journal = dir.path().join("kill-harness.log");
    let marker = dir.path().join("committed.ok");

    let bin = env!("CARGO_BIN_EXE_qf-kill-harness");
    let status = Command::new(bin)
        .arg(&journal)
        .arg(&marker)
        .status()
        .expect("spawn kill harness");

    // abort() yields non-success on all major platforms.
    assert!(
        !status.success(),
        "harness should abort after commit, got {status}"
    );
    assert!(
        marker.exists(),
        "marker must exist — proves commit returned Ok before abort"
    );
    assert!(journal.exists(), "journal file must exist after abort");

    let j = Journal::open_or_create(&journal).expect("reopen journal after kill");
    assert!(
        j.last_seq().0 >= 1,
        "expected at least one committed op after process kill"
    );
    j.verify_chain().expect("hash chain after process kill");
}

#[test]
fn process_kill_harness_binary_is_built() {
    let bin = PathBuf::from(env!("CARGO_BIN_EXE_qf-kill-harness"));
    assert!(bin.exists(), "missing binary {}", bin.display());
}
