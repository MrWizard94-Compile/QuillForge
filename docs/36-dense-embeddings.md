# Offline Dense Embeddings

**Document ID:** QF-SEARCH-DENSE-001  
**Status:** Foundation (optional model pack)  
**Law:** Local-first — no network weights  

---

## Algorithm: `hash_ngram_v1`

| Piece | Detail |
|-------|--------|
| Features | Word tokens + character 3-grams |
| Projection | Signed feature hashing into fixed dim (default **64**) |
| Normalize | L2 |
| Similarity | Cosine (dot of unit vectors) |
| Pack | [`EmbeddingPack`] — `{ version, dim, seed, algo }` |

Same pack + same text ⇒ same vector (deterministic).

---

## Surfaces

| Surface | Entry |
|---------|--------|
| Library | `qf_search::embed_text`, `SearchMode::Dense` |
| Engine | `SearchEngine::with_pack` / `embedding_pack` |
| GUI | Search panel → **Dense** |
| Hybrid | Includes dense path |
| CLI | `search-demo` prints pack JSON + Dense hits |

---

## Verify

```bash
cargo test -p qf-search
cargo run -p qf-app -- search-demo
```

---

## Out of scope

- Downloaded neural weights / ONNX packs  
- GPU embedding inference  
- Cross-document vector DB  
