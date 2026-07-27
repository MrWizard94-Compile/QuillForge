//! Content hashing primitives.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hash256(pub [u8; 32]);

impl Hash256 {
    pub const ZERO: Self = Self([0u8; 32]);

    pub fn sha256(data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let out = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&out);
        Self(arr)
    }

    pub fn chain(prev: &Self, data: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(prev.0);
        hasher.update(data);
        let out = hasher.finalize();
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&out);
        Self(arr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_stable() {
        let a = Hash256::sha256(b"quillforge");
        let b = Hash256::sha256(b"quillforge");
        assert_eq!(a, b);
        assert_ne!(a, Hash256::ZERO);
    }
}
