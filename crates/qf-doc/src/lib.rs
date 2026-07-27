//! Document AST structures and pure command expansion helpers.
//!
//! Handlers expand commands into operations; they never touch journal or kernel.

mod editing;

pub use editing::*;

use qf_blocks::{BlockBody, BlockMetadata};
use qf_core::{ActorId, BlockId, DocumentId, PreservedMap, SectionId, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub id: DocumentId,
    pub meta: DocumentMeta,
    pub root_section_id: SectionId,
    pub sections: BTreeMap<SectionId, Section>,
    pub blocks: BTreeMap<BlockId, Block>,
    pub section_order: Vec<SectionId>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DocumentMeta {
    pub title: String,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub authors: Vec<ActorId>,
    pub language: Option<String>,
    pub custom: PreservedMap,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub id: SectionId,
    pub parent: Option<SectionId>,
    pub children: Vec<SectionId>,
    pub blocks: Vec<BlockId>,
    pub meta: SectionMeta,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SectionMeta {
    pub title: Option<String>,
    pub custom: PreservedMap,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Block {
    pub id: BlockId,
    pub meta: BlockMetadata,
    pub body: BlockBody,
}

impl Document {
    pub fn empty(id: DocumentId, root: SectionId, actor: ActorId, at: Timestamp) -> Self {
        let mut sections = BTreeMap::new();
        sections.insert(
            root,
            Section {
                id: root,
                parent: None,
                children: vec![],
                blocks: vec![],
                meta: SectionMeta::default(),
            },
        );
        Self {
            id,
            meta: DocumentMeta {
                title: String::new(),
                created_at: at,
                updated_at: at,
                authors: vec![actor],
                language: None,
                custom: PreservedMap::new(),
            },
            root_section_id: root,
            sections,
            blocks: BTreeMap::new(),
            section_order: vec![root],
        }
    }

    /// Project editable text for a text-bearing block.
    pub fn project_text(&self, block_id: BlockId) -> Option<String> {
        let block = self.blocks.get(&block_id)?;
        match &block.body {
            BlockBody::Paragraph(p) => Some(p.spans.iter().map(|s| s.text.as_str()).collect()),
            BlockBody::Heading(h) => Some(h.spans.iter().map(|s| s.text.as_str()).collect()),
            BlockBody::Quote(q) => Some(q.spans.iter().map(|s| s.text.as_str()).collect()),
            BlockBody::Code(c) => Some(c.text.clone()),
            _ => None,
        }
    }

    pub fn contains_block(&self, block_id: BlockId) -> bool {
        self.blocks.contains_key(&block_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::SeqIdGen;

    #[test]
    fn empty_document_has_root() {
        let mut gen = SeqIdGen::default();
        let doc = Document::empty(
            DocumentId::new(&mut gen),
            SectionId::new(&mut gen),
            ActorId::new(&mut gen),
            Timestamp { unix_ms: 0 },
        );
        assert_eq!(doc.sections.len(), 1);
        assert!(doc.blocks.is_empty());
    }
}
