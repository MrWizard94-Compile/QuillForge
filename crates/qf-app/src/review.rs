//! Word-class Review surface: suggestions accept/reject.
//!
//! Binding law (AI + collab): suggestions are recorded via journal ops only.
//! Accept applies proposed content ops; reject journals status only (text unchanged). Never silent apply.

use qf_ai::{issue_content_permission, AiRequest, AiService, LocalNoteProvider};
use qf_collab::{Suggestion, SuggestionSource, SuggestionStatus};
use qf_core::{Operation, SuggestionId, TextOffset};
use serde::{Deserialize, Serialize};

use crate::{EditorController, EditorError};

/// Summary row for the Review panel (no full op payloads in UI lists).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuggestionRow {
    pub id: SuggestionId,
    pub note: String,
    pub status: SuggestionStatus,
    pub source_label: String,
    pub op_count: usize,
}

impl SuggestionRow {
    pub fn from_suggestion(s: &Suggestion) -> Self {
        let source_label = match &s.source {
            SuggestionSource::Human => "Human".into(),
            SuggestionSource::Ai { provider } => format!("AI ({provider})"),
        };
        Self {
            id: s.id,
            note: s.note.clone().unwrap_or_else(|| "(no note)".into()),
            status: s.status.clone(),
            source_label,
            op_count: s.proposed_ops.len(),
        }
    }
}

/// Review panel chrome state (not durable — selection only).
#[derive(Clone, Debug, Default)]
pub struct ReviewPanel {
    pub visible: bool,
    pub selected: Option<SuggestionId>,
    /// Instruction used when proposing a local suggestion.
    pub instruction: String,
}

impl ReviewPanel {
    pub fn word_class_default() -> Self {
        Self {
            visible: false,
            selected: None,
            instruction: "clarify".into(),
        }
    }
}

impl EditorController {
    /// Decode all suggestions stored in the entity table.
    pub fn load_suggestions(&self) -> Vec<Suggestion> {
        let mut out = Vec::new();
        for payload in self.session.state().entities.suggestions.values() {
            if let Ok(s) = serde_json::from_slice::<Suggestion>(payload) {
                out.push(s);
            }
        }
        out.sort_by_key(|s| s.id.as_uuid());
        out
    }

    pub fn pending_suggestions(&self) -> Vec<Suggestion> {
        self.load_suggestions()
            .into_iter()
            .filter(|s| s.status == SuggestionStatus::Pending)
            .collect()
    }

    pub fn pending_suggestion_rows(&self) -> Vec<SuggestionRow> {
        self.pending_suggestions()
            .iter()
            .map(SuggestionRow::from_suggestion)
            .collect()
    }

    pub fn suggestion_by_id(&self, id: SuggestionId) -> Option<Suggestion> {
        self.load_suggestions().into_iter().find(|s| s.id == id)
    }

    /// Propose a local AI suggestion for the focused block (record only — no content change).
    pub fn propose_local_suggestion(&mut self, instruction: &str) -> Result<SuggestionId, EditorError> {
        let block_id = self.input.selection.block_id.ok_or(EditorError::NoFocus)?;
        let selection_text = self
            .session
            .state()
            .document
            .project_text(block_id)
            .unwrap_or_default();
        let doc = self.session.state().document_id;
        let actor = self.session.actor;
        let now = 1_000_i64;
        let mut ai = AiService::new(LocalNoteProvider::default(), now);
        let tok = issue_content_permission(doc, actor, &selection_text, now, 60_000);
        let sid = SuggestionId::new(self.session.ids.as_mut());
        let suggestion = ai
            .request_suggestion(
                doc,
                actor,
                sid,
                AiRequest {
                    permission: tok,
                    selection_text,
                    instruction: instruction.to_string(),
                    target_block: Some(block_id),
                },
            )
            .map_err(|e| EditorError::Other(e.to_string()))?;
        let ops = AiService::<LocalNoteProvider>::ops_record_suggestion(&suggestion)
            .map_err(|e| EditorError::Other(e.to_string()))?;
        self.session.commit_ops(ops)?;
        // Recording suggestions does not change layout content, but keep surface honest.
        self.reflow();
        Ok(suggestion.id)
    }

    /// Human-authored suggestion: propose appending a review note mark via InsertText.
    pub fn propose_human_suggestion(
        &mut self,
        note: &str,
        insert_text: &str,
    ) -> Result<SuggestionId, EditorError> {
        let block_id = self.input.selection.block_id.ok_or(EditorError::NoFocus)?;
        let at = TextOffset(
            self.session
                .state()
                .document
                .project_text(block_id)
                .map(|t| t.chars().count() as u32)
                .unwrap_or(0),
        );
        let id = SuggestionId::new(self.session.ids.as_mut());
        let suggestion = Suggestion {
            id,
            author: self.session.actor,
            note: Some(note.to_string()),
            proposed_ops: vec![Operation::InsertText {
                block_id,
                at,
                text: insert_text.to_string(),
            }],
            status: SuggestionStatus::Pending,
            source: SuggestionSource::Human,
        };
        let ops = AiService::<LocalNoteProvider>::ops_record_suggestion(&suggestion)
            .map_err(|e| EditorError::Other(e.to_string()))?;
        self.session.commit_ops(ops)?;
        self.reflow();
        Ok(id)
    }

    /// Accept: apply proposed ops + mark accepted (journal path only).
    pub fn accept_suggestion(&mut self, id: SuggestionId) -> Result<(), EditorError> {
        let suggestion = self
            .suggestion_by_id(id)
            .ok_or_else(|| EditorError::Other("suggestion not found".into()))?;
        if suggestion.status != SuggestionStatus::Pending {
            return Err(EditorError::Other("suggestion not pending".into()));
        }
        let ops = AiService::<LocalNoteProvider>::ops_accept_suggestion(&suggestion)
            .map_err(|e| EditorError::Other(e.to_string()))?;
        self.session.commit_ops(ops)?;
        self.resync_selection_from_canonical();
        self.reflow();
        Ok(())
    }

    /// Reject: status only; document text unchanged.
    pub fn reject_suggestion(&mut self, id: SuggestionId) -> Result<(), EditorError> {
        let suggestion = self
            .suggestion_by_id(id)
            .ok_or_else(|| EditorError::Other("suggestion not found".into()))?;
        if suggestion.status != SuggestionStatus::Pending {
            return Err(EditorError::Other("suggestion not pending".into()));
        }
        let ops = AiService::<LocalNoteProvider>::ops_reject_suggestion(&suggestion)
            .map_err(|e| EditorError::Other(e.to_string()))?;
        self.session.commit_ops(ops)?;
        self.reflow();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::EditorController;
    use qf_collab::SuggestionStatus;
    use tempfile::tempdir;

    #[test]
    fn propose_accept_changes_text_reject_does_not() {
        let dir = tempdir().unwrap();
        let mut ed = EditorController::create_default(&dir.path().join("r.journal")).unwrap();
        ed.ensure_editable_paragraph().unwrap();
        ed.set_focused_plain("Hello review").unwrap();
        let before = ed.focused_text().unwrap();

        let id = ed.propose_local_suggestion("annotate").unwrap();
        assert_eq!(ed.pending_suggestions().len(), 1);
        assert_eq!(ed.focused_text().unwrap(), before);

        ed.accept_suggestion(id).unwrap();
        let after = ed.focused_text().unwrap();
        assert_ne!(after, before);
        assert!(ed.pending_suggestions().is_empty());
    }

    #[test]
    fn reject_keeps_text() {
        let dir = tempdir().unwrap();
        let mut ed = EditorController::create_default(&dir.path().join("r2.journal")).unwrap();
        ed.ensure_editable_paragraph().unwrap();
        ed.set_focused_plain("Stable").unwrap();
        let before = ed.focused_text().unwrap();
        let id = ed.propose_human_suggestion("typo?", " [comment]").unwrap();
        ed.reject_suggestion(id).unwrap();
        assert_eq!(ed.focused_text().unwrap(), before);
        let s = ed.suggestion_by_id(id).unwrap();
        assert_eq!(s.status, SuggestionStatus::Rejected);
    }

    #[test]
    fn accept_non_pending_fails() {
        let dir = tempdir().unwrap();
        let mut ed = EditorController::create_default(&dir.path().join("r3.journal")).unwrap();
        ed.ensure_editable_paragraph().unwrap();
        ed.set_focused_plain("x").unwrap();
        let id = ed.propose_local_suggestion("a").unwrap();
        ed.reject_suggestion(id).unwrap();
        assert!(ed.accept_suggestion(id).is_err());
    }
}
