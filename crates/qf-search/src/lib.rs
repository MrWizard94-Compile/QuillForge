//! Search indexes are derived only — rebuildable from journal/canonical state.

use qf_core::{BlockId, OpSeq, SourceId};
use qf_kernel::{CanonicalState, KernelEvent};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct TokenQuery {
    pub text: String,
}

#[derive(Clone, Debug)]
pub struct SearchHit {
    pub block_id: BlockId,
    pub score: f32,
    pub snippet: String,
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

#[derive(Default)]
pub struct SearchEngine {
    pub state_gen: OpSeq,
    /// token → block ids
    index: HashMap<String, Vec<BlockId>>,
}

impl SearchEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn query_token(&self, q: &TokenQuery) -> Vec<SearchHit> {
        let key = q.text.to_lowercase();
        self.index
            .get(&key)
            .into_iter()
            .flatten()
            .map(|id| SearchHit {
                block_id: *id,
                score: 1.0,
                snippet: key.clone(),
            })
            .collect()
    }

    pub fn query_graph(&self, _q: &GraphQuery) -> Vec<GraphHit> {
        vec![]
    }
}

impl SearchIngestor for SearchEngine {
    fn on_events(&mut self, _events: &[KernelEvent], state: &CanonicalState) {
        self.rebuild(state);
    }

    fn rebuild(&mut self, state: &CanonicalState) {
        self.index.clear();
        self.state_gen = state.op_seq;
        for (id, block) in &state.document.blocks {
            if let Some(text) = state.document.project_text(*id) {
                for token in text.split_whitespace() {
                    let t = token.to_lowercase();
                    self.index.entry(t).or_default().push(*id);
                }
            }
            // Index unknown type names for discoverability.
            if let qf_blocks::BlockBody::Unknown(u) = &block.body {
                self.index
                    .entry(u.original_type.to_lowercase())
                    .or_default()
                    .push(*id);
            }
        }
    }
}
