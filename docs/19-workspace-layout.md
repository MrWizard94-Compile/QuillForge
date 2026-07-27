# Rust Workspace Layout

**Document ID:** QF-WS-001  
**Version:** 1.0.0  
**Status:** Binding  

---

## 1. Root structure

```
QuillForge/
├── Cargo.toml                 # virtual workspace
├── Cargo.lock
├── rust-toolchain.toml
├── clippy.toml
├── rustfmt.toml
├── .gitignore
├── README.md
├── AGENTS.md
├── docs/                      # engineering package (this set)
├── xtask/                     # automation binary
├── crates/
│   ├── qf-core/
│   ├── qf-kernel/
│   ├── qf-journal/
│   ├── qf-doc/
│   ├── qf-blocks/
│   ├── qf-layout/
│   ├── qf-render/
│   ├── qf-gpu/
│   ├── qf-input/
│   ├── qf-storage/
│   ├── qf-search/
│   ├── qf-sync/
│   ├── qf-collab/
│   ├── qf-export/
│   ├── qf-ai/
│   ├── qf-citation/
│   ├── qf-plugin/
│   ├── qf-security/
│   ├── qf-telemetry/
│   ├── qf-theme/
│   └── qf-app/
├── fixtures/                  # golden docs, corrupt packages, fonts
├── scripts/                   # non-rust helpers if needed
└── third_party/               # pinned fonts, licenses
```

---

## 2. Virtual manifest rules

```toml
[workspace]
resolver = "2"
members = [
  "crates/*",
  "xtask",
]
default-members = ["crates/qf-app"]

[workspace.package]
version = "0.0.0"
edition = "2021"
license = "UNLICENSED"
publish = false
authors = ["Quillforge Team"]

[workspace.dependencies]
# pinned versions live here; crates use workspace = true
```

Internal crates use **version `0.0.0`** and `publish = false`.

---

## 3. Dependency direction (acyclic)

```
qf-core
  ↑
  ├── qf-blocks ← qf-doc
  ├── qf-journal
  ├── qf-kernel ← qf-doc, qf-journal, qf-blocks
  ├── qf-security
  ├── qf-telemetry (leaf-ish; no content deps upward)
  │
  ├── qf-storage ← kernel, journal, security
  ├── qf-layout ← qf-doc
  ├── qf-gpu
  ├── qf-render ← layout, gpu, theme
  ├── qf-input
  ├── qf-search ← doc/kernel events
  ├── qf-export ← doc, layout (pdf)
  ├── qf-citation ← doc
  ├── qf-collab ← kernel, doc
  ├── qf-sync ← collab, journal, security
  ├── qf-plugin ← core, security
  ├── qf-ai ← collab suggestions, security
  ├── qf-theme
  └── qf-app ← everything (binary)
```

**Forbidden:** `qf-core` depending on any other `qf-*`.  
**Forbidden:** cycles between layout/render/search and kernel.

---

## 4. Per-crate folder scaffold

```
crates/qf-example/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── error.rs
│   └── ...
└── tests/          # integration tests when needed
```

`qf-app` is a binary crate (`src/main.rs`).

---

## 5. xtask commands

| Command | Purpose |
|---------|---------|
| `cargo xtask check` | fmt + clippy + build + test |
| `cargo xtask gen` | codegen if any |
| `cargo xtask fixtures` | pack golden fixtures |
| `cargo xtask fuzz` | run fuzz smoke |
| `cargo xtask release-dry` | version hygiene |

---

## 6. Toolchain

```toml
# rust-toolchain.toml
[toolchain]
channel = "1.85.0"  # pin on implement; update deliberately
components = ["rustfmt", "clippy", "rust-analyzer"]
```

---

## 7. First implementation order (crates)

1. `qf-core`  
2. `qf-blocks` + `qf-doc`  
3. `qf-journal`  
4. `qf-kernel`  
5. command bus (in `qf-core` + doc handlers)  
6. `qf-storage`  
7. recovery/migration tests  
8. `qf-layout`  
9. `qf-gpu` + `qf-render`  
10. `qf-input` + undo in app/core  
11. `qf-export` + `qf-search`  
12. `qf-collab` + `qf-sync`  
13. `qf-plugin`  
14. `qf-ai`  
15. `qf-security` hardening, `qf-telemetry`, `qf-theme`, `qf-app` polish  

---

*End of workspace layout.*
