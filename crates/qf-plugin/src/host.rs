//! Plugin host: load/unload, capabilities, hooks, proposals.

use crate::caps::*;
use crate::kv::{PluginKv, DEFAULT_KV_QUOTA_BYTES};
use crate::manifest::PluginManifest;
use crate::registry::TypeRegistry;
use qf_core::{Capability, PluginId};
use std::collections::{HashMap, HashSet};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PluginError {
    #[error("denied capability: {0}")]
    Denied(String),
    #[error("not found")]
    NotFound,
    #[error("trap: {0}")]
    Trap(String),
    #[error("quota: {0}")]
    Quota(String),
    #[error("{0}")]
    Other(String),
}

#[derive(Clone, Debug)]
pub enum Hook {
    OnActivate,
    OnDeactivate,
    DecodeBlock { type_name: String, payload: Vec<u8> },
    EncodeBlock { type_name: String, payload: Vec<u8> },
    ExportTransform { format: String },
    SearchContribute,
    ThemeTokens,
}

#[derive(Clone, Debug)]
pub enum HookResult {
    Ok,
    Bytes(Vec<u8>),
}

/// Opaque operation proposal from a plugin (host validates; session commits).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposedOp {
    pub kind: String,
    pub payload: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ProposalId(pub Uuid);

#[derive(Clone, Debug)]
pub struct Proposal {
    pub id: ProposalId,
    pub plugin_id: PluginId,
    pub ops: Vec<ProposedOp>,
}

struct LoadedPlugin {
    manifest: PluginManifest,
    granted: HashSet<String>,
    active: bool,
}

/// Capability-enforcing plugin host.
pub struct PluginHost {
    plugins: HashMap<String, LoadedPlugin>,
    registry: TypeRegistry,
    kv: PluginKv,
    proposals: HashMap<Uuid, Proposal>,
    /// Simulated fuel for hostcall batches (0 = exhausted).
    fuel: u64,
    fuel_budget: u64,
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginHost {
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
            registry: TypeRegistry::default(),
            kv: PluginKv::new(DEFAULT_KV_QUOTA_BYTES),
            proposals: HashMap::new(),
            fuel: u64::MAX / 4,
            fuel_budget: u64::MAX / 4,
        }
    }

    pub fn set_fuel_budget(&mut self, budget: u64) {
        self.fuel_budget = budget;
        self.fuel = budget;
    }

    pub fn type_registry(&self) -> &TypeRegistry {
        &self.registry
    }

    pub fn load(
        &mut self,
        manifest: PluginManifest,
        granted: impl IntoIterator<Item = Capability>,
    ) -> Result<PluginId, PluginError> {
        self.burn_fuel(1)?;
        if crate::native_dynamic_plugins_allowed() {
            // Unreachable under policy; keep explicit.
        }
        let granted: HashSet<String> = granted.into_iter().map(|c| c.0).collect();
        for req in &manifest.required_capabilities {
            if !granted.contains(&req.0) {
                return Err(PluginError::Denied(req.0.clone()));
            }
        }
        // Only grant intersection of requested+optional with user grants.
        let mut effective = HashSet::new();
        for c in manifest
            .required_capabilities
            .iter()
            .chain(manifest.optional_capabilities.iter())
        {
            if granted.contains(&c.0) {
                effective.insert(c.0.clone());
            }
        }
        // Also allow explicitly granted caps even if not listed optional (host discretion).
        for g in &granted {
            effective.insert(g.clone());
        }

        let id = manifest.id.clone();
        if self.plugins.contains_key(&id.0) {
            return Err(PluginError::Other(format!("already loaded: {}", id.0)));
        }
        self.plugins.insert(
            id.0.clone(),
            LoadedPlugin {
                manifest,
                granted: effective,
                active: true,
            },
        );
        Ok(id)
    }

    pub fn unload(&mut self, id: &PluginId) -> Result<(), PluginError> {
        self.burn_fuel(1)?;
        let removed = self.plugins.remove(&id.0).ok_or(PluginError::NotFound)?;
        let _ = removed;
        self.registry.unregister_plugin(&id.0);
        self.kv.clear_plugin(&id.0);
        // Drop pending proposals from this plugin
        self.proposals.retain(|_, p| p.plugin_id.0 != id.0);
        Ok(())
    }

    pub fn check_capability(&self, id: &PluginId, cap: &str) -> Result<(), PluginError> {
        let p = self.plugins.get(&id.0).ok_or(PluginError::NotFound)?;
        if p.granted.contains(cap) {
            Ok(())
        } else {
            Err(PluginError::Denied(cap.into()))
        }
    }

    pub fn register_block_type(
        &mut self,
        id: &PluginId,
        type_name: &str,
        schema: &[u8],
    ) -> Result<(), PluginError> {
        self.burn_fuel(1)?;
        self.check_capability(id, CAP_BLOCK_CUSTOM_REGISTER)?;
        self.registry
            .register(&id.0, type_name, schema.to_vec())
            .map_err(PluginError::Other)
    }

    pub fn propose_operations(
        &mut self,
        id: &PluginId,
        ops: Vec<ProposedOp>,
    ) -> Result<ProposalId, PluginError> {
        self.burn_fuel(1)?;
        self.check_capability(id, CAP_DOCUMENT_PROPOSE_OPS)?;
        if ops.is_empty() {
            return Err(PluginError::Other("empty proposal".into()));
        }
        if ops.len() > 10_000 {
            return Err(PluginError::Quota("too many proposed ops".into()));
        }
        let pid = Uuid::new_v4();
        let proposal = Proposal {
            id: ProposalId(pid),
            plugin_id: id.clone(),
            ops,
        };
        self.proposals.insert(pid, proposal);
        Ok(ProposalId(pid))
    }

    /// Take a proposal for command-path application (single-use).
    pub fn take_proposal(&mut self, id: &ProposalId) -> Option<Proposal> {
        self.proposals.remove(&id.0)
    }

    pub fn kv_get(&mut self, id: &PluginId, key: &[u8]) -> Result<Option<Vec<u8>>, PluginError> {
        self.burn_fuel(1)?;
        if !self.plugins.contains_key(&id.0) {
            return Err(PluginError::NotFound);
        }
        Ok(self.kv.get(&id.0, key))
    }

    pub fn kv_set(&mut self, id: &PluginId, key: &[u8], val: &[u8]) -> Result<(), PluginError> {
        self.burn_fuel(1)?;
        if !self.plugins.contains_key(&id.0) {
            return Err(PluginError::NotFound);
        }
        self.kv.set(&id.0, key, val).map_err(PluginError::Quota)
    }

    pub fn dispatch_hook(&mut self, id: &PluginId, hook: Hook) -> Result<HookResult, PluginError> {
        self.burn_fuel(1)?;
        let p = self.plugins.get(&id.0).ok_or(PluginError::NotFound)?;
        if !p.active {
            return Err(PluginError::Trap("plugin inactive".into()));
        }
        match hook {
            Hook::OnActivate | Hook::OnDeactivate => Ok(HookResult::Ok),
            Hook::DecodeBlock { type_name, payload } => {
                if !self.registry.is_registered(&type_name)
                    && !p
                        .manifest
                        .declared_block_types
                        .iter()
                        .any(|t| t == &type_name)
                {
                    // Still return payload identity for unknown handling
                }
                Ok(HookResult::Bytes(payload))
            }
            Hook::EncodeBlock { payload, .. } => Ok(HookResult::Bytes(payload)),
            Hook::ExportTransform { .. } => {
                self.check_capability(id, CAP_EXPORT_TRANSFORM)?;
                Ok(HookResult::Ok)
            }
            Hook::SearchContribute => {
                self.check_capability(id, CAP_SEARCH_PROVIDER)?;
                Ok(HookResult::Ok)
            }
            Hook::ThemeTokens => {
                self.check_capability(id, CAP_THEME_PROVIDER)?;
                Ok(HookResult::Ok)
            }
        }
    }

    /// Convert a proposal's ops into journalable Operations (safe subset).
    pub fn materialize_proposal_ops(
        proposal: &Proposal,
    ) -> Result<Vec<qf_core::Operation>, PluginError> {
        use qf_core::{BlockBodyBytes, BlockId, Operation, ParentRef};
        let mut out = Vec::new();
        for op in &proposal.ops {
            match op.kind.as_str() {
                "noop" => out.push(Operation::Noop {
                    reason: qf_core::NoopReason::Other("plugin".into()),
                }),
                "insert_block" => {
                    // payload: type_name\0payload_bytes
                    let (type_name, body) = split_type_payload(&op.payload)?;
                    // Block id must be assigned by session IdGen — caller rewrites.
                    out.push(Operation::InsertBlock {
                        block_id: BlockId(uuid::Uuid::nil()),
                        parent: ParentRef::DocumentRoot,
                        index: 0,
                        body: BlockBodyBytes {
                            type_name,
                            payload: body,
                        },
                    });
                }
                other => {
                    return Err(PluginError::Other(format!(
                        "unsupported proposed op kind: {other}"
                    )));
                }
            }
        }
        Ok(out)
    }

    fn burn_fuel(&mut self, n: u64) -> Result<(), PluginError> {
        if self.fuel < n {
            return Err(PluginError::Trap("fuel exhausted".into()));
        }
        self.fuel -= n;
        Ok(())
    }

    /// Public fuel burn for Wasmtime hostcall bridge.
    pub fn burn_fuel_public(&mut self, n: u64) -> Result<(), PluginError> {
        self.burn_fuel(n)
    }

    pub fn reset_fuel(&mut self) {
        self.fuel = self.fuel_budget;
    }
}

fn split_type_payload(payload: &[u8]) -> Result<(String, Vec<u8>), PluginError> {
    let pos = payload
        .iter()
        .position(|b| *b == 0)
        .ok_or_else(|| PluginError::Other("insert_block payload needs type\\0body".into()))?;
    let type_name = String::from_utf8(payload[..pos].to_vec())
        .map_err(|e| PluginError::Other(e.to_string()))?;
    Ok((type_name, payload[pos + 1..].to_vec()))
}
