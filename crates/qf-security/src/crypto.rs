//! At-rest encryption: Argon2id KDF + XChaCha20-Poly1305 AEAD.
//!
//! Wire format for encrypted package files:
//! ```text
//! magic: b"QFENC1\0\0"  (8 bytes)
//! salt:  16 bytes
//! nonce: 24 bytes
//! m_cost: u32 LE (Argon2 memory KiB)
//! t_cost: u32 LE (Argon2 iterations)
//! p_cost: u32 LE (Argon2 parallelism)
//! ciphertext+tag: remaining
//! ```

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use qf_core::Hash256;
use zeroize::{Zeroize, ZeroizeOnDrop};

pub const MAGIC: &[u8; 8] = b"QFENC1\0\0";
pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;
pub const KEY_LEN: usize = 32;

/// Default Argon2id parameters (interactive desktop; not server-hard).
pub const DEFAULT_M_KIB: u32 = 19_456; // ~19 MiB
pub const DEFAULT_T_COST: u32 = 2;
pub const DEFAULT_P_COST: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("invalid passphrase")]
    InvalidPassphrase,
    #[error("corrupt ciphertext")]
    Corrupt,
    #[error("kdf: {0}")]
    Kdf(String),
    #[error("aead: {0}")]
    Aead(String),
    #[error("rng: {0}")]
    Rng(String),
}

#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct SecretKey(pub [u8; KEY_LEN]);

#[derive(Clone, Debug)]
pub struct KdfParams {
    pub m_kib: u32,
    pub t_cost: u32,
    pub p_cost: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            m_kib: DEFAULT_M_KIB,
            t_cost: DEFAULT_T_COST,
            p_cost: DEFAULT_P_COST,
        }
    }
}

/// Derive a 32-byte key from passphrase + salt.
pub fn derive_key(
    passphrase: &str,
    salt: &[u8; SALT_LEN],
    params: &KdfParams,
) -> Result<SecretKey, CryptoError> {
    if passphrase.is_empty() {
        return Err(CryptoError::InvalidPassphrase);
    }
    let argon_params = Params::new(params.m_kib, params.t_cost, params.p_cost, Some(KEY_LEN))
        .map_err(|e| CryptoError::Kdf(e.to_string()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon_params);
    let mut key = [0u8; KEY_LEN];
    argon
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|e| CryptoError::Kdf(e.to_string()))?;
    Ok(SecretKey(key))
}

fn random_bytes<const N: usize>() -> Result<[u8; N], CryptoError> {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf).map_err(|e| CryptoError::Rng(e.to_string()))?;
    Ok(buf)
}

/// Encrypt plaintext package bytes. AAD is empty (header is not authenticated separately;
/// magic/params are plaintext; tag covers ciphertext only — salt/nonce bind via construction).
pub fn encrypt_package(
    passphrase: &str,
    plaintext: &[u8],
    params: &KdfParams,
) -> Result<Vec<u8>, CryptoError> {
    let salt = random_bytes::<SALT_LEN>()?;
    let nonce_bytes = random_bytes::<NONCE_LEN>()?;
    let key = derive_key(passphrase, &salt, params)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key.0));
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ct = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: MAGIC,
            },
        )
        .map_err(|e| CryptoError::Aead(e.to_string()))?;

    let mut out = Vec::with_capacity(8 + SALT_LEN + NONCE_LEN + 12 + ct.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&salt);
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&params.m_kib.to_le_bytes());
    out.extend_from_slice(&params.t_cost.to_le_bytes());
    out.extend_from_slice(&params.p_cost.to_le_bytes());
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Decrypt package bytes produced by [`encrypt_package`].
pub fn decrypt_package(passphrase: &str, data: &[u8]) -> Result<Vec<u8>, CryptoError> {
    if data.len() < 8 + SALT_LEN + NONCE_LEN + 12 + 16 {
        return Err(CryptoError::Corrupt);
    }
    if &data[0..8] != MAGIC {
        return Err(CryptoError::Corrupt);
    }
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&data[8..8 + SALT_LEN]);
    let mut nonce_bytes = [0u8; NONCE_LEN];
    nonce_bytes.copy_from_slice(&data[8 + SALT_LEN..8 + SALT_LEN + NONCE_LEN]);
    let base = 8 + SALT_LEN + NONCE_LEN;
    let m_kib = u32::from_le_bytes(data[base..base + 4].try_into().unwrap());
    let t_cost = u32::from_le_bytes(data[base + 4..base + 8].try_into().unwrap());
    let p_cost = u32::from_le_bytes(data[base + 8..base + 12].try_into().unwrap());
    let ct = &data[base + 12..];
    let params = KdfParams {
        m_kib,
        t_cost,
        p_cost,
    };
    let key = derive_key(passphrase, &salt, &params)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key.0));
    let nonce = XNonce::from_slice(&nonce_bytes);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ct,
                aad: MAGIC,
            },
        )
        .map_err(|_| CryptoError::Corrupt)
}

/// Detect encrypted package by magic.
pub fn is_encrypted_package(data: &[u8]) -> bool {
    data.len() >= 8 && &data[0..8] == MAGIC
}

/// Integrity helper: hash of plaintext after decrypt (for seals).
pub fn plaintext_seal(plaintext: &[u8]) -> Hash256 {
    Hash256::sha256(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// PB-CRYPTO-001 — encrypt/decrypt under test KDF params within CI budget.
    #[test]
    fn budget_encrypt_decrypt_under_limit() {
        let params = KdfParams {
            m_kib: 8,
            t_cost: 1,
            p_cost: 1,
        };
        let pt = vec![0xABu8; 64 * 1024]; // 64 KiB
        let start = Instant::now();
        let ct = encrypt_package("budget-pass", &pt, &params).unwrap();
        let out = decrypt_package("budget-pass", &ct).unwrap();
        let elapsed = start.elapsed();
        assert_eq!(out, pt);
        assert!(
            elapsed < Duration::from_secs(3),
            "PB-CRYPTO-001 exceeded: {elapsed:?}"
        );
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let params = KdfParams {
            m_kib: 8, // fast for tests
            t_cost: 1,
            p_cost: 1,
        };
        let pt = b"hello quillforge package bytes";
        let ct = encrypt_package("correct horse", pt, &params).unwrap();
        assert!(is_encrypted_package(&ct));
        let out = decrypt_package("correct horse", &ct).unwrap();
        assert_eq!(out, pt);
    }

    #[test]
    fn wrong_passphrase_fails() {
        let params = KdfParams {
            m_kib: 8,
            t_cost: 1,
            p_cost: 1,
        };
        let ct = encrypt_package("a", b"secret", &params).unwrap();
        assert!(decrypt_package("b", &ct).is_err());
    }

    #[test]
    fn tamper_detected() {
        let params = KdfParams {
            m_kib: 8,
            t_cost: 1,
            p_cost: 1,
        };
        let mut ct = encrypt_package("a", b"secret", &params).unwrap();
        let last = ct.len() - 1;
        ct[last] ^= 0xff;
        assert!(decrypt_package("a", &ct).is_err());
    }
}
