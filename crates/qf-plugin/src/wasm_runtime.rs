//! Wasmtime guest runtime for capability-gated plugin hostcalls.
//!
//! Guests never see journal, filesystem, or network handles. All mutation
//! proposals flow through [`PluginHost`] which still requires the command path.

use crate::host::{PluginError, PluginHost, ProposedOp};
use crate::caps::{CAP_BLOCK_CUSTOM_REGISTER, CAP_DOCUMENT_PROPOSE_OPS};
use qf_core::PluginId;
use std::cell::RefCell;
use std::rc::Rc;
use wasmtime::{Caller, Config, Engine, Linker, Module, Store};

/// Hostcall opcodes (i32) for the multiplexed `qf.host_call` import.
pub mod opcode {
    pub const LOG: i32 = 1;
    pub const KV_GET: i32 = 2;
    pub const KV_SET: i32 = 3;
    pub const REGISTER_BLOCK_TYPE: i32 = 4;
    pub const PROPOSE_NOOP: i32 = 5;
    pub const PROPOSE_INSERT_BLOCK: i32 = 6;
}

/// Shared bridge so hostcalls can re-enter [`PluginHost`] during guest execution.
struct Bridge {
    host: Rc<RefCell<PluginHost>>,
    plugin_id: PluginId,
    /// Scratch buffer for `kv_get` results (guest reads via `qf.read_scratch`).
    scratch: Vec<u8>,
    last_log: Vec<String>,
}

/// Compiled Wasmtime engine with fuel enabled.
pub struct WasmEngine {
    engine: Engine,
}

impl Default for WasmEngine {
    fn default() -> Self {
        Self::new().expect("wasmtime engine")
    }
}

impl WasmEngine {
    pub fn new() -> Result<Self, PluginError> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);
        // Deterministic-ish: no epoch, no threads, no NaN canonicalization extras needed for v1.
        let engine = Engine::new(&config).map_err(|e| PluginError::Trap(e.to_string()))?;
        Ok(Self { engine })
    }

    /// Instantiate guest wasm, inject fuel, call exported `activate` → i32 status.
    ///
    /// Expected guest imports (module `qf`):
    /// - `host_call(op, ptr_a, len_a, ptr_b, len_b) -> i32`  (0 ok, negative error)
    /// - `scratch_len() -> i32`
    /// - `scratch_copy(dst_ptr, max_len) -> i32`  (bytes written)
    ///
    /// Guest must export `memory` and `activate`.
    pub fn run_activate(
        &self,
        host: Rc<RefCell<PluginHost>>,
        plugin_id: PluginId,
        wasm_bytes: &[u8],
        fuel: u64,
    ) -> Result<GuestRunResult, PluginError> {
        let module =
            Module::new(&self.engine, wasm_bytes).map_err(|e| PluginError::Trap(e.to_string()))?;

        let bridge = Bridge {
            host: host.clone(),
            plugin_id: plugin_id.clone(),
            scratch: Vec::new(),
            last_log: Vec::new(),
        };
        let mut store = Store::new(&self.engine, bridge);
        store
            .set_fuel(fuel)
            .map_err(|e| PluginError::Trap(e.to_string()))?;

        let mut linker = Linker::new(&self.engine);
        linker
            .func_wrap(
                "qf",
                "host_call",
                |mut caller: Caller<'_, Bridge>,
                 op: i32,
                 ptr_a: i32,
                 len_a: i32,
                 ptr_b: i32,
                 len_b: i32|
                 -> i32 {
                    match host_call(&mut caller, op, ptr_a, len_a, ptr_b, len_b) {
                        Ok(code) => code,
                        Err(e) => {
                            let _ = e;
                            -1
                        }
                    }
                },
            )
            .map_err(|e| PluginError::Trap(e.to_string()))?;

        linker
            .func_wrap("qf", "scratch_len", |caller: Caller<'_, Bridge>| -> i32 {
                caller.data().scratch.len() as i32
            })
            .map_err(|e| PluginError::Trap(e.to_string()))?;

        linker
            .func_wrap(
                "qf",
                "scratch_copy",
                |mut caller: Caller<'_, Bridge>, dst: i32, max_len: i32| -> i32 {
                    let data = caller.data().scratch.clone();
                    let n = data.len().min(max_len.max(0) as usize);
                    if n == 0 {
                        return 0;
                    }
                    let Some(mem) = caller.get_export("memory").and_then(|e| e.into_memory()) else {
                        return -1;
                    };
                    if mem.write(&mut caller, dst as usize, &data[..n]).is_err() {
                        return -1;
                    }
                    n as i32
                },
            )
            .map_err(|e| PluginError::Trap(e.to_string()))?;

        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(map_trap)?;

        let activate = instance
            .get_typed_func::<(), i32>(&mut store, "activate")
            .map_err(|e| PluginError::Trap(format!("missing activate export: {e}")))?;

        let status = activate.call(&mut store, ()).map_err(map_trap)?;

        let remaining = store
            .get_fuel()
            .map_err(|e| PluginError::Trap(e.to_string()))?;
        let logs = store.data().last_log.clone();
        Ok(GuestRunResult {
            status,
            fuel_remaining: remaining,
            logs,
        })
    }
}

/// Result of a successful guest activation (trap becomes Err).
#[derive(Clone, Debug)]
pub struct GuestRunResult {
    pub status: i32,
    pub fuel_remaining: u64,
    pub logs: Vec<String>,
}

fn map_trap(err: wasmtime::Error) -> PluginError {
    let s = err.to_string();
    if s.contains("fuel") || s.contains("all fuel") {
        PluginError::Trap("fuel exhausted".into())
    } else {
        PluginError::Trap(s)
    }
}

fn read_guest_bytes(
    caller: &mut Caller<'_, Bridge>,
    ptr: i32,
    len: i32,
) -> Result<Vec<u8>, PluginError> {
    if len < 0 || ptr < 0 {
        return Err(PluginError::Trap("invalid guest pointer".into()));
    }
    let mem = caller
        .get_export("memory")
        .and_then(|e| e.into_memory())
        .ok_or_else(|| PluginError::Trap("guest memory missing".into()))?;
    let mut buf = vec![0u8; len as usize];
    mem.read(&mut *caller, ptr as usize, &mut buf)
        .map_err(|e| PluginError::Trap(e.to_string()))?;
    Ok(buf)
}

fn host_call(
    caller: &mut Caller<'_, Bridge>,
    op: i32,
    ptr_a: i32,
    len_a: i32,
    ptr_b: i32,
    len_b: i32,
) -> Result<i32, PluginError> {
    // Charge host-side fuel for every hostcall as well.
    {
        let mut host = caller.data().host.borrow_mut();
        host.burn_fuel_public(1)?;
    }

    match op {
        opcode::LOG => {
            let bytes = read_guest_bytes(caller, ptr_a, len_a)?;
            let msg = String::from_utf8_lossy(&bytes).into_owned();
            caller.data_mut().last_log.push(msg);
            Ok(0)
        }
        opcode::KV_GET => {
            let key = read_guest_bytes(caller, ptr_a, len_a)?;
            let plugin_id = caller.data().plugin_id.clone();
            let val = {
                let mut host = caller.data().host.borrow_mut();
                host.kv_get(&plugin_id, &key)?
            };
            caller.data_mut().scratch = val.unwrap_or_default();
            Ok(if caller.data().scratch.is_empty() {
                0
            } else {
                1
            })
        }
        opcode::KV_SET => {
            let key = read_guest_bytes(caller, ptr_a, len_a)?;
            let val = read_guest_bytes(caller, ptr_b, len_b)?;
            let plugin_id = caller.data().plugin_id.clone();
            let mut host = caller.data().host.borrow_mut();
            host.kv_set(&plugin_id, &key, &val)?;
            Ok(0)
        }
        opcode::REGISTER_BLOCK_TYPE => {
            let type_name_bytes = read_guest_bytes(caller, ptr_a, len_a)?;
            let schema = read_guest_bytes(caller, ptr_b, len_b)?;
            let type_name = String::from_utf8(type_name_bytes)
                .map_err(|e| PluginError::Other(e.to_string()))?;
            let plugin_id = caller.data().plugin_id.clone();
            // Capability checked inside register_block_type.
            let _ = CAP_BLOCK_CUSTOM_REGISTER;
            let mut host = caller.data().host.borrow_mut();
            host.register_block_type(&plugin_id, &type_name, &schema)?;
            Ok(0)
        }
        opcode::PROPOSE_NOOP => {
            let plugin_id = caller.data().plugin_id.clone();
            let _ = CAP_DOCUMENT_PROPOSE_OPS;
            let mut host = caller.data().host.borrow_mut();
            host.propose_operations(
                &plugin_id,
                vec![ProposedOp {
                    kind: "noop".into(),
                    payload: Vec::new(),
                }],
            )?;
            Ok(0)
        }
        opcode::PROPOSE_INSERT_BLOCK => {
            let type_name = read_guest_bytes(caller, ptr_a, len_a)?;
            let body = read_guest_bytes(caller, ptr_b, len_b)?;
            let mut payload = type_name;
            payload.push(0);
            payload.extend_from_slice(&body);
            let plugin_id = caller.data().plugin_id.clone();
            let mut host = caller.data().host.borrow_mut();
            host.propose_operations(
                &plugin_id,
                vec![ProposedOp {
                    kind: "insert_block".into(),
                    payload,
                }],
            )?;
            Ok(0)
        }
        other => Err(PluginError::Trap(format!("unknown host_call op {other}"))),
    }
}

/// Convenience: load plugin on host then run wasm `activate` with shared fuel budget.
pub fn load_and_activate_wasm(
    host: Rc<RefCell<PluginHost>>,
    manifest: crate::manifest::PluginManifest,
    granted: impl IntoIterator<Item = qf_core::Capability>,
    wasm_bytes: &[u8],
    fuel: u64,
) -> Result<(PluginId, GuestRunResult), PluginError> {
    let id = {
        let mut h = host.borrow_mut();
        h.set_fuel_budget(fuel);
        h.load(manifest, granted)?
    };
    let engine = WasmEngine::new()?;
    let result = engine.run_activate(host, id.clone(), wasm_bytes, fuel)?;
    Ok((id, result))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::caps::{CAP_BLOCK_CUSTOM_REGISTER, CAP_DOCUMENT_PROPOSE_OPS};
    use crate::manifest::PluginManifest;
    use qf_core::{Capability, PluginId};

    fn sample_manifest() -> PluginManifest {
        PluginManifest {
            id: PluginId("com.example.wasm".into()),
            name: "Wasm Demo".into(),
            version: "1.0.0".into(),
            qf_api_version: "1.0".into(),
            entry_wasm: Some("plugin.wasm".into()),
            author: None,
            required_capabilities: vec![
                Capability(CAP_DOCUMENT_PROPOSE_OPS.into()),
                Capability(CAP_BLOCK_CUSTOM_REGISTER.into()),
            ],
            optional_capabilities: vec![],
            declared_block_types: vec!["com.example.wasm/note".into()],
        }
    }

    /// Minimal guest: log, kv set/get, register type, propose noop, return 42.
    fn demo_guest_wasm() -> Vec<u8> {
        let wat = r#"
(module
  (import "qf" "host_call" (func $host_call (param i32 i32 i32 i32 i32) (result i32)))
  (import "qf" "scratch_len" (func $scratch_len (result i32)))
  (import "qf" "scratch_copy" (func $scratch_copy (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "hello-from-wasm")
  (data (i32.const 32) "k")
  (data (i32.const 34) "v")
  (data (i32.const 36) "com.example.wasm/note")
  (data (i32.const 64) "{}")
  (func (export "activate") (result i32)
    ;; log ("hello-from-wasm" = 15 bytes)
    (drop (call $host_call (i32.const 1) (i32.const 0) (i32.const 15) (i32.const 0) (i32.const 0)))
    ;; kv_set k=v
    (drop (call $host_call (i32.const 3) (i32.const 32) (i32.const 1) (i32.const 34) (i32.const 1)))
    ;; kv_get k
    (drop (call $host_call (i32.const 2) (i32.const 32) (i32.const 1) (i32.const 0) (i32.const 0)))
    ;; register block type (type name = 21 bytes)
    (drop (call $host_call (i32.const 4) (i32.const 36) (i32.const 21) (i32.const 64) (i32.const 2)))
    ;; propose noop
    (drop (call $host_call (i32.const 5) (i32.const 0) (i32.const 0) (i32.const 0) (i32.const 0)))
    i32.const 42
  )
)
"#;
        wat::parse_str(wat).expect("wat parse")
    }

    #[test]
    fn wasm_activate_hostcalls_and_capabilities() {
        let host = Rc::new(RefCell::new(PluginHost::new()));
        let wasm = demo_guest_wasm();
        let (id, result) = load_and_activate_wasm(
            host.clone(),
            sample_manifest(),
            [
                Capability(CAP_DOCUMENT_PROPOSE_OPS.into()),
                Capability(CAP_BLOCK_CUSTOM_REGISTER.into()),
            ],
            &wasm,
            1_000_000,
        )
        .unwrap();
        assert_eq!(result.status, 42);
        assert!(result.logs.iter().any(|l| l.contains("hello-from-wasm")));
        assert!(host.borrow().type_registry().is_registered("com.example.wasm/note"));
        let v = host.borrow_mut().kv_get(&id, b"k").unwrap();
        assert_eq!(v.as_deref(), Some(b"v".as_ref()));
    }

    #[test]
    fn wasm_fuel_exhaustion_traps() {
        let host = Rc::new(RefCell::new(PluginHost::new()));
        let id = host
            .borrow_mut()
            .load(
                sample_manifest(),
                [
                    Capability(CAP_DOCUMENT_PROPOSE_OPS.into()),
                    Capability(CAP_BLOCK_CUSTOM_REGISTER.into()),
                ],
            )
            .unwrap();
        // Infinite loop burns fuel deterministically.
        let wat = r#"
(module
  (import "qf" "host_call" (func $host_call (param i32 i32 i32 i32 i32) (result i32)))
  (import "qf" "scratch_len" (func $scratch_len (result i32)))
  (import "qf" "scratch_copy" (func $scratch_copy (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (func (export "activate") (result i32)
    (loop $spin
      br $spin)
    i32.const 0
  )
)
"#;
        let wasm = wat::parse_str(wat).unwrap();
        let engine = WasmEngine::new().unwrap();
        let err = engine.run_activate(host, id, &wasm, 500).unwrap_err();
        assert!(
            matches!(err, PluginError::Trap(_)),
            "expected fuel trap, got {err:?}"
        );
    }

    #[test]
    fn wasm_denied_capability_fails_hostcall() {
        let host = Rc::new(RefCell::new(PluginHost::new()));
        // Grant only propose; guest still tries register_block_type.
        let manifest = PluginManifest {
            required_capabilities: vec![Capability(CAP_DOCUMENT_PROPOSE_OPS.into())],
            ..sample_manifest()
        };
        // Guest will hit register without cap → host_call returns -1 but guest ignores;
        // force by only calling register: use a tiny module.
        let wat = r#"
(module
  (import "qf" "host_call" (func $host_call (param i32 i32 i32 i32 i32) (result i32)))
  (import "qf" "scratch_len" (func $scratch_len (result i32)))
  (import "qf" "scratch_copy" (func $scratch_copy (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "com.example.denied/x")
  (data (i32.const 32) "{}")
  (func (export "activate") (result i32)
    (call $host_call (i32.const 4) (i32.const 0) (i32.const 20) (i32.const 32) (i32.const 2))
  )
)
"#;
        let wasm = wat::parse_str(wat).unwrap();
        let id = host
            .borrow_mut()
            .load(manifest, [Capability(CAP_DOCUMENT_PROPOSE_OPS.into())])
            .unwrap();
        let engine = WasmEngine::new().unwrap();
        let result = engine
            .run_activate(host.clone(), id, &wasm, 100_000)
            .unwrap();
        // host_call returns -1 on denied; guest returns that status
        assert_eq!(result.status, -1);
        assert!(!host
            .borrow()
            .type_registry()
            .is_registered("com.example.denied/x"));
    }
}
