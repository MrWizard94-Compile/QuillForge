//! Capability-based plugin sandbox host.
//!
//! **v1 isolation:** guests are treated as untrusted pure handlers invoked by the host.
//! Native dynamic libraries are forbidden. Raw filesystem and journal access are impossible
//! through this API surface. Future builds may execute the same hostcalls from Wasmtime.
//!
//! Plugins MUST NOT mutate canonical state. They may only:
//! - register types / hooks under capability grants
//! - propose operations (still require command/journal commit outside this crate)
//! - use namespaced private KV within quotas

mod caps;
mod host;
mod kv;
mod manifest;
mod registry;

pub use caps::*;
pub use host::*;
pub use kv::*;
pub use manifest::*;
pub use registry::*;

/// Hard policy: third-party native plugins are not loadable in v1.
pub fn native_dynamic_plugins_allowed() -> bool {
    false
}

/// Hard policy: no raw filesystem paths for plugins.
pub fn raw_filesystem_access_allowed() -> bool {
    false
}

/// Hard policy: plugins never receive journal handles.
pub fn journal_access_allowed() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use qf_core::{Capability, PluginId};

    fn sample_manifest(required: &[&str]) -> PluginManifest {
        PluginManifest {
            id: PluginId("com.example.math".into()),
            name: "Math".into(),
            version: "1.0.0".into(),
            qf_api_version: "1.0".into(),
            entry_wasm: Some("plugin.wasm".into()),
            author: Some("Example".into()),
            required_capabilities: required.iter().map(|s| Capability((*s).into())).collect(),
            optional_capabilities: vec![],
            declared_block_types: vec!["com.example.math/eq".into()],
        }
    }

    #[test]
    fn load_requires_capabilities() {
        let mut host = PluginHost::new();
        let err = host
            .load(sample_manifest(&[CAP_BLOCK_CUSTOM_REGISTER]), [])
            .unwrap_err();
        assert!(matches!(err, PluginError::Denied(_)));
    }

    #[test]
    fn load_and_register_block_type() {
        let mut host = PluginHost::new();
        let id = host
            .load(
                sample_manifest(&[CAP_BLOCK_CUSTOM_REGISTER]),
                [Capability(CAP_BLOCK_CUSTOM_REGISTER.into())],
            )
            .unwrap();
        host.register_block_type(&id, "com.example.math/eq", b"schema")
            .unwrap();
        assert!(host.type_registry().is_registered("com.example.math/eq"));
        host.unload(&id).unwrap();
        assert!(!host.type_registry().is_registered("com.example.math/eq"));
    }

    #[test]
    fn propose_ops_requires_capability() {
        let mut host = PluginHost::new();
        let id = host.load(sample_manifest(&[]), []).unwrap();
        let err = host
            .propose_operations(
                &id,
                vec![ProposedOp {
                    kind: "insert_block".into(),
                    payload: vec![],
                }],
            )
            .unwrap_err();
        assert!(matches!(err, PluginError::Denied(_)));
    }

    #[test]
    fn propose_ops_queues_for_command_path() {
        let mut host = PluginHost::new();
        let id = host
            .load(
                sample_manifest(&[CAP_DOCUMENT_PROPOSE_OPS]),
                [Capability(CAP_DOCUMENT_PROPOSE_OPS.into())],
            )
            .unwrap();
        let pid = host
            .propose_operations(
                &id,
                vec![ProposedOp {
                    kind: "noop".into(),
                    payload: b"x".to_vec(),
                }],
            )
            .unwrap();
        let proposal = host.take_proposal(&pid).unwrap();
        assert_eq!(proposal.plugin_id, id);
        assert_eq!(proposal.ops.len(), 1);
        // Second take is empty
        assert!(host.take_proposal(&pid).is_none());
    }

    #[test]
    fn kv_quota_and_namespace() {
        let mut host = PluginHost::new();
        let id = host.load(sample_manifest(&[]), []).unwrap();
        host.kv_set(&id, b"k", b"v").unwrap();
        assert_eq!(host.kv_get(&id, b"k").unwrap().as_deref(), Some(&b"v"[..]));
        let other = host
            .load(
                PluginManifest {
                    id: PluginId("com.other".into()),
                    name: "o".into(),
                    version: "1".into(),
                    qf_api_version: "1.0".into(),
                    entry_wasm: None,
                    author: None,
                    required_capabilities: vec![],
                    optional_capabilities: vec![],
                    declared_block_types: vec![],
                },
                [],
            )
            .unwrap();
        assert!(host.kv_get(&other, b"k").unwrap().is_none());
    }

    #[test]
    fn fuel_exhaustion_traps() {
        let mut host = PluginHost::new();
        let id = host.load(sample_manifest(&[]), []).unwrap();
        host.set_fuel_budget(0);
        let err = host.dispatch_hook(&id, Hook::OnActivate).unwrap_err();
        assert!(matches!(err, PluginError::Trap(_)));
    }

    #[test]
    fn hard_policies() {
        assert!(!native_dynamic_plugins_allowed());
        assert!(!raw_filesystem_access_allowed());
        assert!(!journal_access_allowed());
    }

    #[test]
    fn export_hook_capability() {
        let mut host = PluginHost::new();
        let id = host
            .load(
                sample_manifest(&[CAP_EXPORT_TRANSFORM]),
                [Capability(CAP_EXPORT_TRANSFORM.into())],
            )
            .unwrap();
        host.dispatch_hook(
            &id,
            Hook::ExportTransform {
                format: "markdown".into(),
            },
        )
        .unwrap();
    }

    #[test]
    fn unknown_block_roundtrip_via_decode_encode() {
        let mut host = PluginHost::new();
        let id = host
            .load(
                sample_manifest(&[CAP_BLOCK_CUSTOM_REGISTER]),
                [Capability(CAP_BLOCK_CUSTOM_REGISTER.into())],
            )
            .unwrap();
        host.register_block_type(&id, "com.example.math/eq", b"{}")
            .unwrap();
        let payload = vec![9, 8, 7];
        let decoded = host
            .dispatch_hook(
                &id,
                Hook::DecodeBlock {
                    type_name: "com.example.math/eq".into(),
                    payload: payload.clone(),
                },
            )
            .unwrap();
        match decoded {
            HookResult::Bytes(b) => assert_eq!(b, payload),
            _ => panic!("expected bytes"),
        }
    }
}
