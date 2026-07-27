//! Custom block type registry owned by the host.

use std::collections::BTreeMap;

#[derive(Clone, Debug, Default)]
pub struct TypeRegistry {
    /// type_name → (plugin_id, schema bytes)
    types: BTreeMap<String, (String, Vec<u8>)>,
}

impl TypeRegistry {
    pub fn register(
        &mut self,
        plugin_id: &str,
        type_name: impl Into<String>,
        schema: Vec<u8>,
    ) -> Result<(), String> {
        let type_name = type_name.into();
        if type_name.is_empty() || !type_name.contains('.') {
            return Err("block type must be reverse-DNS style".into());
        }
        if let Some((owner, _)) = self.types.get(&type_name) {
            if owner != plugin_id {
                return Err(format!("type {type_name} owned by {owner}"));
            }
        }
        self.types
            .insert(type_name, (plugin_id.to_string(), schema));
        Ok(())
    }

    pub fn unregister_plugin(&mut self, plugin_id: &str) {
        self.types.retain(|_, (owner, _)| owner != plugin_id);
    }

    pub fn is_registered(&self, type_name: &str) -> bool {
        self.types.contains_key(type_name)
    }

    pub fn schema(&self, type_name: &str) -> Option<&[u8]> {
        self.types.get(type_name).map(|(_, s)| s.as_slice())
    }

    pub fn owner(&self, type_name: &str) -> Option<&str> {
        self.types.get(type_name).map(|(o, _)| o.as_str())
    }

    pub fn len(&self) -> usize {
        self.types.len()
    }

    pub fn is_empty(&self) -> bool {
        self.types.is_empty()
    }
}
