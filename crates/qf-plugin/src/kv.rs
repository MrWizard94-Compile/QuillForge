//! Plugin-private key-value store (namespaced, quota-limited).

use std::collections::HashMap;

/// Default per-plugin storage quota (bytes of values).
pub const DEFAULT_KV_QUOTA_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Default)]
pub struct PluginKv {
    /// plugin_id → (key → value)
    data: HashMap<String, HashMap<Vec<u8>, Vec<u8>>>,
    /// plugin_id → total value bytes
    usage: HashMap<String, usize>,
    pub quota_bytes: usize,
}

impl PluginKv {
    pub fn new(quota_bytes: usize) -> Self {
        Self {
            data: HashMap::new(),
            usage: HashMap::new(),
            quota_bytes,
        }
    }

    pub fn get(&self, plugin_id: &str, key: &[u8]) -> Option<Vec<u8>> {
        self.data.get(plugin_id)?.get(key).cloned()
    }

    pub fn set(&mut self, plugin_id: &str, key: &[u8], val: &[u8]) -> Result<(), String> {
        let map = self.data.entry(plugin_id.to_string()).or_default();
        let old_len = map.get(key).map(|v| v.len()).unwrap_or(0);
        let usage = self.usage.entry(plugin_id.to_string()).or_insert(0);
        let new_usage = usage.saturating_sub(old_len).saturating_add(val.len());
        if new_usage > self.quota_bytes {
            return Err("plugin kv quota exceeded".into());
        }
        map.insert(key.to_vec(), val.to_vec());
        *usage = new_usage;
        Ok(())
    }

    pub fn clear_plugin(&mut self, plugin_id: &str) {
        self.data.remove(plugin_id);
        self.usage.remove(plugin_id);
    }
}
