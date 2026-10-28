//! The packaged CLI must direct users to the single canonical governance source.

use std::process::Command;

#[test]
fn help_names_the_canonical_constitution_without_a_local_pack_loader() {
    let output = Command::new(env!("CARGO_BIN_EXE_quillforge"))
        .arg("help")
        .output()
        .expect("run the packaged CLI help command");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let text = String::from_utf8(output.stdout).expect("UTF-8 CLI output");
    let law = text
        .lines()
        .find(|line| line.starts_with("Law:"))
        .expect("one visible governance route");
    assert_eq!(
        law,
        "Law: WPAI Constitution — https://github.com/MrWizard94-Compile/Agents-Constitution"
    );
    assert_eq!(
        text.lines().filter(|line| line.starts_with("Law:")).count(),
        1
    );
}
