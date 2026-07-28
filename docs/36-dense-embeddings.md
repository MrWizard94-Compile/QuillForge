# Offline Dense Embeddings

**Document ID:** QF-SEARCH-DENSE-001  
**Status:** Foundation (optional model pack)  
**Law:** Local-first — no network weights  

---

## Algorithm: `hash_ngram_v1` (default)

| Piece | Detail |
|-------|--------|
| Features | Word tokens + character 3-grams |
| Projection | Signed feature hashing into fixed dim (default **64**) |
| Normalize | L2 |
| Similarity | Cosine (dot of unit vectors) |
| Pack | [`EmbeddingPack`] — `{ version, dim, seed, algo, … }` |

Same pack + same text ⇒ same vector (deterministic).

Optional packs: **`neural_hash_v1`**, **`onnx_v1`** — see `docs/39-neural-onnx-pack.md`.

---

## Surfaces

| Surface | Entry |
|---------|--------|
| Library | `qf_search::embed_text`, `SearchMode::Dense` |
| Engine | `SearchEngine::with_pack` / `set_embedding_pack` / `load_embedding_pack_path` |
| GUI | Search panel → **Dense** |
| Hybrid | Includes dense path |
| CLI | `search-demo` prints pack JSON + Dense (+ neural_hash swap) |

---

## Verify

```bash
cargo test -p qf-search
cargo run -p qf-app -- search-demo
```

---

## Out of scope

- Cloud model download  
- GPU embedding inference  
- Cross-document vector DB  
- Full ONNX graph runtime (pack identity ships first)  
