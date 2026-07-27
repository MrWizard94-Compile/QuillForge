//! Security: encryption helpers, ACL, audit, content permission tokens.

mod crypto;

pub use crypto::*;

use qf_core::{ActorId, Capability, DocumentId, Hash256, Role};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContentPermissionToken {
    pub document_id: DocumentId,
    pub actor_id: ActorId,
    pub scope_hash: Hash256,
    pub expires_unix_ms: i64,
    /// Opaque grant id; presence of token proves UI grant.
    pub grant_id: [u8; 16],
}

impl ContentPermissionToken {
    pub fn is_valid_at(&self, now_unix_ms: i64) -> bool {
        now_unix_ms <= self.expires_unix_ms
    }

    /// Reject tokens that claim a different document or actor than the active context.
    pub fn matches_scope(&self, document_id: DocumentId, actor_id: ActorId) -> bool {
        self.document_id == document_id && self.actor_id == actor_id
    }
}

#[derive(Clone, Debug)]
pub struct AclEnforcer {
    pub role: Role,
    pub capabilities: HashSet<String>,
}

impl AclEnforcer {
    pub fn can_edit(&self) -> bool {
        matches!(self.role, Role::Owner | Role::Editor)
    }

    pub fn can_comment(&self) -> bool {
        matches!(
            self.role,
            Role::Owner | Role::Editor | Role::Reviewer | Role::Commenter
        )
    }

    pub fn has_capability(&self, cap: &Capability) -> bool {
        self.capabilities.contains(&cap.0)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditEvent {
    pub actor: ActorId,
    pub kind: String,
    pub document_id: Option<DocumentId>,
    pub op_seq: Option<u64>,
    /// Must not contain document body by default.
    pub detail: String,
}

#[derive(Default)]
pub struct AuditLog {
    events: Vec<AuditEvent>,
}

impl AuditLog {
    pub fn append(&mut self, ev: AuditEvent) {
        // Redaction guard: reject detail that looks like large body dumps.
        if ev.detail.len() > 512 {
            return;
        }
        self.events.push(ev);
    }

    pub fn events(&self) -> &[AuditEvent] {
        &self.events
    }
}

/// Legacy helper retained for simple hashing; prefer [`derive_key`] for package crypto.
pub fn derive_key_material(passphrase: &str, salt: &[u8]) -> Hash256 {
    let mut data = Vec::new();
    data.extend_from_slice(salt);
    data.extend_from_slice(passphrase.as_bytes());
    Hash256::sha256(&data)
}

pub fn passphrase_nonempty(passphrase: &str) -> bool {
    !passphrase.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::SeqIdGen;

    #[test]
    fn audit_rejects_oversized_detail() {
        let mut log = AuditLog::default();
        let mut gen = SeqIdGen::default();
        log.append(AuditEvent {
            actor: ActorId::new(&mut gen),
            kind: "x".into(),
            document_id: None,
            op_seq: None,
            detail: "a".repeat(600),
        });
        assert!(log.events().is_empty());
    }

    #[test]
    fn token_scope_match() {
        let mut gen = SeqIdGen::default();
        let doc = DocumentId::new(&mut gen);
        let actor = ActorId::new(&mut gen);
        let tok = ContentPermissionToken {
            document_id: doc,
            actor_id: actor,
            scope_hash: Hash256::ZERO,
            expires_unix_ms: 1,
            grant_id: [1u8; 16],
        };
        assert!(tok.matches_scope(doc, actor));
        assert!(!tok.matches_scope(doc, ActorId::new(&mut gen)));
    }
}
