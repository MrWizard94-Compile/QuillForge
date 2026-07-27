# Layout golden fixtures

Golden hashes are **content hashes** of `LayoutTree` produced by  
`LayoutEngine::full_layout` with `LayoutConfig::default()`.

| File | Meaning |
|------|---------|
| `empty.hash` | Empty document, one blank page |
| `mixed_blocks.hash` | Multi-type block document (paragraph, heading, code, list, unknown, page break, …) |
| `long_paragraph.hash` | Long text forcing multi-line height estimate |

## Regeneration policy

Hashes are intentional contracts. Only regenerate when layout semantics change **deliberately**, and update this README with the reason in the same commit.

Format: 64 lowercase hex characters (SHA-256) + newline.
