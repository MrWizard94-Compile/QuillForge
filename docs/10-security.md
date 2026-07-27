# Security Specification

**Document ID:** QF-SEC-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Threat model (summary)

| Threat | Mitigation |
|--------|------------|
| Disk theft | Encryption at rest |
| Network eavesdropping | TLS for sync |
| Tampered package | Hash seal + optional signatures |
| Malicious plugin | WASM sandbox + capabilities |
| Silent AI exfiltration | Explicit content permission |
| Privilege escalation in collab | Role + region ACLs |
| Journal tampering | Hash chain + seal |

Assume local OS user is trusted for unlocked session. Protect data at rest and against network/plugin adversaries.

---

## 2. Encryption at rest

### 2.1 Modes

| Mode | Description |
|------|-------------|
| `None` | Default for local casual use |
| `Passphrase` | Argon2id → key; AEAD package |
| `Keyfile` | Key from OS keystore / file |
| `EnterpriseKMS` | Unwrap via external KMS hook (interface only in v1) |

### 2.2 Algorithm (v1 decision)

- **AEAD:** XChaCha20-Poly1305  
- **KDF:** Argon2id (memory-hard parameters documented in package header)  
- **Header:** unencrypted salt, kdf params, cipher id; ciphertext for remaining package body  

```rust
pub struct EncryptionHeader {
    pub cipher: CipherId, // XChaCha20Poly1305 = 1
    pub kdf: KdfId,       // Argon2id = 1
    pub salt: [u8; 16],
    pub kdf_params: Argon2Params,
    pub nonce: [u8; 24],
}
```

**Integrity:** AEAD tag covers ciphertext; MANIFEST hashes still used inside plaintext for structure.

### 2.3 What is encrypted

Entire package body after header when mode ≠ None. LKG uses same mode.

---

## 3. Encryption in transit

- Sync transport: **TLS 1.3** minimum.  
- Certificate pinning optional for enterprise.  
- Mutual TLS optional for enterprise gateways.  
- Presence and content streams both over authenticated TLS sessions.  
- No custom crypto over plain TCP.

---

## 4. Signing

```rust
pub struct PackageSignature {
    pub alg: SigAlg, // Ed25519
    pub public_key_id: KeyId,
    pub signature: Vec<u8>, // over seal hash
}
```

| Use | Rule |
|-----|------|
| Enterprise distribution | Packages may require signature to open |
| Plugins | Ed25519 signature over wasm+manifest |
| Journal | Hash chain is primary; external signature optional for export bundles |

---

## 5. Permissions & roles

```rust
pub enum Role {
    Owner,
    Editor,
    Commenter,
    Viewer,
    Reviewer, // suggestions + comments
}

pub struct AclEntry {
    pub principal: PrincipalId, // actor or group
    pub role: Role,
    pub region: Option<RegionSelector>, // optional section/block lock scope
}
```

| Role | Commands allowed |
|------|------------------|
| Owner | All + ACL change |
| Editor | Content mutations |
| Reviewer | Comments + suggestions; not direct force-edit if policy says so |
| Commenter | Comments only |
| Viewer | Read only |

Authorization runs in command bus before handler execution.

---

## 6. Audit logs

```rust
pub struct AuditEvent {
    pub id: Uuid,
    pub at: Timestamp,
    pub actor: ActorId,
    pub kind: AuditKind,
    pub document_id: Option<DocumentId>,
    pub op_seq: Option<OpSeq>,
    pub detail: AuditDetail, // no full document body by default
}
```

**Defaults:**

- Store actor, action type, op_seq, command kind  
- Do **not** store full text content in audit by default  
- Enterprise policy MAY enable content-hash-only or expanded audit  

Audit log storage: separate local file or package `audit/` segment; append-only.

---

## 7. Secure sync behavior

1. Authenticate user/session before any content transfer.  
2. Authorize document ACL server-side and client-side.  
3. Encrypt stream with TLS; optional additional application-level encryption for zero-trust relays.  
4. Reject unsigned plugin packages in locked-down profiles.  
5. Sync applies remote changes only by producing local commands/merge ops through journal.  
6. Do not sync encryption keys inside document content.  

---

## 8. AI data handling

| Rule | Detail |
|------|--------|
| Default | No content sent |
| Grant | Per-document or per-selection permission UI |
| Scope | Minimal selection preferred |
| Logging | Providers MUST NOT be sent content with telemetry on |
| Storage | AI responses become suggestions; not silent applies |

---

## 9. Secret storage

- Passphrase keys: derived, not stored.  
- Session keys: OS keychain via `qf-security` when available.  
- Memory: zeroize secrets on drop (`zeroize` crate).  

---

## 10. Security tests

- Open encrypted package with wrong passphrase fails  
- Tampered ciphertext fails AEAD  
- Tampered journal hash chain detected  
- Role matrix forbids unauthorized commands  
- Plugin without capability denied  
- AI path blocked without grant  

---

*End of security specification.*
