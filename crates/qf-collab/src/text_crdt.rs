//! RGA-style text CRDT for offline concurrent editing.
//!
//! Characters are identified by `(replica, counter)`. Concurrent inserts after the
//! same anchor are ordered deterministically by `CharId`. Deletes are tombstones.
//!
//! Merge is a pure union of inserts + deletes → same materialization on all replicas.
//! Kernel application still goes through `Operation`s produced by [`TextCrdt::to_replace_ops`].

use qf_core::{BlockId, OffsetRange, Operation, TextOffset};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Globally unique character identity (replica-local counter).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CharId {
    pub replica: u64,
    pub counter: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Insert {
    id: CharId,
    /// Insert immediately after this id (`None` = beginning).
    after: Option<CharId>,
    ch: char,
}

/// Text CRDT document (one logical string, typically one block's text).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TextCrdt {
    pub replica: u64,
    next_counter: u64,
    inserts: BTreeMap<CharId, Insert>,
    deletes: BTreeSet<CharId>,
}

impl TextCrdt {
    pub fn new(replica: u64) -> Self {
        Self {
            replica,
            next_counter: 1,
            inserts: BTreeMap::new(),
            deletes: BTreeSet::new(),
        }
    }

    /// Clone document state under a new replica id (for multi-replica tests / sessions).
    pub fn fork_replica(&self, replica: u64) -> Self {
        Self {
            replica,
            next_counter: 1,
            inserts: self.inserts.clone(),
            deletes: self.deletes.clone(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.materialize().is_empty()
    }

    fn alloc_id(&mut self) -> CharId {
        let id = CharId {
            replica: self.replica,
            counter: self.next_counter,
        };
        self.next_counter = self.next_counter.saturating_add(1);
        id
    }

    /// Visible length (non-tombstoned chars).
    pub fn len_chars(&self) -> usize {
        self.ordered_ids().len()
    }

    /// Insert `text` at a visible UTF scalar index.
    pub fn local_insert(&mut self, index: usize, text: &str) {
        let order = self.ordered_ids();
        let mut after = if index == 0 {
            None
        } else {
            order.get(index - 1).copied()
        };
        for ch in text.chars() {
            let id = self.alloc_id();
            self.inserts.insert(
                id,
                Insert {
                    id,
                    after,
                    ch,
                },
            );
            after = Some(id);
        }
    }

    /// Delete `count` visible characters starting at `index`.
    pub fn local_delete(&mut self, index: usize, count: usize) {
        let order = self.ordered_ids();
        for id in order.into_iter().skip(index).take(count) {
            self.deletes.insert(id);
        }
    }

    /// Merge another replica's state (commutative, associative, idempotent).
    pub fn merge(&mut self, other: &TextCrdt) {
        for (id, ins) in &other.inserts {
            self.inserts.entry(*id).or_insert_with(|| ins.clone());
        }
        self.deletes.extend(other.deletes.iter().copied());
        // Keep counters high enough not to reuse ids if same replica (shouldn't merge self).
        if other.replica == self.replica {
            self.next_counter = self.next_counter.max(other.next_counter);
        }
    }

    /// Materialize visible string deterministically.
    pub fn materialize(&self) -> String {
        let mut out = String::new();
        for id in self.ordered_ids() {
            if let Some(ins) = self.inserts.get(&id) {
                out.push(ins.ch);
            }
        }
        out
    }

    /// Visible CharIds in document order.
    fn ordered_ids(&self) -> Vec<CharId> {
        // Children of each anchor, sorted by CharId for concurrent siblings.
        let mut children: BTreeMap<Option<CharId>, Vec<CharId>> = BTreeMap::new();
        for ins in self.inserts.values() {
            if self.deletes.contains(&ins.id) {
                continue;
            }
            children.entry(ins.after).or_default().push(ins.id);
        }
        for v in children.values_mut() {
            v.sort();
        }
        let mut out = Vec::new();
        fn walk(
            after: Option<CharId>,
            children: &BTreeMap<Option<CharId>, Vec<CharId>>,
            out: &mut Vec<CharId>,
        ) {
            let Some(kids) = children.get(&after) else {
                return;
            };
            for id in kids {
                out.push(*id);
                walk(Some(*id), children, out);
            }
        }
        walk(None, &children, &mut out);
        out
    }

    /// Produce kernel ops that set `block_id` text to this CRDT's materialization
    /// assuming the block currently holds `current` (single-span paragraph model).
    pub fn to_replace_ops(&self, block_id: BlockId, current: &str) -> Vec<Operation> {
        let target = self.materialize();
        if target == current {
            return vec![];
        }
        let mut ops = Vec::new();
        if !current.is_empty() {
            ops.push(Operation::DeleteText {
                block_id,
                range: OffsetRange {
                    start: TextOffset(0),
                    end: TextOffset(current.chars().count() as u32),
                },
            });
        }
        if !target.is_empty() {
            ops.push(Operation::InsertText {
                block_id,
                at: TextOffset(0),
                text: target,
            });
        }
        ops
    }
}

/// Merge two CRDT states into a new state (does not mutate inputs).
pub fn merge_crdt(a: &TextCrdt, b: &TextCrdt) -> TextCrdt {
    let mut out = a.clone();
    out.merge(b);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concurrent_inserts_converge() {
        let mut a = TextCrdt::new(1);
        a.local_insert(0, "Hello");
        let mut b = a.fork_replica(2);
        // Concurrent insert after shared "Hello"
        a.local_insert(5, "A");
        b.local_insert(5, "B");
        let mut a2 = a.clone();
        let mut b2 = b.clone();
        a2.merge(&b);
        b2.merge(&a);
        assert_eq!(a2.materialize(), b2.materialize());
        let s = a2.materialize();
        assert!(s.starts_with("Hello"));
        assert_eq!(s.chars().count(), "Hello".chars().count() + 2);
    }

    #[test]
    fn merge_idempotent_and_commutative() {
        let mut a = TextCrdt::new(1);
        let mut b = TextCrdt::new(2);
        a.local_insert(0, "x");
        b.local_insert(0, "y");
        let ab = merge_crdt(&a, &b);
        let ba = merge_crdt(&b, &a);
        assert_eq!(ab.materialize(), ba.materialize());
        let mut ab2 = ab.clone();
        ab2.merge(&ab);
        assert_eq!(ab2.materialize(), ab.materialize());
    }

    #[test]
    fn delete_and_concurrent_insert_converge() {
        let mut a = TextCrdt::new(1);
        a.local_insert(0, "abcd");
        let mut b = a.fork_replica(2);
        a.local_delete(1, 2); // remove bc → ad
        b.local_insert(2, "X"); // abXcd
        let m = merge_crdt(&a, &b);
        let m2 = merge_crdt(&b, &a);
        assert_eq!(m.materialize(), m2.materialize());
    }

    #[test]
    fn to_replace_ops_roundtrip_string() {
        let mut crdt = TextCrdt::new(1);
        crdt.local_insert(0, "hi");
        let bid = BlockId(uuid::Uuid::from_u128(1));
        let ops = crdt.to_replace_ops(bid, "");
        assert_eq!(ops.len(), 1);
        match &ops[0] {
            Operation::InsertText { text, .. } => assert_eq!(text, "hi"),
            _ => panic!("expected insert"),
        }
    }
}
