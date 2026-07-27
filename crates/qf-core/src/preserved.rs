//! Unknown-field preservation for forward-compatible metadata.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Map that retains unknown keys/values across decode/encode cycles.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreservedMap {
    pub entries: BTreeMap<String, PreservedValue>,
}

impl PreservedMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, key: impl Into<String>, value: PreservedValue) {
        self.entries.insert(key.into(), value);
    }

    pub fn get(&self, key: &str) -> Option<&PreservedValue> {
        self.entries.get(key)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreservedValue {
    Null,
    Bool(bool),
    Integer(i64),
    Text(String),
    Bytes(Vec<u8>),
    Array(Vec<PreservedValue>),
    Map(PreservedMap),
    /// Opaque CBOR (or other) bytes that must round-trip verbatim.
    RawCbor(Vec<u8>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserved_map_order_stable() {
        let mut m = PreservedMap::new();
        m.insert("b", PreservedValue::Integer(2));
        m.insert("a", PreservedValue::Integer(1));
        let keys: Vec<_> = m.entries.keys().cloned().collect();
        assert_eq!(keys, vec!["a".to_string(), "b".to_string()]);
    }
}
