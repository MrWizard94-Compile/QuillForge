//! Quillforge workspace automation (`cargo xtask <cmd>`).
//!
//! Supported commands:
//! - `check` — fmt (check) + clippy + build + test + perf budgets
//! - `perf`  — performance budget tests only (`budget_` filter)
//! - `help`  — print usage

use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".into());

    match cmd.as_str() {
        "check" => run_check(),
        "perf" => run_perf(),
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
         \tcargo xtask check   Run fmt check, clippy, build, test, perf budgets\n\
         \tcargo xtask perf    Run CI performance budget tests only\n\
         \tcargo xtask help    Show this help\n"
    );
}

fn run_cargo(name: &str, args: &[&str]) -> ExitCode {
    eprintln!("==> cargo {}", args.join(" "));
    match Command::new("cargo").args(args).status() {
        Ok(s) if s.success() => ExitCode::SUCCESS,
        Ok(s) => {
            eprintln!("xtask failed at step: {name} (exit {s})");
            ExitCode::FAILURE
        }
        Err(e) => {
            eprintln!("xtask failed to spawn cargo for {name}: {e}");
            ExitCode::FAILURE
        }
    }
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
        (
            "perf",
            &["test", "--workspace", "budget_", "--", "--nocapture"],
        ),
    ];

    for (name, args) in steps {
        if run_cargo(name, args) != ExitCode::SUCCESS {
            return ExitCode::FAILURE;
        }
    }

    eprintln!("xtask check: ok");
    ExitCode::SUCCESS
}

fn run_perf() -> ExitCode {
    let code = run_cargo(
        "perf",
        &["test", "--workspace", "budget_", "--", "--nocapture"],
    );
    if code == ExitCode::SUCCESS {
        eprintln!("xtask perf: ok");
    }
    code
}
