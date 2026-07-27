//! Citation and source reference helpers.

use qf_core::{PreservedMap, SourceId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceKind {
    Url,
    Doi,
    Isbn,
    Local,
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceRef {
    pub id: SourceId,
    pub kind: SourceKind,
    pub locator: String,
    pub meta: PreservedMap,
}

#[derive(Default)]
pub struct SourceTable {
    sources: BTreeMap<SourceId, SourceRef>,
}

impl SourceTable {
    pub fn insert(&mut self, source: SourceRef) {
        self.sources.insert(source.id, source);
    }

    pub fn get(&self, id: SourceId) -> Option<&SourceRef> {
        self.sources.get(&id)
    }
}

pub struct CitationFormatter;

impl CitationFormatter {
    pub fn format_inline(sources: &[&SourceRef]) -> String {
        if sources.is_empty() {
            return String::new();
        }
        let body = sources
            .iter()
            .map(|s| s.locator.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        format!("({body})")
    }
}
