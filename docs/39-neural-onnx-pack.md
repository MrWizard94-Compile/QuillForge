# Neural / ONNX Embedding Packs

**Document ID:** QF-SEARCH-NEURAL-001  
**Status:** Foundation (optional packs + optional graph runtime)  
**Law:** Local-first — no network weight download  

---

## Pack algorithms

| Algo | Dim (default) | Weights | Role |
|------|---------------|---------|------|
| `hash_ngram_v1` | 64 | None | Default signed feature hash |
| `neural_hash_v1` | 128 | None | Multi-scale offline neural-shaped pack |
| `onnx_v1` | pack-declared | **Local file** | Pack + optional graph runtime |

### `neural_hash_v1`

Tokens + prefix/suffix pieces + token bigrams + char 3/4-grams + length bucket → signed feature hashing → L2.

### `onnx_v1` (default path, no feature)

1. Pack JSON declares `model_path` (and optional `model_sha256`).  
2. Loader reads local model bytes, hashes with SHA-256.  
3. Embedding seed mixes with model hash; multi-scale features at pack dim.  

### Optional graph runtime (`--features onnx`)

Cargo feature **`onnx`** on `qf-search` enables [`OnnxGraphRuntime`]:

| Model bytes | Backend label | Behavior |
|-------------|---------------|----------|
| Magic `QFEMB1\0\0` + weight table | `qfemb1_weight_table_v1` | Token/ngram hash → row lookup → sum → L2 |
| Other / opaque / protobuf ONNX | `hash_bound_v1` | Same identity-bound multi-scale path as default |

```bash
cargo test -p qf-search --features onnx
```

```rust
let model = OnnxGraphRuntime::build_qfemb1_model(64, 128, seed);
let pack = onnx_pack_from_model(path, 64)?;
let mut eng = SearchEngine::with_pack(pack);
eng.load_onnx_runtime_from_bytes(&model)?;
```

No native ONNX Runtime / ORT DLL is required. Vendor graph engines remain a later optional path.

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

### QFEMB1 weight table layout

```text
magic:   8 bytes  "QFEMB1\0\0"
dim:     u32 LE
buckets: u32 LE
weights: f32 LE × (buckets × dim)  // row-major
```

| API | |
|-----|--|
| `load_embedding_pack_file` | Load JSON (+ hash model) |
| `onnx_pack_from_model` | Build pack from model path |
| `OnnxGraphRuntime` | feature `onnx` graph load/embed |
| `SearchEngine::load_onnx_runtime_from_bytes` | feature `onnx` |

---

## Verify

```bash
cargo test -p qf-search
cargo test -p qf-search --features onnx
cargo run -p qf-app -- search-demo
```

---

## Out of scope

- Microsoft ONNX Runtime / tract full graph (native)  
- GPU embedding inference  
- Cloud model download  
- Tokenizer vocabulary files  
