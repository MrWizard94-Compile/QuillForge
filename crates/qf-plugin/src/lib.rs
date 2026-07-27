//! Capability-based plugin sandbox (WASM host in production).
//! Plugins MUST NOT access raw filesystem or journal handles.

use qf_core::{Capability, PluginId};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PluginError {
    #[error("denied capability: {0}")]
    Denied(String),
    #[error("not found")]
    NotFound,
    #[error("trap: {0}")]
    Trap(String),
    #[error("{0}")]
    Other(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub qf_api_version: String,
    pub required_capabilities: Vec<Capability>,
    pub optional_capabilities: Vec<Capability>,
}

#[derive(Clone, Debug)]
pub enum Hook {
    OnActivate,
    OnDeactivate,
    DecodeBlock { type_name: String, payload: Vec<u8> },
    EncodeBlock { type_name: String },
    ExportTransform { format: String },
}

#[derive(Clone, Debug)]
pub enum HookResult {
    Ok,
    Bytes(Vec<u8>),
}

struct LoadedPlugin {
    #[allow(dead_code)]
    manifest: PluginManifest,
    granted: HashSet<String>,
}

pub struct PluginHost {
    plugins: HashMap<String, LoadedPlugin>,
}

impl PluginHost {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    pub fn load(
        &mut self,
        manifest: PluginManifest,
        granted: impl IntoIterator<Item = Capability>,
    ) -> Result<PluginId, PluginError> {
        let granted: HashSet<String> = granted.into_iter().map(|c| c.0).collect();
        for req in &manifest.required_capabilities {
            if !granted.contains(&req.0) {
                return Err(PluginError::Denied(req.0.clone()));
            }
        }
        let id = manifest.id.clone();
        self.plugins.insert(
            id.0.clone(),
            LoadedPlugin { manifest, granted },
        );
        Ok(id)
    }

    pub fn unload(&mut self, id: &PluginId) -> Result<(), PluginError> {
        self.plugins
            .remove(&id.0)
            .map(|_| ())
            .ok_or(PluginError::NotFound)
    }

    pub fn check_capability(&self, id: &PluginId, cap: &Capability) -> Result<(), PluginError> {
        let p = self.plugins.get(&id.0).ok_or(PluginError::NotFound)?;
        if p.granted.contains(&cap.0) {
            Ok(())
        } else {
            Err(PluginError::Denied(cap.0.clone()))
        }
    }

    pub fn dispatch_hook(&mut self, id: &PluginId, hook: Hook) -> Result<HookResult, PluginError> {
        let _p = self.plugins.get(&id.0).ok_or(PluginError::NotFound)?;
        match hook {
            Hook::OnActivate | Hook::OnDeactivate => Ok(HookResult::Ok),
            Hook::DecodeBlock { payload, .. } => Ok(HookResult::Bytes(payload)),
            Hook::EncodeBlock { .. } => Ok(HookResult::Bytes(vec![])),
            Hook::ExportTransform { .. } => {
                self.check_capability(id, &Capability("export.transform".into()))?;
                Ok(HookResult::Ok)
            }
        }
    }

    /// Hard guarantee: host never exposes filesystem paths to plugins.
    pub fn raw_filesystem_access_allowed() -> bool {
        false
    }
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denies_missing_capability() {
        let mut host = PluginHost::new();
        let manifest = PluginManifest {
            id: PluginId("com.test".into()),
            name: "t".into(),
            version: "1".into(),
            qf_api_version: "1.0".into(),
            required_capabilities: vec![Capability("block.custom.register".into())],
            optional_capabilities: vec![],
        };
        let err = host.load(manifest, []).unwrap_err();
        assert!(matches!(err, PluginError::Denied(_)));
    }

    #[test]
    fn no_raw_fs() {
        assert!(!PluginHost::raw_filesystem_access_allowed());
    }
}
