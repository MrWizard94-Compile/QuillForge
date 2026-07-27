//! Citation and source reference helpers (Zotero-inspired integrity).

use qf_core::{PreservedMap, SourceId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

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

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CitationError {
    #[error("empty locator")]
    EmptyLocator,
    #[error("invalid url locator")]
    InvalidUrl,
    #[error("invalid doi locator")]
    InvalidDoi,
    #[error("invalid isbn locator")]
    InvalidIsbn,
}

/// Validate source locator shape for known kinds (fail closed on empty).
pub fn validate_source(source: &SourceRef) -> Result<(), CitationError> {
    let loc = source.locator.trim();
    if loc.is_empty() {
        return Err(CitationError::EmptyLocator);
    }
    match source.kind {
        SourceKind::Url => {
            if !(loc.starts_with("https://") || loc.starts_with("http://")) {
                return Err(CitationError::InvalidUrl);
            }
        }
        SourceKind::Doi => {
            // Accept bare DOI or doi.org URL form.
            let bare = loc
                .trim_start_matches("https://doi.org/")
                .trim_start_matches("http://doi.org/");
            if !bare.contains('/') || bare.len() < 3 {
                return Err(CitationError::InvalidDoi);
            }
        }
        SourceKind::Isbn => {
            let digits: String = loc
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == 'X')
                .collect();
            if digits.len() != 10 && digits.len() != 13 {
                return Err(CitationError::InvalidIsbn);
            }
        }
        SourceKind::Local | SourceKind::Other => {}
    }
    Ok(())
}

#[derive(Default)]
pub struct SourceTable {
    sources: BTreeMap<SourceId, SourceRef>,
}

impl SourceTable {
    pub fn insert(&mut self, source: SourceRef) -> Result<(), CitationError> {
        validate_source(&source)?;
        self.sources.insert(source.id, source);
        Ok(())
    }

    pub fn insert_unchecked(&mut self, source: SourceRef) {
        self.sources.insert(source.id, source);
    }

    pub fn get(&self, id: SourceId) -> Option<&SourceRef> {
        self.sources.get(&id)
    }

    pub fn len(&self) -> usize {
        self.sources.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
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
            .map(|s| format_source_marker(s))
            .collect::<Vec<_>>()
            .join("; ");
        format!("({body})")
    }
}

/// Stable, human-readable marker for export and bibliography lines.
pub fn format_source_marker(source: &SourceRef) -> String {
    match source.kind {
        SourceKind::Url => source.locator.clone(),
        SourceKind::Doi => {
            if source.locator.starts_with("http") {
                source.locator.clone()
            } else {
                format!("doi:{}", source.locator)
            }
        }
        SourceKind::Isbn => format!("isbn:{}", source.locator),
        SourceKind::Local => format!("local:{}", source.locator),
        SourceKind::Other => source.locator.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::SeqIdGen;

    #[test]
    fn rejects_empty_and_bad_url() {
        let mut gen = SeqIdGen::default();
        let empty = SourceRef {
            id: SourceId::new(&mut gen),
            kind: SourceKind::Url,
            locator: String::new(),
            meta: PreservedMap::new(),
        };
        assert_eq!(validate_source(&empty), Err(CitationError::EmptyLocator));
        let bad_url = SourceRef {
            id: SourceId::new(&mut gen),
            kind: SourceKind::Url,
            locator: "not-a-url".into(),
            meta: PreservedMap::new(),
        };
        assert_eq!(validate_source(&bad_url), Err(CitationError::InvalidUrl));
    }

    #[test]
    fn accepts_doi_and_isbn() {
        let mut gen = SeqIdGen::default();
        let doi = SourceRef {
            id: SourceId::new(&mut gen),
            kind: SourceKind::Doi,
            locator: "10.1000/xyz123".into(),
            meta: PreservedMap::new(),
        };
        assert!(validate_source(&doi).is_ok());
        let isbn = SourceRef {
            id: SourceId::new(&mut gen),
            kind: SourceKind::Isbn,
            locator: "9780306406157".into(),
            meta: PreservedMap::new(),
        };
        assert!(validate_source(&isbn).is_ok());
    }
}
