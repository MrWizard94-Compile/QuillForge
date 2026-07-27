//! AI suggestions apply only via explicit accept → DocumentSession commit.

use qf_ai::{issue_content_permission, AiRequest, AiService, LocalNoteProvider};
use qf_core::{Command, ParentRef, SeqIdGen, SuggestionId, TextOffset};
use qf_storage::DocumentSession;

#[test]
fn propose_record_accept_mutates_only_on_accept() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = DocumentSession::create_default(&dir.path().join("ai.log")).unwrap();

    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: b"Hello".to_vec(),
        })
        .unwrap();
    let block_id = *session.state().document.blocks.keys().next().unwrap();
    let text_before = session.state().document.project_text(block_id).unwrap();

    let mut gen = SeqIdGen::default();
    let doc = session.state().document_id;
    let actor = session.actor;
    let mut ai = AiService::new(LocalNoteProvider::default(), 1_000);
    let tok = issue_content_permission(doc, actor, &text_before, 1_000, 60_000);
    let suggestion = ai
        .request_suggestion(
            doc,
            actor,
            SuggestionId::new(&mut gen),
            AiRequest {
                permission: tok,
                selection_text: text_before.clone(),
                instruction: "annotate".into(),
                target_block: Some(block_id),
            },
        )
        .unwrap();

    // Record pending suggestion — content unchanged.
    let record_ops = AiService::<LocalNoteProvider>::ops_record_suggestion(&suggestion).unwrap();
    session.commit_ops(record_ops).unwrap();
    assert_eq!(
        session.state().document.project_text(block_id).unwrap(),
        text_before
    );
    assert!(session
        .state()
        .entities
        .suggestions
        .contains_key(&suggestion.id));

    // Accept applies proposed ops.
    let accept_ops = AiService::<LocalNoteProvider>::ops_accept_suggestion(&suggestion).unwrap();
    session.commit_ops(accept_ops).unwrap();
    let after = session.state().document.project_text(block_id).unwrap();
    assert_ne!(after, text_before);
    assert!(after.contains("annotate") || after.len() > text_before.len());
}

#[test]
fn reject_does_not_change_text() {
    let dir = tempfile::tempdir().unwrap();
    let mut session = DocumentSession::create_default(&dir.path().join("ai2.log")).unwrap();
    session
        .commit_command(Command::InsertBlock {
            parent: ParentRef::DocumentRoot,
            index: 0,
            body_type: "paragraph".into(),
            body_payload: b"Stay".to_vec(),
        })
        .unwrap();
    let block_id = *session.state().document.blocks.keys().next().unwrap();
    let before = session.state().document.project_text(block_id).unwrap();

    let mut gen = SeqIdGen::default();
    let mut ai = AiService::new(LocalNoteProvider::default(), 50);
    let doc = session.state().document_id;
    let actor = session.actor;
    let tok = issue_content_permission(doc, actor, &before, 50, 1000);
    let suggestion = ai
        .request_suggestion(
            doc,
            actor,
            SuggestionId::new(&mut gen),
            AiRequest {
                permission: tok,
                selection_text: before.clone(),
                instruction: "nope".into(),
                target_block: Some(block_id),
            },
        )
        .unwrap();
    session
        .commit_ops(AiService::<LocalNoteProvider>::ops_record_suggestion(&suggestion).unwrap())
        .unwrap();
    session
        .commit_ops(AiService::<LocalNoteProvider>::ops_reject_suggestion(&suggestion).unwrap())
        .unwrap();
    assert_eq!(
        session.state().document.project_text(block_id).unwrap(),
        before
    );
    let _ = TextOffset(0);
}
