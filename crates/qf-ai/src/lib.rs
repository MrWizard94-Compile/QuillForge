//! AI may propose suggestions only. Never silently mutates content.
//! Content requires an explicit ContentPermissionToken.

use qf_collab::{Suggestion, SuggestionSource, SuggestionStatus};
use qf_core::{ActorId, Operation, SuggestionId};
use qf_security::ContentPermissionToken;

#[derive(Clone, Debug)]
pub struct AiRequest {
    pub permission: ContentPermissionToken,
    pub selection_text: String,
    pub instruction: String,
}

#[derive(Clone, Debug)]
pub struct AiProposal {
    pub note: String,
    pub proposed_ops: Vec<Operation>,
}

pub trait AiProvider: Send + Sync {
    fn propose(&self, req: &AiRequest) -> Result<AiProposal, AiError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiError {
    PermissionDenied,
    Provider(String),
}

pub struct AiService<P: AiProvider> {
    provider: P,
    now_unix_ms: i64,
}

impl<P: AiProvider> AiService<P> {
    pub fn new(provider: P, now_unix_ms: i64) -> Self {
        Self {
            provider,
            now_unix_ms,
        }
    }

    /// Create a pending Suggestion. Does NOT apply ops to the document.
    pub fn request_suggestion(
        &self,
        author: ActorId,
        suggestion_id: SuggestionId,
        req: AiRequest,
    ) -> Result<Suggestion, AiError> {
        if !req.permission.is_valid_at(self.now_unix_ms) {
            return Err(AiError::PermissionDenied);
        }
        let proposal = self.provider.propose(&req)?;
        Ok(Suggestion {
            id: suggestion_id,
            author,
            note: Some(proposal.note),
            proposed_ops: proposal.proposed_ops,
            status: SuggestionStatus::Pending,
            source: SuggestionSource::Ai {
                provider: "default".into(),
            },
        })
    }
}

/// Explicit non-API: there is no `apply_silently` method on AiService.
#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::{DocumentId, Hash256, SeqIdGen};

    struct EchoProvider;
    impl AiProvider for EchoProvider {
        fn propose(&self, req: &AiRequest) -> Result<AiProposal, AiError> {
            Ok(AiProposal {
                note: format!("re: {}", req.instruction),
                proposed_ops: vec![],
            })
        }
    }

    #[test]
    fn rejects_expired_permission() {
        let mut gen = SeqIdGen::default();
        let svc = AiService::new(EchoProvider, 1000);
        let req = AiRequest {
            permission: ContentPermissionToken {
                document_id: DocumentId::new(&mut gen),
                actor_id: ActorId::new(&mut gen),
                scope_hash: Hash256::ZERO,
                expires_unix_ms: 500,
                grant_id: [0u8; 16],
            },
            selection_text: "hi".into(),
            instruction: "rewrite".into(),
        };
        let err = svc
            .request_suggestion(ActorId::new(&mut gen), SuggestionId::new(&mut gen), req)
            .unwrap_err();
        assert_eq!(err, AiError::PermissionDenied);
    }
}
