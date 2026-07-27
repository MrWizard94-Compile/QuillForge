# Plugin Sandbox Specification

**Document ID:** QF-PLG-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Goals

- Extensibility without compromising integrity or security  
- Plugins MUST NOT bypass command/journal/kernel  
- Plugins MUST NOT have raw filesystem access  
- Capability-based least privilege  

---

## 2. Isolation model (v1 decision)

**WebAssembly (component model) plugins** running in a deterministic guest.

| Property | Choice |
|----------|--------|
| Runtime | Wasmtime (or equivalent) with fuel/epoch interruption |
| Memory | Isolated guest linear memory |
| Threads | No untrusted threads in v1 |
| Filesystem | None by default |
| Network | None by default; capability-gated HTTP later |
| Native `.dll/.so` plugins | **Forbidden** for third parties in v1 |

---

## 3. Manifest

```toml
# plugin.toml (packaged with wasm)
id = "com.example.mathblocks"
name = "Math Blocks"
version = "1.2.0"
qf_api_version = "1.0"
entry_wasm = "plugin.wasm"
author = "Example Inc"
license = "MIT"

[capabilities]
# empty default
required = [
  "block.custom.register",
  "export.transform",
]
optional = [
  "search.provider",
  "ai.provider",
  "theme.provider",
]

[blocks]
"com.example.mathblocks/equation2" = { schema = "schemas/equation2.cbor" }

[permissions_rationale]
"export.transform" = "Adds LaTeX export pass"
```

Unsigned plugins: allowed only in developer mode with explicit user trust prompt.  
Enterprise: require signature by trusted keys (see security).

---

## 4. Capabilities

```rust
pub enum Capability {
    BlockCustomRegister,
    ExportTransform,
    SearchProvider,
    AiProvider,
    ThemeProvider,
    UiPanel,
    ClipboardRead,
    ClipboardWrite,
    NetworkHttp { host_patterns: Vec<String> }, // optional, default deny
    DocumentRead,          // read canonical projection via host API
    DocumentProposeOps,    // propose ops; still need user/command commit
    UnknownReplace,        // dangerous; rarely granted
}
```

**Host enforces:** every hostcall checks capability. Missing → `PluginError::Denied`.

---

## 5. Host API (guest-visible)

```rust
// Conceptual WIT-like interface
trait PluginHost {
    fn log(&self, level: Level, msg: &str);
    fn get_block_snapshot(&self, id: BlockId) -> Result<BlockView, HostError>;
    fn propose_operations(&self, ops: Vec<OperationDto>) -> Result<ProposalId, HostError>;
    fn register_block_type(&self, type_name: &str, schema: &[u8]) -> Result<(), HostError>;
    fn register_export_hook(&self, format: &str) -> Result<(), HostError>;
    fn storage_get(&self, key: &[u8]) -> Result<Option<Vec<u8>>, HostError>; // plugin-private KV
    fn storage_set(&self, key: &[u8], val: &[u8]) -> Result<(), HostError>;
}
```

**Plugin-private KV:** stored under package or user plugin data dir, namespaced by `PluginId`, quota-limited. Not raw FS paths.

**Propose operations:** creates a pending proposal; application applies only via command bus (`PluginCommand` or user accept). Plugins never receive `Journal` handles.

---

## 6. Hooks

| Hook | When | Mutates canonical? |
|------|------|--------------------|
| `on_activate` | Plugin load | No |
| `on_deactivate` | Unload | No |
| `decode_block` | Load unknown type match | No (returns typed view) |
| `encode_block` | Save typed block | No (returns bytes) |
| `export_transform` | Export pipeline stage | No (transforms export AST) |
| `search_contribute` | Indexing | No |
| `ai_complete` | AI request routed to plugin | No |
| `theme_tokens` | Theme query | No |
| `render_placeholder` | UI for custom block | No |

---

## 7. Custom blocks

1. Manifest declares type name.  
2. On activate, host registers type in `TypeRegistry`.  
3. Decode/encode hooks convert between `original_payload` and structured guest objects.  
4. Editing UI for custom blocks is plugin-provided view; edits become `propose_operations`.  
5. On deactivate, blocks become `UnknownBlock` with preserved payload.

---

## 8. Resource limits

| Resource | v1 default |
|----------|------------|
| Memory | 64 MiB |
| Fuel per hostcall batch | configured epoch |
| Plugin KV size | 16 MiB |
| Concurrent plugins | soft 32 |
| Export hook timeout | 5s |

Violation → trap plugin instance; document remains healthy.

---

## 9. Security rules

1. No pointer sharing with host AST.  
2. All data copied across boundary.  
3. Operations proposed by plugins re-validated by command handlers as if user-issued (plus plugin actor attribution).  
4. AI provider plugins still require user content permission grants.  
5. Plugins cannot read other plugins' KV.  

---

## 10. Failure behavior

| Failure | Result |
|---------|--------|
| Wasm trap | Disable plugin; notify; content → unknown on next load if types unregistered |
| Invalid op proposal | Reject proposal |
| Timeout | Cancel hook; continue without plugin contribution |
| Capability abuse attempts | Deny + audit |

---

## 11. Testing

- Capability denial matrix  
- Malicious plugin attempts journal access (must be impossible)  
- Custom block round-trip with plugin removed  
- Fuel exhaustion resilience  
- Signature verification  

---

*End of plugin sandbox specification.*
