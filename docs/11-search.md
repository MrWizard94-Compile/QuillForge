# Search Architecture Specification

**Document ID:** QF-SRCH-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Principle

Search indexes are **derived only**. They are built from:

- Canonical state snapshots, and/or  
- Journal/kernel events  

They are **never** authoritative. Corruption of index → rebuild. Index MUST NOT be required for document open.

---

## 2. Search modes

| Mode | Purpose | v1 |
|------|---------|----|
| **Token search** | Full-text lexical | Required |
| **Semantic search** | Embedding similarity | Optional provider |
| **Graph search** | Citations, links, refs | Required (basic) |

---

## 3. Indexing pipeline

```
KernelEvent → SearchIngestor → Analyzer → IndexWriter → Segment files
```

```rust
pub trait SearchIngestor {
    fn on_events(&mut self, events: &[KernelEvent], state: &CanonicalState) -> Result<()>;
    fn rebuild(&mut self, state: &CanonicalState) -> Result<()>;
}
```

**Rules:**

- Ingest is idempotent w.r.t. `op_seq`.  
- Background task; never blocks commit.  
- On recovery, if index gen ≠ kernel `op_seq`, rebuild or catch up.

---

## 4. Token search

### 4.1 Analyzer

1. Iterate text-bearing blocks in stable order  
2. Unicode segmentation (word breaks)  
3. Lowercase (Unicode case fold)  
4. Optional stopword list (language-specific; configurable)  
5. Produce tokens with offsets → `BlockId` + span offsets  

### 4.2 Index structure

- Inverted index: `token → postings (block_id, pos)*`  
- Doc store optional for snippets  
- Storage: tantivy-like custom or embed tantivy as implementation detail  

**Decision:** Use an embedded inverted index library **behind** `qf-search` traits (implementation may use tantivy). Trait boundary allows replacement.

```rust
pub struct TokenQuery {
    pub text: String,
    pub filters: SearchFilters,
}

pub struct SearchHit {
    pub block_id: BlockId,
    pub score: f32,
    pub snippet: String,
    pub range: Option<DocRangeStable>,
}
```

---

## 5. Semantic search

```rust
pub trait EmbeddingProvider: Send + Sync {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;
    fn dim(&self) -> usize;
}
```

- Default: disabled (no model download without consent).  
- Vectors stored in derived index only.  
- Unknown blocks: embed only if plugin provides text extract; never embed raw binary without grant.  
- AI embedding providers use same content permission model.

---

## 6. Graph search

Graph nodes: blocks, sources, assets, comments (optional).  
Edges: citation refs, links, embed refs, section parent.

```rust
pub enum GraphQuery {
    Backlinks { target: SourceId },
    OutboundLinks { block_id: BlockId },
    Path { from: NodeId, to: NodeId, max_depth: u8 },
}
```

Built from canonical state; updated on relevant events.

---

## 7. Plugin search providers

Capability `search.provider` allows additional query routes. Host merges results with trust labels. Providers cannot write index for core token stream.

---

## 8. Privacy

- Indexes live local beside package or in app cache dir keyed by doc id + content hash.  
- Telemetry MUST NOT upload queries or document text.  
- Encrypted packages: index must be encrypted or stored only unlocked in memory/secure temp.

---

## 9. Quality gates

- Event-sourced index matches rebuild-from-scratch for same `op_seq`  
- Delete block removes postings  
- Unknown block type string searchable  
- p95 query < 50ms for 10k blocks token search on reference hardware  

---

*End of search specification.*
