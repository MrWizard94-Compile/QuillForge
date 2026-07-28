//! Process-kill durability harness.
//!
//! Usage: `qf-kill-harness <journal_path> <marker_path>`
//!
//! 1. Create session, commit one block, rely on journal fsync contract.
//! 2. Write `marker_path` only after commit returns Ok.
//! 3. Abort the process without Drop cleanup (simulates power loss / kill -9).
//!
//! Parent tests reopen the journal and assert the committed op survived.

use qf_core::{Command, ParentRef};
use qf_storage::DocumentSession;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process;

fn main() {
    let mut args = env::args().skip(1);
    let journal = PathBuf::from(args.next().expect("journal path"));
    let marker = PathBuf::from(args.next().expect("marker path"));

    let mut session = DocumentSession::create_default(&journal).expect("create session");
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: b"kill-harness-payload".to_vec(),
        })
        .expect("commit");

    // Commit path fsync'd before Ok — marker proves we passed that barrier.
    fs::write(&marker, b"committed\n").expect("write marker");

    // Hard abort: no Drop, no graceful shutdown.
    process::abort();
}
