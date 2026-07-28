# Neural / ONNX Embedding Packs

**Document ID:** QF-SEARCH-NEURAL-001  
**Status:** Foundation (optional packs)  
**Law:** Local-first — no network weight download  

---

## Pack algorithms

| Algo | Dim (default) | Weights | Role |
|------|---------------|---------|------|
| `hash_ngram_v1` | 64 | None | Default signed feature hash |
| `neural_hash_v1` | 128 | None | Multi-scale offline neural-shaped pack |
| `onnx_v1` | pack-declared | **Local file** | Space bound to model content hash |

### `neural_hash_v1`

Tokens + prefix/suffix pieces + token bigrams + char 3/4-grams + length bucket → signed feature hashing → L2.

Use when you want a richer offline dense path without shipping ONNX runtime.

### `onnx_v1`

1. Pack JSON declares `model_path` (and optional `model_sha256`).  
2. Loader reads local model bytes, hashes with SHA-256.  
3. Embedding seed mixes with model hash; multi-scale features at pack dim.  

This **does not run ONNX graph inference** yet. It makes pack identity and offline search stable while a future `onnx` runtime feature can swap the embed implementation without changing the pack file format.

---

## Pack file format

```json
{
  "version": 1,
  "dim": 128,
  "seed": 1,
  "algo": "onnx_v1",
  "model_path": "mini.onnx",
  "model_sha256": "optional hex"
}
```

| API | |
|-----|--|
| `load_embedding_pack_file` | Load JSON (+ hash ONNX model) |
| `write_embedding_pack_file` | Write JSON |
| `onnx_pack_from_model` | Build pack from model path |
| `EmbeddingPack::neural_hash_v1` | In-memory neural pack |
| `SearchEngine::set_embedding_pack` | Swap pack + re-embed docs |
| `SearchEngine::load_embedding_pack_path` | Disk pack → engine |

---

## Verify

```bash
cargo test -p qf-search
cargo test -p qf-search neural_hash -- --nocapture
cargo test -p qf-search onnx_pack -- --nocapture
cargo run -p qf-app -- search-demo
```

---

## Out of scope

- ONNX Runtime / tract graph execution  
- GPU embedding inference  
- Cloud model download  
- Tokenizer vocabulary files  
