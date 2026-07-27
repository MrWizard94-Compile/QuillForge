//! Plugin manifest types and parsing.

use qf_core::{Capability, PluginId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub qf_api_version: String,
    pub entry_wasm: Option<String>,
    pub author: Option<String>,
    pub required_capabilities: Vec<Capability>,
    pub optional_capabilities: Vec<Capability>,
    pub declared_block_types: Vec<String>,
}

/// Minimal TOML-like line parser for manifests (no full TOML dependency).
///
/// Supported keys:
/// `id`, `name`, `version`, `qf_api_version`, `entry_wasm`, `author`,
/// `required = ["a","b"]`, `optional = [...]`, `blocks = ["type"]`
pub fn parse_manifest_toml_lite(text: &str) -> Result<PluginManifest, String> {
    let mut id = None;
    let mut name = None;
    let mut version = None;
    let mut qf_api_version = None;
    let mut entry_wasm = None;
    let mut author = None;
    let mut required = Vec::new();
    let mut optional = Vec::new();
    let mut blocks = Vec::new();

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let k = k.trim();
        let v = v.trim().trim_matches('"').trim();
        match k {
            "id" => id = Some(v.to_string()),
            "name" => name = Some(v.to_string()),
            "version" => version = Some(v.to_string()),
            "qf_api_version" => qf_api_version = Some(v.to_string()),
            "entry_wasm" => entry_wasm = Some(v.to_string()),
            "author" => author = Some(v.to_string()),
            "required" => required = parse_string_list(v),
            "optional" => optional = parse_string_list(v),
            "blocks" => blocks = parse_string_list(v),
            _ => {}
        }
    }

    Ok(PluginManifest {
        id: PluginId(id.ok_or("missing id")?),
        name: name.ok_or("missing name")?,
        version: version.unwrap_or_else(|| "0.0.0".into()),
        qf_api_version: qf_api_version.unwrap_or_else(|| "1.0".into()),
        entry_wasm,
        author,
        required_capabilities: required.into_iter().map(Capability).collect(),
        optional_capabilities: optional.into_iter().map(Capability).collect(),
        declared_block_types: blocks,
    })
}

fn parse_string_list(v: &str) -> Vec<String> {
    let v = v.trim().trim_start_matches('[').trim_end_matches(']');
    if v.is_empty() {
        return vec![];
    }
    v.split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lite_manifest() {
        let m = parse_manifest_toml_lite(
            r#"
id = "com.example.x"
name = "X"
version = "1.2.3"
required = ["block.custom.register", "export.transform"]
blocks = ["com.example.x/t"]
"#,
        )
        .unwrap();
        assert_eq!(m.id.0, "com.example.x");
        assert_eq!(m.required_capabilities.len(), 2);
        assert_eq!(m.declared_block_types, vec!["com.example.x/t"]);
    }
}
