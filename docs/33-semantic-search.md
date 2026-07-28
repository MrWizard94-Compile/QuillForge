# Semantic / Hybrid Search

**Document ID:** QF-SEARCH-001  
**Status:** Foundation  
**Law:** Derived-only indexes (rebuild from journal / canonical state)  

---

## Modes

| Mode | Algorithm |
|------|-----------|
| **Token** | Inverted index; multi-word = AND of tokens |
| **Phrase** | Case-insensitive contiguous substring |
| **Semantic** | TF–IDF bag-of-words cosine (local, offline) |
| **Hybrid** | Merge paths; best score per block |

No cloud embeddings. Suitable for local-first / offline product law.

---

## Surfaces

| Surface | Entry |
|---------|--------|
| Library | `qf_search::SearchEngine` |
| Editor | `EditorController::search_text` / `search_with_mode` |
| GUI | Home → Search · status **Search** panel |
| CLI | `cargo run -p qf-app -- search-demo` |

---

## Verify

```bash
cargo test -p qf-search
cargo test -p qf-app hybrid_search -- --nocapture
cargo run -p qf-app -- search-demo
```

---

## Out of scope (next)

- Dense vector embeddings / external model packs  
- Full citation graph queries  
- Cross-document corpus index  
