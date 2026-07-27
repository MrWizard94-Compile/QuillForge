# Unknown-Content Preservation Specification

**Document ID:** QF-UNK-001  
**Version:** 1.0.0  
**Status:** Binding  

**Priority rank:** 4 (after integrity, recovery, determinism)

---

## 1. Problem statement

Plugins and future app versions introduce block types, marks, and metadata keys that older (or different) installations cannot interpret. The largest long-term data-loss risk is **silent dropping** of unrecognized content during save/load/migrate/edit.

Quillforge treats unknown content as first-class durable data.

---

## 2. Hard requirement

Custom blocks and unknown data MUST survive round trips unchanged, including after:

- Plugin removal  
- Plugin reinstall  
- Save/load  
- Package migration  
- Document edits around the unknown content  

If the system cannot interpret a block, it MUST preserve the **original type** and **payload exactly**.

---

## 3. Core structure

```rust
pub struct UnknownBlock {
    /// Original type discriminator as stored in package, e.g. "com.example.math/v2".
    pub original_type: String,
    /// Exact original payload bytes (CBOR/JSON/plugin encoding) as found on disk.
    pub original_payload: Vec<u8>,
    /// Shared block metadata (ids, attrs). Attr unknown keys also preserved.
    pub metadata: BlockMetadata,
}
```

### Why this exists

| Reason | Detail |
|--------|--------|
| Plugin lifecycle | User removes plugin; content must remain until plugin returns |
| Forward compatibility | Newer docs open in older apps without stripping |
| Migration safety | Migrations rewrite known schema only |
| Edit adjacency | Users can still move/delete surrounding blocks |
| Legal/audit | Enterprises cannot lose regulated content because a plugin expired |

---

## 4. Serialization

### 4.1 On-disk block encoding (canonical CBOR map)

```text
{
  "id": <uuid bytes or string>,
  "meta": { ... },
  "type": "<original_type string>",
  "body": <encoded known body> | { "unknown": true, "payload": <bstr> }
}
```

**Rules:**

1. When decoding: if `type` is not in the **active type registry**, construct `UnknownBlock` with:
   - `original_type = type`
   - `original_payload = raw body bytes` **exactly as sliced from the package** (prefer storing raw frame)
2. Prefer capturing **raw body byte slice** before typed decode attempts, so failed partial parses do not corrupt payload.
3. When encoding `UnknownBlock`: write `type = original_type` and body bytes = `original_payload` **verbatim**. Do not re-encode through a typed schema.
4. `metadata.attrs` uses `PreservedMap` which retains unknown keys and values as raw CBOR items.

```rust
pub struct PreservedMap {
    /// Known keys decoded; unknown keys kept as raw pairs for round-trip.
    entries: BTreeMap<String, PreservedValue>,
}

pub enum PreservedValue {
    Null,
    Bool(bool),
    Integer(i64),
    Text(String),
    Bytes(Vec<u8>),
    Array(Vec<PreservedValue>),
    Map(PreservedMap),
    /// Opaque CBOR that didn't fit above; still round-tripped
    RawCbor(Vec<u8>),
}
```

### 4.2 Marks

Unknown marks:

```rust
Mark::Unknown { type_name: String, payload: Vec<u8> }
```

Same verbatim payload rules.

### 4.3 Package-level unknown files

Files in `.qfdoc` not recognized by the current reader are retained in `other_files` and rewritten unchanged on save (unless user explicitly strips on export).

---

## 5. Runtime behavior

### 5.1 Display

- Unknown blocks render as a **placeholder card**: type name, size, “plugin missing” status.  
- Content is not interpreted.  
- Placeholder is derived UI only; does not alter payload.

### 5.2 Editing around unknown content

| Operation | Allowed? |
|-----------|----------|
| Move block | Yes (structural op) |
| Delete block | Yes (explicit user command) |
| Duplicate block | Yes (copies payload bytes) |
| Edit interior payload without plugin | No (no silent partial edit) |
| Split unknown as text | No |
| Apply text marks inside | No |

### 5.3 Plugin reinstall promotion

```
on_load / on_plugin_activate:
  if registry recognizes original_type:
    attempt decode(original_payload)
    if Ok(typed) → BlockBody::Typed(...)  // in-memory only until next save encodes typed form
    if Err → remain UnknownBlock (payload unchanged)
```

**Save after successful promotion:** May write typed encoding **only if** encoder is bit-exact compatible or versioned upgrade is explicit. Safest path: if plugin declares `preserve_raw_until_edit`, keep raw until user edits through plugin.

**v1 decision:** After successful decode, save MAY write typed form from plugin encoder. Round-trip tests require plugin encoder to preserve semantic equality. Raw unknown path remains until promotion succeeds.

### 5.4 Migrations

Migrations MUST:

- Count unknown blocks before/after  
- Assert `original_payload` equality per block id  
- Not parse payload  

Unless a migration is specifically written for a known-retired type (then it is a known type at migrate time).

---

## 6. Operations affecting unknown blocks

```rust
// Allowed structural ops
Operation::MoveBlock { .. }
Operation::RemoveBlock { .. }
Operation::InsertBlock { body: BlockBody::Unknown(..), .. }

// Forbidden without plugin capability
// any op that mutates original_payload partially
```

Explicit raw replace (admin/plugin):

```rust
Operation::ReplaceUnknownPayload {
    block_id: BlockId,
    original_type: String,
    original_payload: Vec<u8>,
}
```

Requires capability `document.unknown.replace`.

---

## 7. Export behavior

| Format | Behavior |
|--------|----------|
| Markdown | HTML comment or fenced block with base64 payload + type |
| HTML | `<div data-qf-unknown-type="..." data-qf-payload-b64="...">` |
| PDF | Visible placeholder box with type name |
| DOCX | Structured document tag / alt chunk if possible; else placeholder paragraph |
| `.qfdoc` copy | Verbatim preservation |

Deterministic export includes stable placeholder text.

---

## 8. Search / layout / AI

| System | Behavior |
|--------|----------|
| Token search | Index type string + optional plugin-provided text extract; never drop block from index identity |
| Layout | Fixed min-height placeholder; deterministic |
| AI | MUST NOT receive unknown payloads unless user grants capability and explicitly includes them |

---

## 9. Invariants

| ID | Invariant |
|----|-----------|
| U1 | Decode→encode of UnknownBlock is byte-identical for type+payload |
| U2 | Edits to other blocks do not modify unknown payloads |
| U3 | Migration without type knowledge does not modify unknown payloads |
| U4 | Plugin removal does not rewrite unknown payloads |
| U5 | Delete of unknown requires explicit op |

---

## 10. Required tests

1. **Round-trip:** invent type `test.unknown/v1` with random payload → save → load → payload equal.  
2. **Adjacent edit:** insert paragraphs before/after → payload equal.  
3. **Move/delete/undo:** structural ops; undo restores.  
4. **Migrate:** run sample migration; payloads equal.  
5. **Plugin cycle:** known→save→remove plugin→load unknown→reinstall→promote.  
6. **Export/import:** HTML unknown attributes survive re-import where supported.  
7. **Fuzz:** random unknown payloads with CBOR edge cases.

---

## 11. Implementation placement

| Piece | Crate |
|-------|-------|
| `UnknownBlock` type | `qf-blocks` / `qf-doc` |
| Decode registry | `qf-doc` |
| Preserve maps | `qf-core` |
| Tests | `qf-storage` integration + `qf-doc` |

---

*End of unknown-content preservation specification.*
