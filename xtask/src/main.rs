//! Quillforge workspace automation (`cargo xtask <cmd>`).
//!
//! Supported commands:
//! - `check` — fmt (check) + clippy + build + test
//! - `help`  — print usage

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".into());

    match cmd.as_str() {
        "check" => run_check(),
        "help" | "-h" | "--help" => {
            print_help();
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("unknown xtask command: {other}");
            print_help();
            ExitCode::FAILURE
        }
    }
}

fn print_help() {
    eprintln!(
        "Quillforge xtask\n\n\
         Usage:\n\
         \tcargo xtask check   Run fmt check, clippy, build, test\n\
         \tcargo xtask help    Show this help\n"
    );
}

fn run_check() -> ExitCode {
    let steps: &[(&str, &[&str])] = &[
        ("fmt", &["fmt", "--all", "--", "--check"]),
        (
            "clippy",
            &[
                "clippy",
                "--workspace",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
        ),
        ("build", &["build", "--workspace"]),
        ("test", &["test", "--workspace"]),
    ];

    for (name, args) in steps {
        eprintln!("==> cargo {}", args.join(" "));
        let status = Command::new("cargo").args(*args).status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => {
                eprintln!("xtask check failed at step: {name} (exit {s})");
                return ExitCode::FAILURE;
            }
            Err(e) => {
                eprintln!("xtask check failed to spawn cargo for {name}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    eprintln!("xtask check: ok");
    ExitCode::SUCCESS
}
