//! Plugin proposals only enter canonical state via DocumentSession (command path).

use qf_core::{Capability, PluginId};
use qf_plugin::{parse_manifest_toml_lite, PluginHost, ProposedOp, CAP_DOCUMENT_PROPOSE_OPS};
use qf_storage::DocumentSession;

#[test]
fn proposal_materialized_and_committed_through_session() {
    let dir = tempfile::tempdir().unwrap();
    let mut host = PluginHost::new();
    let manifest = parse_manifest_toml_lite(
        r#"
id = "com.example.writer"
name = "Writer"
required = ["document.propose_ops"]
"#,
    )
    .unwrap();
    let pid = host
        .load(manifest, [Capability(CAP_DOCUMENT_PROPOSE_OPS.into())])
        .unwrap();

    let proposal_id = host
        .propose_operations(
            &pid,
            vec![ProposedOp {
                kind: "noop".into(),
                payload: vec![],
            }],
        )
        .unwrap();
    let proposal = host.take_proposal(&proposal_id).unwrap();
    let ops = PluginHost::materialize_proposal_ops(&proposal).unwrap();
    assert_eq!(ops.len(), 1);

    let mut session = DocumentSession::create_default(&dir.path().join("p.log")).unwrap();
    let before = session.state().op_seq;
    session.commit_ops(ops).unwrap();
    assert!(session.state().op_seq.0 > before.0);

    // Host never had journal access
    assert!(!qf_plugin::journal_access_allowed());
    assert!(!qf_plugin::raw_filesystem_access_allowed());
    let _ = PluginId("x".into());
}
