//! Search indexes are **derived only** — rebuildable from journal/canonical state.
//!
//! # Modes
//!
//! | Mode | Behavior |
//! |------|----------|
//! | **Token** | Exact token inverted index (case-folded whitespace split) |
//! | **Phrase** | Contiguous substring match (case-insensitive) |
//! | **Semantic** | Cosine similarity over TF–IDF bag-of-words vectors (local, no network) |
//! | **Hybrid** | Union of token + phrase + semantic with score merge |
//!
//! No cloud embedding API. Suitable offline-first / local-first product law.

use qf_core::{BlockId, OpSeq, SourceId};
use qf_kernel::{CanonicalState, KernelEvent};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug)]
pub struct TokenQuery {
    pub text: String,
}

/// How to interpret a free-text query string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SearchMode {
    /// Exact tokens (AND of whitespace-separated terms when multi-word).
    Token,
    /// Case-insensitive substring.
    Phrase,
    /// TF–IDF cosine (bag-of-words "semantic" without external models).
    Semantic,
    /// Token ∪ phrase ∪ semantic, best score wins per block.
    #[default]
    Hybrid,
}

#[derive(Clone, Debug)]
pub struct SearchQuery {
    pub text: String,
    pub mode: SearchMode,
    /// Max hits to return (default 50).
    pub limit: usize,
}

impl SearchQuery {
    pub fn hybrid(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            mode: SearchMode::Hybrid,
            limit: 50,
        }
    }

    pub fn semantic(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            mode: SearchMode::Semantic,
            limit: 50,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub block_id: BlockId,
    pub score: f32,
    pub snippet: String,
    /// Which path produced this hit (for UI/debug).
    pub path: HitPath,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HitPath {
    Token,
    Phrase,
    Semantic,
}

#[derive(Clone, Debug)]
pub enum GraphQuery {
    Backlinks { target: SourceId },
    OutboundLinks { block_id: BlockId },
}

#[derive(Clone, Debug)]
pub struct GraphHit {
    pub block_id: BlockId,
}

pub trait SearchIngestor {
    fn on_events(&mut self, events: &[KernelEvent], state: &CanonicalState);
    fn rebuild(&mut self, state: &CanonicalState);
}

#[derive(Clone, Debug)]
struct BlockDoc {
    block_id: BlockId,
    text: String,
    /// term → raw count
    tf: HashMap<String, u32>,
}

#[derive(Default)]
pub struct SearchEngine {
    pub state_gen: OpSeq,
    /// token → block ids
    index: HashMap<String, Vec<BlockId>>,
    docs: Vec<BlockDoc>,
    /// document frequency per term
    df: HashMap<String, u32>,
    /// term → blocks that mention citation type names etc.
    n_docs: u32,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn block_count(&self) -> usize {
        self.docs.len()
    }

    pub fn token_count(&self) -> usize {
        self.index.len()
    }

    pub fn query_token(&self, q: &TokenQuery) -> Vec<SearchHit> {
        self.query(&SearchQuery {
            text: q.text.clone(),
            mode: SearchMode::Token,
            limit: 50,
        })
    }

    pub fn query(&self, q: &SearchQuery) -> Vec<SearchHit> {
        let limit = q.limit.max(1);
        let mut hits = match q.mode {
            SearchMode::Token => self.query_token_and(&q.text),
            SearchMode::Phrase => self.query_phrase(&q.text),
            SearchMode::Semantic => self.query_semantic(&q.text),
            SearchMode::Hybrid => {
                let mut m = HashMap::<BlockId, SearchHit>::new();
                for h in self
                    .query_token_and(&q.text)
                    .into_iter()
                    .chain(self.query_phrase(&q.text))
                    .chain(self.query_semantic(&q.text))
                {
                    m.entry(h.block_id)
                        .and_modify(|e| {
                            if h.score > e.score {
                                *e = h.clone();
                            }
                        })
                        .or_insert(h);
                }
                let mut v: Vec<_> = m.into_values().collect();
                v.sort_by(|a, b| {
                    b.score
                        .partial_cmp(&a.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
                v
            }
        };
        hits.truncate(limit);
        hits
    }

    fn query_token_and(&self, text: &str) -> Vec<SearchHit> {
        let terms: Vec<String> = tokenize(text);
        if terms.is_empty() {
            return vec![];
        }
        // AND: intersection of postings
        let mut sets: Vec<HashSet<BlockId>> = terms
            .iter()
            .map(|t| {
                self.index
                    .get(t)
                    .into_iter()
                    .flatten()
                    .copied()
                    .collect()
            })
            .collect();
        sets.sort_by_key(HashSet::len);
        let mut iter = sets.into_iter();
        let Some(mut acc) = iter.next() else {
            return vec![];
        };
        for s in iter {
            acc.retain(|id| s.contains(id));
        }
        let mut hits: Vec<SearchHit> = acc
            .into_iter()
            .map(|block_id| {
                let snippet = self
                    .docs
                    .iter()
                    .find(|d| d.block_id == block_id)
                    .map(|d| snippet_around(&d.text, &terms[0], 48))
                    .unwrap_or_else(|| terms.join(" "));
                SearchHit {
                    block_id,
                    score: 1.0 + 0.1 * terms.len() as f32,
                    snippet,
                    path: HitPath::Token,
                }
            })
            .collect();
        hits.sort_by(|a, b| a.block_id.as_uuid().cmp(&b.block_id.as_uuid()));
        hits
    }

    fn query_phrase(&self, text: &str) -> Vec<SearchHit> {
        let needle = text.trim().to_lowercase();
        if needle.is_empty() {
            return vec![];
        }
        let mut hits = Vec::new();
        for doc in &self.docs {
            let hay = doc.text.to_lowercase();
            if let Some(pos) = hay.find(&needle) {
                let score = 0.85 + (needle.len() as f32 / (hay.len().max(1) as f32)).min(0.15);
                hits.push(SearchHit {
                    block_id: doc.block_id,
                    score,
                    snippet: snippet_at(&doc.text, pos, needle.len(), 48),
                    path: HitPath::Phrase,
                });
            }
        }
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits
    }

    fn query_semantic(&self, text: &str) -> Vec<SearchHit> {
        let q_terms = tokenize(text);
        if q_terms.is_empty() || self.n_docs == 0 {
            return vec![];
        }
        let q_tf = term_counts(&q_terms);
        let q_vec = self.tfidf_vec(&q_tf);
        let q_norm = l2_norm(&q_vec);
        if q_norm <= f32::EPSILON {
            return vec![];
        }
        let mut hits = Vec::new();
        for doc in &self.docs {
            let d_vec = self.tfidf_vec(&doc.tf);
            let d_norm = l2_norm(&d_vec);
            if d_norm <= f32::EPSILON {
                continue;
            }
            let sim = cosine(&q_vec, q_norm, &d_vec, d_norm);
            // Threshold: weak matches discarded for UX.
            if sim >= 0.08 {
                hits.push(SearchHit {
                    block_id: doc.block_id,
                    score: sim,
                    snippet: snippet_around(&doc.text, q_terms.first().map(|s| s.as_str()).unwrap_or(""), 48),
                    path: HitPath::Semantic,
                });
            }
        }
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits
    }

    fn tfidf_vec(&self, tf: &HashMap<String, u32>) -> HashMap<String, f32> {
        let n = self.n_docs.max(1) as f32;
        let mut out = HashMap::new();
        for (term, &count) in tf {
            let df = *self.df.get(term).unwrap_or(&1) as f32;
            let idf = ((n + 1.0) / (df + 1.0)).ln() + 1.0;
            let tf_w = 1.0 + (count as f32).ln();
            out.insert(term.clone(), tf_w * idf);
        }
        out
    }

    pub fn query_graph(&self, _q: &GraphQuery) -> Vec<GraphHit> {
        // Citation graph filled when entity wiring is productized.
        vec![]
    }
}

impl SearchIngestor for SearchEngine {
    fn on_events(&mut self, _events: &[KernelEvent], state: &CanonicalState) {
        self.rebuild(state);
    }

    fn rebuild(&mut self, state: &CanonicalState) {
        self.index.clear();
        self.docs.clear();
        self.df.clear();
        self.state_gen = state.op_seq;
        self.n_docs = 0;

        for (id, block) in &state.document.blocks {
            let mut text = state.document.project_text(*id).unwrap_or_default();
            // Index unknown type names for discoverability.
            if let qf_blocks::BlockBody::Unknown(u) = &block.body {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&u.original_type);
            }
            if let qf_blocks::BlockBody::Image(img) = &block.body {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&img.alt);
            }
            if text.trim().is_empty() {
                continue;
            }
            let terms = tokenize(&text);
            let tf = term_counts(&terms);
            for term in tf.keys() {
                *self.df.entry(term.clone()).or_insert(0) += 1;
                self.index.entry(term.clone()).or_default().push(*id);
            }
            self.docs.push(BlockDoc {
                block_id: *id,
                text,
                tf,
            });
            self.n_docs += 1;
        }
        // Dedup postings.
        for ids in self.index.values_mut() {
            ids.sort_by_key(|b| b.as_uuid());
            ids.dedup();
        }
    }
}

fn tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

fn term_counts(terms: &[String]) -> HashMap<String, u32> {
    let mut m = HashMap::new();
    for t in terms {
        *m.entry(t.clone()).or_insert(0) += 1;
    }
    m
}

fn l2_norm(v: &HashMap<String, f32>) -> f32 {
    v.values().map(|x| x * x).sum::<f32>().sqrt()
}

fn cosine(a: &HashMap<String, f32>, a_n: f32, b: &HashMap<String, f32>, b_n: f32) -> f32 {
    let mut dot = 0.0f32;
    // Iterate smaller map.
    let (small, large) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    for (k, va) in small {
        if let Some(vb) = large.get(k) {
            dot += va * vb;
        }
    }
    dot / (a_n * b_n)
}

fn snippet_around(text: &str, term: &str, radius: usize) -> String {
    let lower = text.to_lowercase();
    let t = term.to_lowercase();
    if let Some(pos) = lower.find(&t) {
        return snippet_at(text, pos, t.len(), radius);
    }
    let take: String = text.chars().take(radius * 2).collect();
    if text.chars().count() > radius * 2 {
        format!("{take}…")
    } else {
        take
    }
}

fn snippet_at(text: &str, byte_pos: usize, match_len: usize, radius: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    let end_byte = byte_pos.saturating_add(match_len).min(text.len());
    // Align to char boundary for prefix length.
    let prefix_end = text
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= byte_pos)
        .last()
        .unwrap_or(0);
    let char_pos = text[..prefix_end].chars().count().min(chars.len());
    let match_chars = text
        .get(prefix_end..end_byte)
        .map(|s| s.chars().count())
        .unwrap_or(1)
        .max(1);
    let start = char_pos.saturating_sub(radius);
    let end = (char_pos + match_chars + radius).min(chars.len());
    let mut s: String = chars[start..end].iter().collect();
    if start > 0 {
        s.insert(0, '…');
    }
    if end < chars.len() {
        s.push('…');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::{
        ActorId, BlockBodyBytes, DocumentId, Operation, ParentRef, SectionId, SeqIdGen,
        TextOffset, Timestamp,
    };
    use qf_kernel::Kernel;

    fn kernel_with_docs(paragraphs: &[&str]) -> (Kernel, Vec<BlockId>) {
        let mut gen = SeqIdGen::default();
        let mut k = Kernel::fresh(
            DocumentId::new(&mut gen),
            SectionId::new(&mut gen),
            ActorId::new(&mut gen),
            Timestamp { unix_ms: 0 },
        );
        let mut ids = Vec::new();
        for (i, p) in paragraphs.iter().enumerate() {
            let bid = BlockId::new(&mut gen);
            k.apply_batch(&[
                Operation::InsertBlock {
                    block_id: bid,
                    parent: ParentRef::DocumentRoot,
                    index: i as u32,
                    body: BlockBodyBytes {
                        type_name: "paragraph".into(),
                        payload: Vec::new(),
                    },
                },
                Operation::InsertText {
                    block_id: bid,
                    at: TextOffset(0),
                    text: (*p).into(),
                },
            ])
            .unwrap();
            ids.push(bid);
        }
        (k, ids)
    }

    #[test]
    fn token_and_requires_all_terms() {
        let (k, ids) = kernel_with_docs(&["alpha beta", "alpha only", "beta gamma"]);
        let mut eng = SearchEngine::new();
        eng.rebuild(k.state());
        let hits = eng.query_token(&TokenQuery {
            text: "alpha beta".into(),
        });
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_id, ids[0]);
    }

    #[test]
    fn phrase_finds_substring() {
        let (k, ids) = kernel_with_docs(&["The quick brown fox", "lazy dog"]);
        let mut eng = SearchEngine::new();
        eng.rebuild(k.state());
        let hits = eng.query(&SearchQuery {
            text: "quick brown".into(),
            mode: SearchMode::Phrase,
            limit: 10,
        });
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].block_id, ids[0]);
        assert!(hits[0].snippet.to_lowercase().contains("quick"));
    }

    #[test]
    fn semantic_ranks_related_block_higher() {
        let (k, ids) = kernel_with_docs(&[
            "machine learning models and neural networks for classification",
            "baking recipes with flour and sugar",
            "deep learning transformers attention mechanisms",
        ]);
        let mut eng = SearchEngine::new();
        eng.rebuild(k.state());
        let hits = eng.query(&SearchQuery::semantic("neural network deep learning"));
        assert!(!hits.is_empty());
        // Top hit should be ML-related (0 or 2), not baking (1).
        assert_ne!(hits[0].block_id, ids[1]);
        assert!(hits[0].path == HitPath::Semantic);
        assert!(hits[0].score > 0.08);
    }

    #[test]
    fn hybrid_finds_exact_and_related() {
        let (k, ids) = kernel_with_docs(&[
            "quillforge local first document editor",
            "completely unrelated astronomy notes",
            "local-first offline journal durability",
        ]);
        let mut eng = SearchEngine::new();
        eng.rebuild(k.state());
        let hits = eng.query(&SearchQuery::hybrid("local first"));
        let hit_ids: HashSet<_> = hits.iter().map(|h| h.block_id).collect();
        assert!(hit_ids.contains(&ids[0]));
        // Semantic should also surface the third block.
        assert!(
            hit_ids.contains(&ids[2]) || hits.iter().any(|h| h.path == HitPath::Semantic),
            "expected related offline/local block among {hits:?}"
        );
    }

    #[test]
    fn rebuild_is_deterministic() {
        let (k, _) = kernel_with_docs(&["one two three"]);
        let mut a = SearchEngine::new();
        let mut b = SearchEngine::new();
        a.rebuild(k.state());
        b.rebuild(k.state());
        assert_eq!(a.token_count(), b.token_count());
        assert_eq!(a.block_count(), b.block_count());
        assert_eq!(a.state_gen, b.state_gen);
    }
}
