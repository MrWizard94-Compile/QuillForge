//! AI may **propose suggestions only**. Never silently mutates document content.
//!
//! # Invariants
//!
//! 1. Content access requires a valid [`ContentPermissionToken`].
//! 2. [`AiService`] has no `apply` / `commit` method — accept is a separate command path.
//! 3. Audit events never store full document body.
//! 4. Selection size is bounded (abuse protection).

use qf_collab::{Suggestion, SuggestionSource, SuggestionStatus};
use qf_core::{
    ActorId, BlockId, DocumentId, Hash256, Operation, ParentRef, SuggestionId, TextOffset,
};
use qf_security::{AuditEvent, AuditLog, ContentPermissionToken};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

/// Maximum characters of selection text an AI request may include.
pub const MAX_SELECTION_CHARS: usize = 32_768;
/// Soft rate limit: max successful proposals per service instance (reset externally).
pub const DEFAULT_MAX_PROPOSALS_PER_SESSION: u64 = 100;

#[derive(Clone, Debug)]
pub struct AiRequest {
    pub permission: ContentPermissionToken,
    pub selection_text: String,
    pub instruction: String,
    /// Optional block target for providers that emit structural ops.
    pub target_block: Option<BlockId>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AiProposal {
    pub note: String,
    pub proposed_ops: Vec<Operation>,
    pub provider_id: String,
}

pub trait AiProvider: Send + Sync {
    fn provider_id(&self) -> &str;
    fn propose(&self, req: &AiRequest) -> Result<AiProposal, AiError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiError {
    PermissionDenied,
    ScopeMismatch,
    SelectionTooLarge,
    RateLimited,
    Provider(String),
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiError::PermissionDenied => write!(f, "permission denied"),
            AiError::ScopeMismatch => write!(f, "token scope mismatch"),
            AiError::SelectionTooLarge => write!(f, "selection too large"),
            AiError::RateLimited => write!(f, "rate limited"),
            AiError::Provider(s) => write!(f, "provider: {s}"),
        }
    }
}

impl std::error::Error for AiError {}

pub struct AiServiceConfig {
    pub max_selection_chars: usize,
    pub max_proposals: u64,
}

impl Default for AiServiceConfig {
    fn default() -> Self {
        Self {
            max_selection_chars: MAX_SELECTION_CHARS,
            max_proposals: DEFAULT_MAX_PROPOSALS_PER_SESSION,
        }
    }
}

pub struct AiService<P: AiProvider> {
    provider: P,
    now_unix_ms: i64,
    config: AiServiceConfig,
    proposals_made: AtomicU64,
    audit: AuditLog,
}

impl<P: AiProvider> AiService<P> {
    pub fn new(provider: P, now_unix_ms: i64) -> Self {
        Self {
            provider,
            now_unix_ms,
            config: AiServiceConfig::default(),
            proposals_made: AtomicU64::new(0),
            audit: AuditLog::default(),
        }
    }

    pub fn with_config(mut self, config: AiServiceConfig) -> Self {
        self.config = config;
        self
    }

    pub fn set_now(&mut self, now_unix_ms: i64) {
        self.now_unix_ms = now_unix_ms;
    }

    pub fn audit_events(&self) -> &[AuditEvent] {
        self.audit.events()
    }

    pub fn proposals_made(&self) -> u64 {
        self.proposals_made.load(Ordering::Relaxed)
    }

    /// Validate token + request; call provider; return **pending** suggestion only.
    ///
    /// Does **not** apply operations to any document.
    pub fn request_suggestion(
        &mut self,
        document_id: DocumentId,
        author: ActorId,
        suggestion_id: SuggestionId,
        req: AiRequest,
    ) -> Result<Suggestion, AiError> {
        self.validate_request(document_id, author, &req)?;

        if self.proposals_made.load(Ordering::Relaxed) >= self.config.max_proposals {
            return Err(AiError::RateLimited);
        }

        let proposal = self.provider.propose(&req).map_err(|e| match e {
            AiError::Provider(s) => AiError::Provider(s),
            other => other,
        })?;

        self.proposals_made.fetch_add(1, Ordering::Relaxed);
        self.audit.append(AuditEvent {
            actor: author,
            kind: "ai.suggestion.proposed".into(),
            document_id: Some(document_id),
            op_seq: None,
            detail: format!(
                "provider={} instruction_len={} selection_len={} ops={}",
                proposal.provider_id,
                req.instruction.len(),
                req.selection_text.chars().count(),
                proposal.proposed_ops.len()
            ),
        });

        Ok(Suggestion {
            id: suggestion_id,
            author,
            note: Some(proposal.note),
            proposed_ops: proposal.proposed_ops,
            status: SuggestionStatus::Pending,
            source: SuggestionSource::Ai {
                provider: proposal.provider_id,
            },
        })
    }

    fn validate_request(
        &self,
        document_id: DocumentId,
        author: ActorId,
        req: &AiRequest,
    ) -> Result<(), AiError> {
        if !req.permission.is_valid_at(self.now_unix_ms) {
            return Err(AiError::PermissionDenied);
        }
        if req.permission.document_id != document_id || req.permission.actor_id != author {
            return Err(AiError::ScopeMismatch);
        }
        if req.selection_text.chars().count() > self.config.max_selection_chars {
            return Err(AiError::SelectionTooLarge);
        }
        Ok(())
    }

    /// Serialize suggestion for kernel entity table (UpsertSuggestion payload).
    pub fn encode_suggestion_payload(suggestion: &Suggestion) -> Result<Vec<u8>, AiError> {
        serde_json::to_vec(suggestion).map_err(|e| AiError::Provider(e.to_string()))
    }

    /// Build journal ops that **record** a pending suggestion (not content apply).
    pub fn ops_record_suggestion(suggestion: &Suggestion) -> Result<Vec<Operation>, AiError> {
        let payload = Self::encode_suggestion_payload(suggestion)?;
        Ok(vec![Operation::UpsertSuggestion {
            suggestion_id: suggestion.id,
            payload,
        }])
    }

    /// Build journal ops for **accept**: apply proposed content ops + mark accepted.
    ///
    /// Caller must run these through dry-run + journal (DocumentSession).
    pub fn ops_accept_suggestion(suggestion: &Suggestion) -> Result<Vec<Operation>, AiError> {
        if suggestion.status != SuggestionStatus::Pending {
            return Err(AiError::Provider("suggestion not pending".into()));
        }
        let mut ops = suggestion.proposed_ops.clone();
        let mut accepted = suggestion.clone();
        accepted.status = SuggestionStatus::Accepted;
        let payload = Self::encode_suggestion_payload(&accepted)?;
        ops.push(Operation::UpsertSuggestion {
            suggestion_id: suggestion.id,
            payload,
        });
        Ok(ops)
    }

    /// Build journal ops for **reject** (status only; no content change).
    pub fn ops_reject_suggestion(suggestion: &Suggestion) -> Result<Vec<Operation>, AiError> {
        let mut rejected = suggestion.clone();
        rejected.status = SuggestionStatus::Rejected;
        rejected.proposed_ops.clear();
        let payload = Self::encode_suggestion_payload(&rejected)?;
        Ok(vec![Operation::UpsertSuggestion {
            suggestion_id: suggestion.id,
            payload,
        }])
    }
}

/// Local offline provider: proposes inserting instruction-derived note text (for tests/demo).
pub struct LocalNoteProvider {
    pub id: String,
}

impl Default for LocalNoteProvider {
    fn default() -> Self {
        Self {
            id: "local-note".into(),
        }
    }
}

impl AiProvider for LocalNoteProvider {
    fn provider_id(&self) -> &str {
        &self.id
    }

    fn propose(&self, req: &AiRequest) -> Result<AiProposal, AiError> {
        let note = format!("AI note: {}", req.instruction);
        let mut proposed_ops = Vec::new();
        if let Some(block_id) = req.target_block {
            // Append a short annotation as text insert at end of selection length.
            let at = TextOffset(req.selection_text.chars().count() as u32);
            proposed_ops.push(Operation::InsertText {
                block_id,
                at,
                text: format!(
                    " [{}]",
                    req.instruction.chars().take(40).collect::<String>()
                ),
            });
        }
        Ok(AiProposal {
            note,
            proposed_ops,
            provider_id: self.id.clone(),
        })
    }
}

/// Issue a short-lived content permission token (UI grant).
pub fn issue_content_permission(
    document_id: DocumentId,
    actor_id: ActorId,
    selection_text: &str,
    now_unix_ms: i64,
    ttl_ms: i64,
) -> ContentPermissionToken {
    ContentPermissionToken {
        document_id,
        actor_id,
        scope_hash: Hash256::sha256(selection_text.as_bytes()),
        expires_unix_ms: now_unix_ms.saturating_add(ttl_ms),
        grant_id: {
            let h =
                Hash256::sha256(format!("{document_id:?}:{actor_id:?}:{now_unix_ms}").as_bytes());
            let mut g = [0u8; 16];
            g.copy_from_slice(&h.0[..16]);
            g
        },
    }
}

/// Structural helper: empty paragraph insert op (for accept tests).
pub fn op_insert_empty_paragraph(block_id: BlockId) -> Operation {
    Operation::InsertBlock {
        block_id,
        parent: ParentRef::DocumentRoot,
        index: 0,
        body: qf_core::BlockBodyBytes {
            type_name: "paragraph".into(),
            payload: vec![],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::SeqIdGen;

    #[test]
    fn rejects_expired_permission() {
        let mut gen = SeqIdGen::default();
        let mut svc = AiService::new(LocalNoteProvider::default(), 1000);
        let doc = DocumentId::new(&mut gen);
        let actor = ActorId::new(&mut gen);
        let req = AiRequest {
            permission: ContentPermissionToken {
                document_id: doc,
                actor_id: actor,
                scope_hash: Hash256::ZERO,
                expires_unix_ms: 500,
                grant_id: [0u8; 16],
            },
            selection_text: "hi".into(),
            instruction: "rewrite".into(),
            target_block: None,
        };
        let err = svc
            .request_suggestion(doc, actor, SuggestionId::new(&mut gen), req)
            .unwrap_err();
        assert_eq!(err, AiError::PermissionDenied);
    }

    #[test]
    fn rejects_scope_mismatch() {
        let mut gen = SeqIdGen::default();
        let mut svc = AiService::new(LocalNoteProvider::default(), 100);
        let doc = DocumentId::new(&mut gen);
        let actor = ActorId::new(&mut gen);
        let other = ActorId::new(&mut gen);
        let tok = issue_content_permission(doc, actor, "x", 100, 1000);
        let req = AiRequest {
            permission: tok,
            selection_text: "x".into(),
            instruction: "y".into(),
            target_block: None,
        };
        let err = svc
            .request_suggestion(doc, other, SuggestionId::new(&mut gen), req)
            .unwrap_err();
        assert_eq!(err, AiError::ScopeMismatch);
    }

    #[test]
    fn rejects_oversized_selection() {
        let mut gen = SeqIdGen::default();
        let mut svc =
            AiService::new(LocalNoteProvider::default(), 100).with_config(AiServiceConfig {
                max_selection_chars: 5,
                max_proposals: 10,
            });
        let doc = DocumentId::new(&mut gen);
        let actor = ActorId::new(&mut gen);
        let tok = issue_content_permission(doc, actor, "123456", 100, 1000);
        let req = AiRequest {
            permission: tok,
            selection_text: "123456".into(),
            instruction: "y".into(),
            target_block: None,
        };
        let err = svc
            .request_suggestion(doc, actor, SuggestionId::new(&mut gen), req)
            .unwrap_err();
        assert_eq!(err, AiError::SelectionTooLarge);
    }

    #[test]
    fn rate_limit_proposals() {
        let mut gen = SeqIdGen::default();
        let mut svc =
            AiService::new(LocalNoteProvider::default(), 100).with_config(AiServiceConfig {
                max_selection_chars: 100,
                max_proposals: 1,
            });
        let doc = DocumentId::new(&mut gen);
        let actor = ActorId::new(&mut gen);
        let tok = issue_content_permission(doc, actor, "hi", 100, 1000);
        let req = AiRequest {
            permission: tok.clone(),
            selection_text: "hi".into(),
            instruction: "a".into(),
            target_block: None,
        };
        svc.request_suggestion(doc, actor, SuggestionId::new(&mut gen), req.clone())
            .unwrap();
        let err = svc
            .request_suggestion(doc, actor, SuggestionId::new(&mut gen), req)
            .unwrap_err();
        assert_eq!(err, AiError::RateLimited);
    }

    #[test]
    fn audit_does_not_store_selection_body() {
        let mut gen = SeqIdGen::default();
        let mut svc = AiService::new(LocalNoteProvider::default(), 100);
        let doc = DocumentId::new(&mut gen);
        let actor = ActorId::new(&mut gen);
        let secret = "TOP_SECRET_DOCUMENT_BODY_XYZ";
        let tok = issue_content_permission(doc, actor, secret, 100, 1000);
        let req = AiRequest {
            permission: tok,
            selection_text: secret.into(),
            instruction: "summarize".into(),
            target_block: None,
        };
        svc.request_suggestion(doc, actor, SuggestionId::new(&mut gen), req)
            .unwrap();
        for ev in svc.audit_events() {
            assert!(
                !ev.detail.contains(secret),
                "audit leaked selection: {}",
                ev.detail
            );
        }
    }

    #[test]
    fn accept_ops_include_proposed_then_status() {
        let mut gen = SeqIdGen::default();
        let bid = BlockId::new(&mut gen);
        let s = Suggestion {
            id: SuggestionId::new(&mut gen),
            author: ActorId::new(&mut gen),
            note: Some("n".into()),
            proposed_ops: vec![Operation::InsertText {
                block_id: bid,
                at: TextOffset(0),
                text: "!".into(),
            }],
            status: SuggestionStatus::Pending,
            source: SuggestionSource::Ai {
                provider: "t".into(),
            },
        };
        let ops = AiService::<LocalNoteProvider>::ops_accept_suggestion(&s).unwrap();
        assert!(ops.len() >= 2);
        assert!(matches!(ops[0], Operation::InsertText { .. }));
        assert!(matches!(
            ops.last().unwrap(),
            Operation::UpsertSuggestion { .. }
        ));
    }
}
