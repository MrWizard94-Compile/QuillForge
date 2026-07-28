//! Offline dense embeddings — pack-driven, local-first.
//!
//! # Packs
//!
//! | Algo | Role |
//! |------|------|
//! | [`EMBED_ALGO_HASH_NGRAM_V1`] | Tokens + char 3-grams (default, dim 64) |
//! | [`EMBED_ALGO_NEURAL_HASH_V1`] | Multi-scale offline neural-shaped pack (dim 128) |
//! | [`EMBED_ALGO_ONNX_V1`] | Local ONNX pack: model file identity + multi-scale embed |
//!
//! No cloud weights. Full ONNX graph inference is optional later; `onnx_v1`
//! binds the embedding space to a local model file's content hash so pack
//! swaps change vectors deterministically without a runtime dependency.

use qf_core::Hash256;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Default dense dimension for document blocks (hash_ngram_v1).
pub const DEFAULT_DENSE_DIM: usize = 64;

/// Default dimension for neural_hash_v1 multi-scale pack.
pub const NEURAL_HASH_DIM: usize = 128;

/// Embedding algorithm identifiers (versioned).
pub const EMBED_ALGO_HASH_NGRAM_V1: &str = "hash_ngram_v1";
pub const EMBED_ALGO_NEURAL_HASH_V1: &str = "neural_hash_v1";
pub const EMBED_ALGO_ONNX_V1: &str = "onnx_v1";

const SEED_HASH_NGRAM: u64 = 0x51F1_F0CE_D0C5_EED5;
const SEED_NEURAL_HASH: u64 = 0x4E45_5552_414C_4831; // "NEURALH1" flavor
const SEED_ONNX_BASE: u64 = 0x0F4E_4E58_5041_434B; // "ONNXPAC" flavor

/// Portable offline embedding configuration ("model pack" metadata).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmbeddingPack {
    pub version: u32,
    pub dim: u32,
    pub seed: u64,
    pub algo: String,
    /// Optional local model path (ONNX pack). Relative paths resolve against pack file parent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_path: Option<String>,
    /// Optional SHA-256 hex of model bytes (integrity / identity).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_sha256: Option<String>,
}

impl Default for EmbeddingPack {
    fn default() -> Self {
        Self {
            version: 1,
            dim: DEFAULT_DENSE_DIM as u32,
            seed: SEED_HASH_NGRAM,
            algo: EMBED_ALGO_HASH_NGRAM_V1.into(),
            model_path: None,
            model_sha256: None,
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
            model_path: None,
            model_sha256: None,
        }
    }

    /// Offline multi-scale neural-shaped pack (no weights file).
    pub fn neural_hash_v1() -> Self {
        Self {
            version: 1,
            dim: NEURAL_HASH_DIM as u32,
            seed: SEED_NEURAL_HASH,
            algo: EMBED_ALGO_NEURAL_HASH_V1.into(),
            model_path: None,
            model_sha256: None,
        }
    }

    pub fn dim(&self) -> usize {
        self.dim as usize
    }

    pub fn is_onnx(&self) -> bool {
        self.algo == EMBED_ALGO_ONNX_V1
    }

    pub fn is_neural_hash(&self) -> bool {
        self.algo == EMBED_ALGO_NEURAL_HASH_V1
    }

    /// Encode pack as stable JSON (for sidecar / docs / hash).
    pub fn to_json(&self) -> String {
        let mut s = format!(
            r#"{{"version":{},"dim":{},"seed":{},"algo":"{}""#,
            self.version, self.dim, self.seed, self.algo
        );
        if let Some(p) = &self.model_path {
            s.push_str(&format!(r#","model_path":"{}""#, json_escape(p)));
        }
        if let Some(h) = &self.model_sha256 {
            s.push_str(&format!(r#","model_sha256":"{}""#, json_escape(h)));
        }
        s.push('}');
        s
    }
}

fn json_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Embed free text into a dense L2-normalized vector using the pack algo.
pub fn embed_text(text: &str, pack: &EmbeddingPack) -> Vec<f32> {
    match pack.algo.as_str() {
        EMBED_ALGO_NEURAL_HASH_V1 => embed_neural_hash(text, pack),
        EMBED_ALGO_ONNX_V1 => embed_onnx_bound(text, pack),
        _ => embed_hash_ngram(text, pack),
    }
}

/// Classic signed feature hashing (tokens + char 3-grams).
fn embed_hash_ngram(text: &str, pack: &EmbeddingPack) -> Vec<f32> {
    let dim = pack.dim();
    let mut v = vec![0.0f32; dim];
    if dim == 0 {
        return v;
    }
    for tok in text.split(|c: char| !c.is_alphanumeric()) {
        if tok.is_empty() {
            continue;
        }
        let t = tok.to_lowercase();
        accumulate_feature(&mut v, pack.seed, b"t:", t.as_bytes());
    }
    accumulate_char_ngrams(&mut v, pack.seed, text, 3, b"g3:");
    l2_normalize_inplace(&mut v);
    v
}

/// Multi-scale offline pack: tokens, bigrams, char 3/4-grams, length bucket.
fn embed_neural_hash(text: &str, pack: &EmbeddingPack) -> Vec<f32> {
    let dim = pack.dim().max(8);
    let mut v = vec![0.0f32; dim];
    let seed = pack.seed;

    let tokens: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect();

    for tok in &tokens {
        accumulate_feature(&mut v, seed, b"t:", tok.as_bytes());
        // Subword-ish prefixes/suffixes (offline stand-in for BPE pieces).
        let chars: Vec<char> = tok.chars().collect();
        if chars.len() >= 3 {
            let pre: String = chars.iter().take(3).collect();
            let suf: String = chars
                .iter()
                .rev()
                .take(3)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            accumulate_feature(&mut v, seed, b"pre:", pre.as_bytes());
            accumulate_feature(&mut v, seed, b"suf:", suf.as_bytes());
        }
    }
    for w in tokens.windows(2) {
        let bigram = format!("{}_{}", w[0], w[1]);
        accumulate_feature(&mut v, seed, b"bi:", bigram.as_bytes());
    }
    accumulate_char_ngrams(&mut v, seed, text, 3, b"g3:");
    accumulate_char_ngrams(&mut v, seed, text, 4, b"g4:");

    let bucket = (tokens.len().min(64) as u64).to_le_bytes();
    accumulate_feature(&mut v, seed, b"len:", &bucket);

    l2_normalize_inplace(&mut v);
    v
}

/// ONNX pack path: seed mixed with model identity hash, then multi-scale embed.
fn embed_onnx_bound(text: &str, pack: &EmbeddingPack) -> Vec<f32> {
    let mut mixed = pack.clone();
    if let Some(hex) = &pack.model_sha256 {
        let mut seed = pack.seed ^ SEED_ONNX_BASE;
        for (i, b) in hex.as_bytes().iter().enumerate().take(16) {
            seed ^= u64::from(*b) << ((i % 8) * 8);
        }
        mixed.seed = seed;
    } else {
        mixed.seed = pack.seed ^ SEED_ONNX_BASE;
    }
    mixed.algo = EMBED_ALGO_NEURAL_HASH_V1.into();
    embed_neural_hash(text, &mixed)
}

fn accumulate_char_ngrams(v: &mut [f32], seed: u64, text: &str, n: usize, prefix: &[u8]) {
    let chars: Vec<char> = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect();
    if chars.len() < n {
        return;
    }
    for w in chars.windows(n) {
        let mut buf = [0u8; 16];
        let mut len = 0;
        for ch in w {
            for b in ch.encode_utf8(&mut [0u8; 4]).as_bytes() {
                if len < buf.len() {
                    buf[len] = *b;
                    len += 1;
                }
            }
        }
        accumulate_feature(v, seed, prefix, &buf[..len]);
    }
}

fn accumulate_feature(v: &mut [f32], seed: u64, prefix: &[u8], feat: &[u8]) {
    let dim = v.len() as u64;
    if dim == 0 {
        return;
    }
    let h = hash64(seed, prefix, feat);
    let idx = (h % dim) as usize;
    let sign = if (h >> 63) == 0 { 1.0f32 } else { -1.0f32 };
    v[idx] += sign;
}

fn hash64(seed: u64, prefix: &[u8], feat: &[u8]) -> u64 {
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

// --- Pack file I/O (optional neural / ONNX packs on disk) ---

/// Load an embedding pack JSON sidecar (local file only).
///
/// For `onnx_v1`, `model_path` is required and must point at a readable file.
/// Model bytes are hashed into `model_sha256` when not already set.
pub fn load_embedding_pack_file(path: &Path) -> Result<EmbeddingPack, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("read pack {}: {e}", path.display()))?;
    let mut pack: EmbeddingPack = serde_json::from_str(&raw)
        .map_err(|e| format!("parse pack {}: {e}", path.display()))?;
    if pack.dim < 8 {
        pack.dim = 8;
    }
    if pack.algo.is_empty() {
        pack.algo = EMBED_ALGO_HASH_NGRAM_V1.into();
    }
    if pack.algo == EMBED_ALGO_ONNX_V1 {
        let model_rel = pack
            .model_path
            .clone()
            .ok_or_else(|| "onnx_v1 pack requires model_path".to_string())?;
        let model_path = resolve_model_path(path, &model_rel);
        let bytes = std::fs::read(&model_path)
            .map_err(|e| format!("onnx model {}: {e}", model_path.display()))?;
        if bytes.is_empty() {
            return Err("onnx model file is empty".into());
        }
        let hash = Hash256::sha256(&bytes);
        let hex = hex_encode(&hash.0);
        if let Some(declared) = &pack.model_sha256 {
            if declared != &hex {
                return Err(format!(
                    "model_sha256 mismatch: pack={declared} file={hex}"
                ));
            }
        } else {
            pack.model_sha256 = Some(hex);
        }
        pack.model_path = Some(model_rel);
    }
    Ok(pack)
}

/// Write pack JSON to path.
pub fn write_embedding_pack_file(path: &Path, pack: &EmbeddingPack) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("create_dir {}: {e}", parent.display()))?;
        }
    }
    // Prefer serde_json for round-trip load fidelity.
    let body = serde_json::to_string_pretty(pack).map_err(|e| e.to_string())?;
    std::fs::write(path, body).map_err(|e| format!("write pack {}: {e}", path.display()))
}

/// Build an `onnx_v1` pack bound to a local model file (hashes contents).
pub fn onnx_pack_from_model(model_path: &Path, dim: usize) -> Result<EmbeddingPack, String> {
    let bytes = std::fs::read(model_path)
        .map_err(|e| format!("read model {}: {e}", model_path.display()))?;
    if bytes.is_empty() {
        return Err("onnx model file is empty".into());
    }
    let hash = Hash256::sha256(&bytes);
    let hex = hex_encode(&hash.0);
    let mut seed = SEED_ONNX_BASE;
    for (i, b) in hash.0.iter().enumerate().take(8) {
        seed ^= u64::from(*b) << (i * 8);
    }
    Ok(EmbeddingPack {
        version: 1,
        dim: dim.max(8) as u32,
        seed,
        algo: EMBED_ALGO_ONNX_V1.into(),
        model_path: Some(model_path.display().to_string()),
        model_sha256: Some(hex),
    })
}

fn resolve_model_path(pack_path: &Path, model_rel: &str) -> PathBuf {
    let p = Path::new(model_rel);
    if p.is_absolute() {
        return p.to_path_buf();
    }
    pack_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(p)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

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

    #[test]
    fn neural_hash_pack_ranks_related() {
        let pack = EmbeddingPack::neural_hash_v1();
        assert_eq!(pack.dim(), NEURAL_HASH_DIM);
        let a = embed_text("neural network deep learning models", &pack);
        let b = embed_text("deep learning transformers attention", &pack);
        let c = embed_text("sourdough bread flour water salt", &pack);
        assert!(dense_cosine(&a, &b) > dense_cosine(&a, &c));
        assert_eq!(a, embed_text("neural network deep learning models", &pack));
    }

    #[test]
    fn onnx_pack_binds_to_model_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let model_a = dir.path().join("a.onnx");
        let model_b = dir.path().join("b.onnx");
        std::fs::write(&model_a, b"fake-onnx-weights-aaa").unwrap();
        std::fs::write(&model_b, b"fake-onnx-weights-bbb").unwrap();
        let pa = onnx_pack_from_model(&model_a, 128).unwrap();
        let pb = onnx_pack_from_model(&model_b, 128).unwrap();
        assert!(pa.is_onnx());
        assert_ne!(pa.model_sha256, pb.model_sha256);
        let va = embed_text("hello world", &pa);
        let vb = embed_text("hello world", &pb);
        assert_ne!(va, vb, "different models must induce different spaces");
        assert_eq!(va, embed_text("hello world", &pa));
    }

    #[test]
    fn pack_file_round_trip_neural() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("neural.qfpack.json");
        let pack = EmbeddingPack::neural_hash_v1();
        write_embedding_pack_file(&path, &pack).unwrap();
        let loaded = load_embedding_pack_file(&path).unwrap();
        assert_eq!(loaded.algo, EMBED_ALGO_NEURAL_HASH_V1);
        assert_eq!(loaded.dim(), NEURAL_HASH_DIM);
    }

    #[test]
    fn pack_file_loads_onnx_with_relative_model() {
        let dir = tempfile::tempdir().unwrap();
        let model = dir.path().join("mini.onnx");
        let mut f = std::fs::File::create(&model).unwrap();
        f.write_all(b"\x08\x01onnx-fake-payload").unwrap();
        drop(f);
        let pack_path = dir.path().join("model.qfpack.json");
        let body = r#"{
          "version": 1,
          "dim": 96,
          "seed": 1,
          "algo": "onnx_v1",
          "model_path": "mini.onnx"
        }"#;
        std::fs::write(&pack_path, body).unwrap();
        let loaded = load_embedding_pack_file(&pack_path).unwrap();
        assert!(loaded.is_onnx());
        assert!(loaded.model_sha256.is_some());
        assert_eq!(loaded.dim(), 96);
        let v = embed_text("attention layers", &loaded);
        assert_eq!(v.len(), 96);
    }
}
