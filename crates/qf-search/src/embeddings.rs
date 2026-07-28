//! Offline dense embeddings — signed feature hashing (tokens + char n-grams).
//!
//! # Model pack
//!
//! [`EmbeddingPack`] is a tiny deterministic "model": dimension + seed + algo id.
//! No network weights. Same pack + same text ⇒ same vector (reproducible).

use serde::{Deserialize, Serialize};

/// Default dense dimension for document blocks.
pub const DEFAULT_DENSE_DIM: usize = 64;

/// Embedding algorithm identifier (versioned).
pub const EMBED_ALGO_HASH_NGRAM_V1: &str = "hash_ngram_v1";

/// Portable offline embedding configuration ("model pack" metadata).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingPack {
    pub version: u32,
    pub dim: u32,
    pub seed: u64,
    pub algo: String,
}

impl Default for EmbeddingPack {
    fn default() -> Self {
        Self {
            version: 1,
            dim: DEFAULT_DENSE_DIM as u32,
            seed: 0x51F1_F0CE_D0C5_EED5, // "quillforge seed" flavor
            algo: EMBED_ALGO_HASH_NGRAM_V1.into(),
        }
    }
}

impl EmbeddingPack {
    pub fn v1(dim: usize, seed: u64) -> Self {
        Self {
            version: 1,
            dim: dim.max(8) as u32,
            seed,
            algo: EMBED_ALGO_HASH_NGRAM_V1.into(),
        }
    }

    pub fn dim(&self) -> usize {
        self.dim as usize
    }

    /// Encode pack as stable JSON (for sidecar / docs / hash).
    pub fn to_json(&self) -> String {
        format!(
            r#"{{"version":{},"dim":{},"seed":{},"algo":"{}"}}"#,
            self.version, self.dim, self.seed, self.algo
        )
    }
}

/// Embed free text into a dense L2-normalized vector.
pub fn embed_text(text: &str, pack: &EmbeddingPack) -> Vec<f32> {
    let dim = pack.dim();
    let mut v = vec![0.0f32; dim];
    if dim == 0 {
        return v;
    }
    // Tokens
    for tok in text.split(|c: char| !c.is_alphanumeric()) {
        if tok.is_empty() {
            continue;
        }
        let t = tok.to_lowercase();
        accumulate_feature(&mut v, pack.seed, b"t:", t.as_bytes());
    }
    // Character 3-grams over normalized text (captures morphology / near-semantic)
    let chars: Vec<char> = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    if chars.len() >= 3 {
        for w in chars.windows(3) {
            let mut buf = [0u8; 12];
            let mut n = 0;
            for ch in w {
                for b in ch.encode_utf8(&mut [0u8; 4]).as_bytes() {
                    if n < buf.len() {
                        buf[n] = *b;
                        n += 1;
                    }
                }
            }
            accumulate_feature(&mut v, pack.seed, b"g3:", &buf[..n]);
        }
    }
    l2_normalize_inplace(&mut v);
    v
}

fn accumulate_feature(v: &mut [f32], seed: u64, prefix: &[u8], feat: &[u8]) {
    let dim = v.len() as u64;
    if dim == 0 {
        return;
    }
    let h = hash64(seed, prefix, feat);
    let idx = (h % dim) as usize;
    // Sign from high bit
    let sign = if (h >> 63) == 0 { 1.0f32 } else { -1.0f32 };
    v[idx] += sign;
}

fn hash64(seed: u64, prefix: &[u8], feat: &[u8]) -> u64 {
    // FNV-1a 64 with seed mix (deterministic, no deps).
    let mut h = 0xcbf29ce484222325u64 ^ seed.rotate_left(13);
    for &b in prefix.iter().chain(feat.iter()) {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn l2_normalize_inplace(v: &mut [f32]) {
    let mut s = 0.0f32;
    for x in v.iter() {
        s += x * x;
    }
    let n = s.sqrt();
    if n > f32::EPSILON {
        for x in v.iter_mut() {
            *x /= n;
        }
    }
}

/// Cosine similarity of two equal-length dense vectors (assumed L2-normalized → dot).
pub fn dense_cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
    }
    dot.clamp(-1.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embed_is_deterministic() {
        let pack = EmbeddingPack::default();
        let a = embed_text("neural networks and deep learning", &pack);
        let b = embed_text("neural networks and deep learning", &pack);
        assert_eq!(a, b);
        assert_eq!(a.len(), DEFAULT_DENSE_DIM);
        let norm: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4 || norm < 1e-6);
    }

    #[test]
    fn related_texts_have_higher_cosine_than_unrelated() {
        let pack = EmbeddingPack::default();
        let ml1 = embed_text("machine learning neural network models", &pack);
        let ml2 = embed_text("deep learning transformers neural nets", &pack);
        let bake = embed_text("sourdough bread flour water salt oven", &pack);
        let sim_ml = dense_cosine(&ml1, &ml2);
        let sim_x = dense_cosine(&ml1, &bake);
        assert!(
            sim_ml > sim_x,
            "related {sim_ml} should exceed unrelated {sim_x}"
        );
    }

    #[test]
    fn pack_json_stable() {
        let p = EmbeddingPack::default();
        assert!(p.to_json().contains("hash_ngram_v1"));
        assert!(p.to_json().contains("\"dim\":64"));
    }
}
