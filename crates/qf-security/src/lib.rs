//! Security: encryption helpers, ACL, audit, content permission tokens.

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

/// Passphrase key derivation placeholder — production uses Argon2id.
pub fn derive_key_material(passphrase: &str, salt: &[u8]) -> Hash256 {
    let mut data = Vec::new();
    data.extend_from_slice(salt);
    data.extend_from_slice(passphrase.as_bytes());
    Hash256::sha256(&data)
}
