//! Local-first font packs for deterministic export embedding.
//!
//! # Law
//!
//! - Fonts are **local packages** (bytes + metadata); no system font substitution
//!   for export (avoids cross-OS nondeterminism).
//! - Default pin: built-in monospaced TrueType (`quill_mono_v1`) generated in-process
//!   so fixtures need no binary assets.
//! - PDF embeds TrueType via `/FontFile2`; DOCX uses the family name in styles.

use qf_core::Hash256;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Font pack algorithm / pin id.
pub const FONT_ALGO_QUILL_MONO_V1: &str = "quill_mono_v1";
pub const FONT_ALGO_TRUETYPE_V1: &str = "truetype_v1";

/// Portable font pack metadata + optional TTF payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FontPack {
    pub version: u32,
    /// Human / CSS family name (e.g. "QuillForge Mono").
    pub family: String,
    /// PDF BaseFont / PostScript-style name (no spaces preferred).
    pub postscript_name: String,
    pub algo: String,
    /// SHA-256 hex of `ttf_bytes` when present.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// TrueType / OpenType bytes (not serialized in tiny JSON sidecars by default).
    #[serde(skip)]
    pub ttf_bytes: Vec<u8>,
}

impl Default for FontPack {
    fn default() -> Self {
        quill_mono_v1()
    }
}

impl FontPack {
    /// Built-in deterministic monospaced pin (always embeddable).
    pub fn pin_default() -> Self {
        quill_mono_v1()
    }

    pub fn from_ttf_bytes(family: impl Into<String>, bytes: Vec<u8>) -> Result<Self, String> {
        if bytes.len() < 12 {
            return Err("font file too small".into());
        }
        if !looks_like_ttf(&bytes) {
            return Err("not a TrueType/OpenType font (bad sfnt magic)".into());
        }
        let family = family.into();
        let ps = family_to_postscript(&family);
        let hash = Hash256::sha256(&bytes);
        Ok(Self {
            version: 1,
            family,
            postscript_name: ps,
            algo: FONT_ALGO_TRUETYPE_V1.into(),
            sha256: Some(hex_encode(&hash.0)),
            ttf_bytes: bytes,
        })
    }

    pub fn load_ttf_path(path: &Path, family: impl Into<String>) -> Result<Self, String> {
        let bytes =
            std::fs::read(path).map_err(|e| format!("read font {}: {e}", path.display()))?;
        Self::from_ttf_bytes(family, bytes)
    }

    pub fn content_hash(&self) -> Hash256 {
        Hash256::sha256(&self.ttf_bytes)
    }

    /// Units-per-em (defaults 1000 if unreadable).
    pub fn units_per_em(&self) -> u16 {
        parse_units_per_em(&self.ttf_bytes).unwrap_or(1000)
    }

    /// Advance width in font units for ASCII printable (fixed mono for pin font).
    pub fn advance_width(&self, ch: u8) -> u16 {
        if !(32..=126).contains(&ch) {
            return 0;
        }
        // Monospace pin and foundation fallback: half em (or UPEM/2).
        self.units_per_em() / 2
    }

    /// PDF widths array for FirstChar=32 .. LastChar=126 (glyph space units).
    pub fn pdf_widths_32_126(&self) -> Vec<u16> {
        (32u8..=126).map(|c| self.advance_width(c)).collect()
    }

    pub fn to_json_meta(&self) -> String {
        format!(
            r#"{{"version":{},"family":"{}","postscript_name":"{}","algo":"{}","sha256":{}}}"#,
            self.version,
            json_esc(&self.family),
            json_esc(&self.postscript_name),
            json_esc(&self.algo),
            self.sha256
                .as_ref()
                .map(|h| format!(r#""{}""#, json_esc(h)))
                .unwrap_or_else(|| "null".into())
        )
    }
}

fn json_esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn family_to_postscript(family: &str) -> String {
    let s: String = family
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    if s.is_empty() {
        "QuillForgeFont".into()
    } else {
        s
    }
}

fn looks_like_ttf(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    let mag = &bytes[0..4];
    mag == b"\x00\x01\x00\x00" || mag == b"OTTO" || mag == b"true" || mag == b"typ1"
}

fn parse_units_per_em(ttf: &[u8]) -> Option<u16> {
    let off = find_table(ttf, b"head")?;
    if ttf.len() < off + 20 {
        return None;
    }
    Some(u16::from_be_bytes([ttf[off + 18], ttf[off + 19]]))
}

fn find_table(ttf: &[u8], tag: &[u8; 4]) -> Option<usize> {
    if ttf.len() < 12 {
        return None;
    }
    let num = u16::from_be_bytes([ttf[4], ttf[5]]) as usize;
    let mut p = 12usize;
    for _ in 0..num {
        if p + 16 > ttf.len() {
            return None;
        }
        if &ttf[p..p + 4] == tag {
            let offset = u32::from_be_bytes([ttf[p + 8], ttf[p + 9], ttf[p + 10], ttf[p + 11]]);
            return Some(offset as usize);
        }
        p += 16;
    }
    None
}

fn hex_encode(bytes: &[u8]) -> String {
    const H: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(H[(b >> 4) as usize] as char);
        out.push(H[(b & 0xf) as usize] as char);
    }
    out
}

/// Built-in monospaced TrueType pin used when no custom file is supplied.
pub fn quill_mono_v1() -> FontPack {
    let bytes = build_mono_ttf("QuillForge Mono", 1000, 500);
    let hash = Hash256::sha256(&bytes);
    FontPack {
        version: 1,
        family: "QuillForge Mono".into(),
        postscript_name: "QuillForgeMono".into(),
        algo: FONT_ALGO_QUILL_MONO_V1.into(),
        sha256: Some(hex_encode(&hash.0)),
        ttf_bytes: bytes,
    }
}

// --- Minimal TrueType builder (ASCII monospaced rectangles) ---

/// Build a deterministic monospaced TTF covering U+0020–U+007E.
pub fn build_mono_ttf(name: &str, upem: u16, advance: u16) -> Vec<u8> {
    let first = 32u16;
    let last = 126u16;
    let glyph_count = (last - first + 1) + 1; // + .notdef
    let mut glyf: Vec<u8> = Vec::new();
    let mut loca: Vec<u32> = Vec::with_capacity(glyph_count as usize + 1);
    // glyph 0 .notdef — empty
    loca.push(0);
    glyf.extend_from_slice(&empty_glyph());
    loca.push(glyf.len() as u32);
    for ch in first..=last {
        if ch == 32 {
            glyf.extend_from_slice(&empty_glyph());
        } else {
            glyf.extend_from_slice(&box_glyph(advance, upem));
        }
        loca.push(glyf.len() as u32);
    }

    let hmtx = build_hmtx(glyph_count, advance);
    let head = build_head(upem);
    let hhea = build_hhea(upem, advance, glyph_count);
    let maxp = build_maxp(glyph_count);
    let cmap = build_cmap(first, last);
    let name_tbl = build_name(name);
    let post = build_post();
    let loca_bytes = build_loca_long(&loca);

    // Assemble with table directory (sorted by tag).
    let mut tables: Vec<([u8; 4], Vec<u8>)> = vec![
        (*b"cmap", cmap),
        (*b"glyf", glyf),
        (*b"head", head),
        (*b"hhea", hhea),
        (*b"hmtx", hmtx),
        (*b"loca", loca_bytes),
        (*b"maxp", maxp),
        (*b"name", name_tbl),
        (*b"post", post),
    ];
    tables.sort_by(|a, b| a.0.cmp(&b.0));

    let num_tables = tables.len() as u16;
    let entry_selector = (15u16 - num_tables.leading_zeros() as u16).saturating_sub(1);
    let search_range = 16u16 * (1u16 << entry_selector.min(4));
    let range_shift = num_tables * 16 - search_range;

    let mut out = Vec::new();
    out.extend_from_slice(&0x00010000u32.to_be_bytes()); // scaler type
    out.extend_from_slice(&num_tables.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());

    let header_len = 12 + tables.len() * 16;
    let mut offset = header_len as u32;
    // Reserve directory
    let dir_start = out.len();
    out.resize(header_len, 0);

    for (i, (tag, data)) in tables.iter().enumerate() {
        let mut padded = data.clone();
        while padded.len() % 4 != 0 {
            padded.push(0);
        }
        let checksum = table_checksum(&padded);
        let off = offset;
        let len = data.len() as u32;
        let entry = dir_start + i * 16;
        out[entry..entry + 4].copy_from_slice(tag);
        out[entry + 4..entry + 8].copy_from_slice(&checksum.to_be_bytes());
        out[entry + 8..entry + 12].copy_from_slice(&off.to_be_bytes());
        out[entry + 12..entry + 16].copy_from_slice(&len.to_be_bytes());
        out.extend_from_slice(&padded);
        offset = out.len() as u32;
    }

    // head.checkSumAdjustment
    if let Some(head_off) = find_table(&out, b"head") {
        // checksum of entire font with adjustment = 0
        if head_off + 12 <= out.len() {
            out[head_off + 8..head_off + 12].copy_from_slice(&0u32.to_be_bytes());
            let sum = table_checksum(&out);
            let adj = 0xB1B0AFBAu32.wrapping_sub(sum);
            out[head_off + 8..head_off + 12].copy_from_slice(&adj.to_be_bytes());
        }
    }
    out
}

fn table_checksum(data: &[u8]) -> u32 {
    let mut sum = 0u32;
    let mut i = 0;
    while i + 4 <= data.len() {
        sum = sum.wrapping_add(u32::from_be_bytes([
            data[i],
            data[i + 1],
            data[i + 2],
            data[i + 3],
        ]));
        i += 4;
    }
    if i < data.len() {
        let mut last = [0u8; 4];
        last[..data.len() - i].copy_from_slice(&data[i..]);
        sum = sum.wrapping_add(u32::from_be_bytes(last));
    }
    sum
}

fn empty_glyph() -> Vec<u8> {
    // numberOfContours = 0
    let mut g = Vec::new();
    g.extend_from_slice(&0i16.to_be_bytes());
    g.extend_from_slice(&0i16.to_be_bytes()); // xMin
    g.extend_from_slice(&0i16.to_be_bytes());
    g.extend_from_slice(&0i16.to_be_bytes());
    g.extend_from_slice(&0i16.to_be_bytes()); // yMax
    g
}

fn box_glyph(advance: u16, upem: u16) -> Vec<u8> {
    // Simple rectangle outline in font units.
    let pad = (advance as i16) / 8;
    let x0 = pad;
    let x1 = advance as i16 - pad;
    let y0 = (upem as i16) / 10;
    let y1 = (upem as i16) * 7 / 10;
    // 4-point contour
    let mut g = Vec::new();
    g.extend_from_slice(&1i16.to_be_bytes()); // contours
    g.extend_from_slice(&x0.to_be_bytes());
    g.extend_from_slice(&y0.to_be_bytes());
    g.extend_from_slice(&x1.to_be_bytes());
    g.extend_from_slice(&y1.to_be_bytes());
    // endPtsOfContours
    g.extend_from_slice(&3u16.to_be_bytes());
    g.extend_from_slice(&0u16.to_be_bytes()); // instruction length
                                              // flags: on-curve for 4 points
    g.extend_from_slice(&[0x01; 4]);
    // x deltas: x0, x1-x0, x1-x1=0? points: (x0,y0),(x1,y0),(x1,y1),(x0,y1)
    // Use short vectors with sign
    write_coord_delta(&mut g, x0);
    write_coord_delta(&mut g, x1 - x0);
    write_coord_delta(&mut g, 0);
    write_coord_delta(&mut g, x0 - x1);
    write_coord_delta(&mut g, y0);
    write_coord_delta(&mut g, 0);
    write_coord_delta(&mut g, y1 - y0);
    write_coord_delta(&mut g, 0);
    g
}

fn write_coord_delta(out: &mut Vec<u8>, d: i16) {
    // Store as full int16 (flag bit 1 = x-short not set → 2-byte signed)
    // Flags already set only ON_CURVE; for simplicity use 2-byte coords always
    // Actually with flag 0x01 only, x/y are 2-byte signed deltas — correct.
    out.extend_from_slice(&d.to_be_bytes());
}

fn build_hmtx(glyph_count: u16, advance: u16) -> Vec<u8> {
    let mut v = Vec::with_capacity(glyph_count as usize * 4);
    for _ in 0..glyph_count {
        v.extend_from_slice(&advance.to_be_bytes());
        v.extend_from_slice(&0i16.to_be_bytes()); // lsb
    }
    v
}

fn build_head(upem: u16) -> Vec<u8> {
    let mut h = vec![0u8; 54];
    h[0..4].copy_from_slice(&0x00010000u32.to_be_bytes());
    // revision 1.0
    h[4..8].copy_from_slice(&0x00010000u32.to_be_bytes());
    // checksum adjustment later
    h[12..16].copy_from_slice(&0x5F0F3CF5u32.to_be_bytes()); // magic
    h[18..20].copy_from_slice(&upem.to_be_bytes());
    // created/modified longDateTime zeros ok
    // xMin yMin xMax yMax
    h[36..38].copy_from_slice(&0i16.to_be_bytes());
    h[38..40].copy_from_slice(&0i16.to_be_bytes());
    h[40..42].copy_from_slice(&(upem as i16).to_be_bytes());
    h[42..44].copy_from_slice(&(upem as i16).to_be_bytes());
    h[50..52].copy_from_slice(&0u16.to_be_bytes()); // indexToLocFormat long
    h
}

fn build_hhea(upem: u16, advance: u16, glyph_count: u16) -> Vec<u8> {
    let mut h = vec![0u8; 36];
    h[0..4].copy_from_slice(&0x00010000u32.to_be_bytes());
    let asc = (upem as i16) * 8 / 10;
    let desc = -((upem as i16) / 5);
    h[4..6].copy_from_slice(&asc.to_be_bytes());
    h[6..8].copy_from_slice(&desc.to_be_bytes());
    h[10..12].copy_from_slice(&asc.to_be_bytes()); // advanceWidthMax later
    h[10..12].copy_from_slice(&advance.to_be_bytes());
    h[34..36].copy_from_slice(&glyph_count.to_be_bytes()); // numberOfHMetrics
    h
}

fn build_maxp(glyph_count: u16) -> Vec<u8> {
    let mut m = vec![0u8; 32];
    m[0..4].copy_from_slice(&0x00010000u32.to_be_bytes());
    m[4..6].copy_from_slice(&glyph_count.to_be_bytes());
    m[6..8].copy_from_slice(&4u16.to_be_bytes()); // maxPoints
    m[8..10].copy_from_slice(&1u16.to_be_bytes()); // maxContours
    m
}

fn build_cmap(first: u16, last: u16) -> Vec<u8> {
    // Format 4, platform 3 encoding 1 (BMP)
    let seg_count = 2u16; // [first..last], [0xFFFF]
    let seg_count_x2 = seg_count * 2;
    let entry_selector = 1u16;
    let search_range = 2u16 * 2; // 2 * 2^entry_selector
    let range_shift = seg_count_x2 - search_range;

    let mut body = Vec::new();
    body.extend_from_slice(&4u16.to_be_bytes()); // format
                                                 // length filled later
    body.extend_from_slice(&0u16.to_be_bytes()); // length placeholder
    body.extend_from_slice(&0u16.to_be_bytes()); // language
    body.extend_from_slice(&seg_count_x2.to_be_bytes());
    body.extend_from_slice(&search_range.to_be_bytes());
    body.extend_from_slice(&entry_selector.to_be_bytes());
    body.extend_from_slice(&range_shift.to_be_bytes());
    // endCode
    body.extend_from_slice(&last.to_be_bytes());
    body.extend_from_slice(&0xFFFFu16.to_be_bytes());
    body.extend_from_slice(&0u16.to_be_bytes()); // reservedPad
                                                 // startCode
    body.extend_from_slice(&first.to_be_bytes());
    body.extend_from_slice(&0xFFFFu16.to_be_bytes());
    // idDelta: glyph id for char c = c - first + 1 (.notdef is 0)
    let id_delta = (1i16).wrapping_sub(first as i16);
    body.extend_from_slice(&id_delta.to_be_bytes());
    body.extend_from_slice(&1i16.to_be_bytes()); // for 0xFFFF → glyph 0
                                                 // idRangeOffset
    body.extend_from_slice(&0u16.to_be_bytes());
    body.extend_from_slice(&0u16.to_be_bytes());

    let len = (body.len()) as u16;
    body[2..4].copy_from_slice(&len.to_be_bytes());

    // cmap header + encoding record
    let mut cmap = Vec::new();
    cmap.extend_from_slice(&0u16.to_be_bytes()); // version
    cmap.extend_from_slice(&1u16.to_be_bytes()); // numTables
    cmap.extend_from_slice(&3u16.to_be_bytes()); // platform
    cmap.extend_from_slice(&1u16.to_be_bytes()); // encoding
    cmap.extend_from_slice(&12u32.to_be_bytes()); // offset to subtable
    cmap.extend_from_slice(&body);
    cmap
}

fn build_name(family: &str) -> Vec<u8> {
    // One name record: Windows Unicode BMP English family (nameID 1)
    let raw: Vec<u8> = family
        .encode_utf16()
        .flat_map(|u| u.to_be_bytes())
        .collect();
    let mut t = Vec::new();
    t.extend_from_slice(&0u16.to_be_bytes()); // format
    t.extend_from_slice(&1u16.to_be_bytes()); // count
    t.extend_from_slice(&18u16.to_be_bytes()); // stringOffset
                                               // record
    t.extend_from_slice(&3u16.to_be_bytes()); // platform
    t.extend_from_slice(&1u16.to_be_bytes()); // encoding
    t.extend_from_slice(&0x0409u16.to_be_bytes()); // language en-US
    t.extend_from_slice(&1u16.to_be_bytes()); // nameID family
    t.extend_from_slice(&(raw.len() as u16).to_be_bytes());
    t.extend_from_slice(&0u16.to_be_bytes()); // offset
    t.extend_from_slice(&raw);
    t
}

fn build_post() -> Vec<u8> {
    let mut p = vec![0u8; 32];
    p[0..4].copy_from_slice(&0x00030000u32.to_be_bytes()); // format 3
    p
}

fn build_loca_long(offsets: &[u32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(offsets.len() * 4);
    for o in offsets {
        v.extend_from_slice(&o.to_be_bytes());
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quill_mono_is_valid_ttf() {
        let pack = quill_mono_v1();
        assert!(looks_like_ttf(&pack.ttf_bytes));
        assert_eq!(pack.algo, FONT_ALGO_QUILL_MONO_V1);
        assert!(pack.ttf_bytes.len() > 100);
        assert_eq!(pack.units_per_em(), 1000);
        let a = quill_mono_v1();
        assert_eq!(
            a.ttf_bytes, pack.ttf_bytes,
            "pin font must be deterministic"
        );
    }

    #[test]
    fn custom_ttf_round_trip_meta() {
        let pin = quill_mono_v1();
        let custom = FontPack::from_ttf_bytes("Custom Face", pin.ttf_bytes.clone()).unwrap();
        assert_eq!(custom.algo, FONT_ALGO_TRUETYPE_V1);
        assert!(custom.sha256.is_some());
        assert!(custom.to_json_meta().contains("Custom Face"));
    }

    #[test]
    fn rejects_garbage() {
        assert!(FontPack::from_ttf_bytes("x", b"not-a-font".to_vec()).is_err());
    }
}
