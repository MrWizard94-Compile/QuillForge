//! Optional local ONNX-pack **graph runtime** (pure Rust, no native ORT).
//!
//! Enabled with Cargo feature `onnx` on `qf-search`.
//!
//! # Backends
//!
//! | Magic / shape | Backend |
//! |---------------|---------|
//! | `QFEMB1\\0\\0` weight table | Dense bag-of-hash × embedding matrix (true graph matmul path) |
//! | Other / protobuf ONNX bytes | Identity-bound multi-scale hash (same as default `onnx_v1`) |
//!
//! Full vendor ONNX Runtime (Microsoft ORT / tract) remains optional future work;
//! this module ships a **local-first, deterministic** graph path for pack-native
//! weight tables that can be stored as `.onnx` sidecars.

use crate::embeddings::{
    dense_cosine, embed_text, EmbeddingPack, EMBED_ALGO_ONNX_V1, SEED_ONNX_BASE,
};
use qf_core::Hash256;

/// Magic for Quillforge embedding weight-table "models" (8 bytes).
pub const QFEMB1_MAGIC: &[u8; 8] = b"QFEMB1\0\0";

/// How the runtime executes an `onnx_v1` pack.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnnxBackendKind {
    /// Multi-scale hash mixed with model content identity.
    HashBound,
    /// Local weight-table graph: token hashes → rows → sum → L2.
    WeightTable,
}

/// Loaded graph runtime for dense embedding.
#[derive(Clone, Debug)]
pub struct OnnxGraphRuntime {
    pack: EmbeddingPack,
    kind: OnnxBackendKind,
    /// Row-major `[buckets × dim]` when `WeightTable`.
    weights: Vec<f32>,
    buckets: u32,
}

impl OnnxGraphRuntime {
    /// Load runtime from model bytes already associated with an `onnx_v1` pack.
    pub fn from_model_bytes(pack: EmbeddingPack, model_bytes: &[u8]) -> Result<Self, String> {
        if pack.algo != EMBED_ALGO_ONNX_V1 {
            return Err(format!(
                "OnnxGraphRuntime requires algo onnx_v1, got {}",
                pack.algo
            ));
        }
        if model_bytes.is_empty() {
            return Err("empty model bytes".into());
        }
        // Integrity: if pack declares sha256, verify.
        if let Some(hex) = &pack.model_sha256 {
            let got = hex_encode(&Hash256::sha256(model_bytes).0);
            if got != *hex {
                return Err(format!("model sha256 mismatch: pack={hex} got={got}"));
            }
        }
        if model_bytes.starts_with(QFEMB1_MAGIC) {
            let (dim, buckets, weights) = parse_qfemb1(model_bytes)?;
            if dim as u32 != pack.dim && pack.dim >= 8 {
                // Prefer pack dim when table dim differs slightly — require exact.
                if dim != pack.dim() {
                    return Err(format!("QFEMB1 dim {dim} != pack dim {}", pack.dim()));
                }
            }
            let mut pack = pack;
            pack.dim = dim as u32;
            return Ok(Self {
                pack,
                kind: OnnxBackendKind::WeightTable,
                weights,
                buckets,
            });
        }
        // Generic ONNX / opaque weights: identity-bound hash path (deterministic).
        Ok(Self {
            pack,
            kind: OnnxBackendKind::HashBound,
            weights: Vec::new(),
            buckets: 0,
        })
    }

    /// Load model file path declared on the pack (absolute or as-is).
    pub fn from_pack_model_path(pack: EmbeddingPack) -> Result<Self, String> {
        let path = pack
            .model_path
            .as_ref()
            .ok_or_else(|| "onnx pack missing model_path".to_string())?;
        let bytes = std::fs::read(path).map_err(|e| format!("read model {path}: {e}"))?;
        Self::from_model_bytes(pack, &bytes)
    }

    pub fn pack(&self) -> &EmbeddingPack {
        &self.pack
    }

    pub fn backend(&self) -> OnnxBackendKind {
        self.kind
    }

    pub fn backend_label(&self) -> &'static str {
        match self.kind {
            OnnxBackendKind::HashBound => "hash_bound_v1",
            OnnxBackendKind::WeightTable => "qfemb1_weight_table_v1",
        }
    }

    /// Embed text using the active graph backend.
    pub fn embed(&self, text: &str) -> Vec<f32> {
        match self.kind {
            OnnxBackendKind::HashBound => embed_text(text, &self.pack),
            OnnxBackendKind::WeightTable => embed_weight_table(
                text,
                self.pack.dim(),
                self.buckets as usize,
                &self.weights,
                self.pack.seed ^ SEED_ONNX_BASE,
            ),
        }
    }

    /// Build a deterministic QFEMB1 model file (for tests / offline packs).
    pub fn build_qfemb1_model(dim: usize, buckets: usize, seed: u64) -> Vec<u8> {
        let dim = dim.max(8);
        let buckets = buckets.max(8);
        let mut out = Vec::with_capacity(8 + 8 + dim * buckets * 4);
        out.extend_from_slice(QFEMB1_MAGIC);
        out.extend_from_slice(&(dim as u32).to_le_bytes());
        out.extend_from_slice(&(buckets as u32).to_le_bytes());
        // Deterministic pseudo-weights from seed (no RNG dependency).
        for b in 0..buckets {
            for d in 0..dim {
                let h = mix64(seed, b as u64, d as u64);
                // Map to [-1, 1]
                let x = ((h >> 11) as f32) / (u64::MAX >> 11) as f32 * 2.0 - 1.0;
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        out
    }
}

fn parse_qfemb1(bytes: &[u8]) -> Result<(usize, u32, Vec<f32>), String> {
    if bytes.len() < 16 {
        return Err("QFEMB1 truncated header".into());
    }
    if &bytes[0..8] != QFEMB1_MAGIC {
        return Err("not QFEMB1".into());
    }
    let dim = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
    let buckets = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]);
    if dim < 8 || buckets < 8 {
        return Err("QFEMB1 dim/buckets too small".into());
    }
    let need = 16 + dim * buckets as usize * 4;
    if bytes.len() < need {
        return Err(format!(
            "QFEMB1 truncated body: need {need} have {}",
            bytes.len()
        ));
    }
    let mut weights = Vec::with_capacity(dim * buckets as usize);
    let mut off = 16;
    while off + 4 <= need {
        let w = f32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]]);
        weights.push(w);
        off += 4;
    }
    Ok((dim, buckets, weights))
}

fn embed_weight_table(
    text: &str,
    dim: usize,
    buckets: usize,
    weights: &[f32],
    seed: u64,
) -> Vec<f32> {
    let mut v = vec![0.0f32; dim];
    if buckets == 0 || weights.len() < dim * buckets {
        return v;
    }
    for tok in text.split(|c: char| !c.is_alphanumeric()) {
        if tok.is_empty() {
            continue;
        }
        let t = tok.to_lowercase();
        let h = mix64(seed, 0x74 /*'t'*/, hash_bytes(t.as_bytes()));
        let bucket = (h as usize) % buckets;
        let row = bucket * dim;
        for d in 0..dim {
            v[d] += weights[row + d];
        }
    }
    // Character 3-gram features (same idea as neural_hash morphology).
    let chars: Vec<u8> = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| {
            let mut b = [0u8; 4];
            let s = c.encode_utf8(&mut b);
            s.as_bytes().to_vec()
        })
        .collect();
    if chars.len() >= 3 {
        for w in chars.windows(3) {
            let h = mix64(seed, 0x67 /*'g'*/, hash_bytes(w));
            let bucket = (h as usize) % buckets;
            let row = bucket * dim;
            for d in 0..dim {
                v[d] += weights[row + d] * 0.5;
            }
        }
    }
    l2_normalize(&mut v);
    v
}

fn hash_bytes(b: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &x in b {
        h ^= u64::from(x);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn mix64(seed: u64, a: u64, b: u64) -> u64 {
    let mut h = seed.rotate_left(17) ^ a.wrapping_mul(0x9E3779B97F4A7C15);
    h ^= b;
    h = h.wrapping_mul(0xBF58476D1CE4E5B9);
    h ^ (h >> 31)
}

fn l2_normalize(v: &mut [f32]) {
    let mut s = 0.0f32;
    for x in v.iter() {
        s += *x * *x;
    }
    let n = s.sqrt();
    if n > f32::EPSILON {
        for x in v.iter_mut() {
            *x /= n;
        }
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(H[(b >> 4) as usize] as char);
        out.push(H[(b & 0xf) as usize] as char);
    }
    out
}

// Re-export for tests that assert cosine API still available under feature.
#[allow(dead_code)]
fn _cosine_smoke(a: &[f32], b: &[f32]) -> f32 {
    dense_cosine(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embeddings::onnx_pack_from_model;
    use crate::embeddings::DEFAULT_DENSE_DIM;

    #[test]
    fn qfemb1_weight_table_is_deterministic_and_ranked() {
        let dim = 64usize;
        let buckets = 128usize;
        let model = OnnxGraphRuntime::build_qfemb1_model(dim, buckets, 0xC0FFEE);
        assert!(model.starts_with(QFEMB1_MAGIC));
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("emb.onnx");
        std::fs::write(&path, &model).unwrap();
        let pack = onnx_pack_from_model(&path, dim).unwrap();
        let rt = OnnxGraphRuntime::from_model_bytes(pack, &model).unwrap();
        assert_eq!(rt.backend(), OnnxBackendKind::WeightTable);
        assert_eq!(rt.backend_label(), "qfemb1_weight_table_v1");
        let a = rt.embed("neural network deep learning");
        let b = rt.embed("neural network deep learning");
        assert_eq!(a, b);
        assert_eq!(a.len(), dim);
        let c = rt.embed("sourdough bread flour water salt");
        assert!(dense_cosine(&a, &rt.embed("deep learning transformers")) > dense_cosine(&a, &c));
    }

    #[test]
    fn opaque_onnx_bytes_use_hash_bound() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("opaque.onnx");
        std::fs::write(&path, b"\x08\x01not-really-onnx-but-opaque").unwrap();
        let pack = onnx_pack_from_model(&path, DEFAULT_DENSE_DIM).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let rt = OnnxGraphRuntime::from_model_bytes(pack.clone(), &bytes).unwrap();
        assert_eq!(rt.backend(), OnnxBackendKind::HashBound);
        let v = rt.embed("hello");
        assert_eq!(v.len(), pack.dim());
        assert_eq!(v, embed_text("hello", rt.pack()));
    }
}
