//! Lightweight import of Markdown / HTML / plain text into [`DocxImportBlock`] lists.
//!
//! Same intermediate shape as DOCX import so the journaled apply path is shared.
//! Dependency-light scanners (no full HTML/MD parsers).

use crate::docx_import::DocxImportBlock;

/// Parse Markdown into structural import blocks.
pub fn import_markdown(text: &str) -> Vec<DocxImportBlock> {
    let mut out = Vec::new();
    let mut lines = text.lines().peekable();
    let mut para = String::new();
    let mut in_code = false;
    let mut code = String::new();

    let flush_para = |out: &mut Vec<DocxImportBlock>, para: &mut String| {
        let t = para.trim().to_string();
        para.clear();
        if !t.is_empty() {
            out.push(DocxImportBlock::Paragraph { text: t });
        }
    };

    while let Some(line) = lines.next() {
        if in_code {
            if line.trim_start().starts_with("```") {
                in_code = false;
                out.push(DocxImportBlock::Code {
                    text: code.trim_end_matches('\n').to_string(),
                });
                code.clear();
            } else {
                code.push_str(line);
                code.push('\n');
            }
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            flush_para(&mut out, &mut para);
            in_code = true;
            continue;
        }
        if trimmed == "---" || trimmed == "***" {
            flush_para(&mut out, &mut para);
            out.push(DocxImportBlock::PageBreak);
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            flush_para(&mut out, &mut para);
            out.push(DocxImportBlock::Heading {
                level: 3,
                text: rest.to_string(),
            });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            flush_para(&mut out, &mut para);
            out.push(DocxImportBlock::Heading {
                level: 2,
                text: rest.to_string(),
            });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            flush_para(&mut out, &mut para);
            out.push(DocxImportBlock::Heading {
                level: 1,
                text: rest.to_string(),
            });
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("> ") {
            flush_para(&mut out, &mut para);
            out.push(DocxImportBlock::Quote {
                text: rest.to_string(),
            });
            continue;
        }
        if trimmed.is_empty() {
            flush_para(&mut out, &mut para);
            continue;
        }
        if !para.is_empty() {
            para.push(' ');
        }
        para.push_str(trimmed);
        let _ = lines;
    }
    if in_code {
        out.push(DocxImportBlock::Code {
            text: code.trim_end_matches('\n').to_string(),
        });
    }
    flush_para(&mut out, &mut para);
    if out.is_empty() {
        out.push(DocxImportBlock::Paragraph {
            text: String::new(),
        });
    }
    out
}

/// Parse a simple HTML document into import blocks (tag scan).
pub fn import_html(html: &str) -> Vec<DocxImportBlock> {
    let mut out = Vec::new();
    let mut rest = html;
    // Drop script/style coarsely.
    while let Some(i) = rest.find('<') {
        let before = &rest[..i];
        push_loose_text(&mut out, before);
        let after = &rest[i..];
        let Some(end) = after.find('>') else {
            break;
        };
        let tag = &after[..=end];
        rest = &after[end + 1..];
        let lower = tag.to_ascii_lowercase();
        if lower.starts_with("<script") || lower.starts_with("<style") {
            let close = if lower.starts_with("<script") {
                "</script>"
            } else {
                "</style>"
            };
            if let Some(c) = rest.to_ascii_lowercase().find(close) {
                rest = &rest[c + close.len()..];
            }
            continue;
        }
        if lower.starts_with("<br") || lower == "<hr>" || lower.starts_with("<hr ") {
            if lower.starts_with("<hr") {
                out.push(DocxImportBlock::PageBreak);
            } else if let Some(DocxImportBlock::Paragraph { text }) = out.last_mut() {
                if !text.is_empty() {
                    text.push('\n');
                }
            }
            continue;
        }
        // Opening structural tags: capture until matching close for simple cases.
        if let Some(level) = heading_level(&lower) {
            if let Some((inner, next)) = take_until_close(rest, &format!("h{level}")) {
                out.push(DocxImportBlock::Heading {
                    level,
                    text: strip_tags_and_unescape(inner).trim().to_string(),
                });
                rest = next;
            }
            continue;
        }
        // Order matters: `<pre` must be checked before `<p`.
        if lower.starts_with("<pre") {
            if let Some((inner, next)) = take_until_close(rest, "pre") {
                out.push(DocxImportBlock::Code {
                    text: strip_tags_and_unescape(inner),
                });
                rest = next;
            }
            continue;
        }
        if lower.starts_with("<blockquote") {
            if let Some((inner, next)) = take_until_close(rest, "blockquote") {
                out.push(DocxImportBlock::Quote {
                    text: strip_tags_and_unescape(inner).trim().to_string(),
                });
                rest = next;
            }
            continue;
        }
        if is_open_tag(&lower, "p") || is_open_tag(&lower, "div") {
            let name = if is_open_tag(&lower, "p") { "p" } else { "div" };
            if let Some((inner, next)) = take_until_close(rest, name) {
                let t = strip_tags_and_unescape(inner).trim().to_string();
                if !t.is_empty() {
                    out.push(DocxImportBlock::Paragraph { text: t });
                }
                rest = next;
            }
            continue;
        }
    }
    push_loose_text(&mut out, rest);
    if out.is_empty() {
        out.push(DocxImportBlock::Paragraph {
            text: String::new(),
        });
    }
    out
}

/// Plain text: paragraphs split on blank lines.
pub fn import_plain(text: &str) -> Vec<DocxImportBlock> {
    let mut out = Vec::new();
    let mut para = String::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !para.is_empty() {
                out.push(DocxImportBlock::Paragraph {
                    text: para.trim().to_string(),
                });
                para.clear();
            }
        } else {
            if !para.is_empty() {
                para.push('\n');
            }
            para.push_str(line);
        }
    }
    if !para.is_empty() {
        out.push(DocxImportBlock::Paragraph {
            text: para.trim().to_string(),
        });
    }
    if out.is_empty() {
        out.push(DocxImportBlock::Paragraph {
            text: text.to_string(),
        });
    }
    out
}

/// Detect format from path extension and import.
pub fn import_text_path(path: &std::path::Path) -> Result<Vec<DocxImportBlock>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let text = String::from_utf8_lossy(&bytes);
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    Ok(match ext.as_str() {
        "md" | "markdown" => import_markdown(&text),
        "html" | "htm" => import_html(&text),
        "txt" | "text" => import_plain(&text),
        other => {
            return Err(format!(
                "unsupported text import extension .{other} (use md|html|txt)"
            ));
        }
    })
}

fn push_loose_text(out: &mut Vec<DocxImportBlock>, s: &str) {
    let t = strip_tags_and_unescape(s).trim().to_string();
    if t.is_empty() {
        return;
    }
    out.push(DocxImportBlock::Paragraph { text: t });
}

fn heading_level(open_tag_lower: &str) -> Option<u8> {
    (1..=6u8).find(|&lvl| is_open_tag(open_tag_lower, &format!("h{lvl}")))
}

/// True for `<name`, `<name>`, `<name ...` but not `<namex`.
fn is_open_tag(open_tag_lower: &str, name: &str) -> bool {
    let p = format!("<{name}");
    if !open_tag_lower.starts_with(&p) {
        return false;
    }
    let rest = &open_tag_lower[p.len()..];
    rest.is_empty()
        || rest.starts_with('>')
        || rest.starts_with(' ')
        || rest.starts_with('/')
        || rest.starts_with('\n')
        || rest.starts_with('\t')
}

fn take_until_close<'a>(rest: &'a str, name: &str) -> Option<(&'a str, &'a str)> {
    let close = format!("</{name}>");
    let lower = rest.to_ascii_lowercase();
    let idx = lower.find(&close)?;
    Some((&rest[..idx], &rest[idx + close.len()..]))
}

fn strip_tags_and_unescape(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('<') {
        out.push_str(&rest[..i]);
        let after = &rest[i..];
        if let Some(end) = after.find('>') {
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    out.push_str(rest);
    xml_unescape(&out)
}

fn xml_unescape(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_headings_and_code() {
        let blocks = import_markdown(
            "# Title\n\nHello body\n\n```\nfn x() {}\n```\n\n> quoted\n\n---\n\nAfter",
        );
        assert!(matches!(
            &blocks[0],
            DocxImportBlock::Heading { level: 1, text } if text == "Title"
        ));
        assert!(blocks
            .iter()
            .any(|b| matches!(b, DocxImportBlock::Paragraph { text } if text.contains("Hello"))));
        assert!(blocks
            .iter()
            .any(|b| matches!(b, DocxImportBlock::Code { text } if text.contains("fn x"))));
        assert!(blocks
            .iter()
            .any(|b| matches!(b, DocxImportBlock::Quote { text } if text.contains("quoted"))));
        assert!(blocks
            .iter()
            .any(|b| matches!(b, DocxImportBlock::PageBreak)));
    }

    #[test]
    fn html_basic_structure() {
        let blocks =
            import_html("<html><body><h1>Hi</h1><p>Para &amp; co</p><pre>code</pre></body></html>");
        assert!(matches!(
            &blocks[0],
            DocxImportBlock::Heading { level: 1, text } if text == "Hi"
        ));
        assert!(blocks.iter().any(
            |b| matches!(b, DocxImportBlock::Paragraph { text } if text.contains("Para & co"))
        ));
        assert!(blocks
            .iter()
            .any(|b| matches!(b, DocxImportBlock::Code { text } if text.contains("code"))));
    }

    #[test]
    fn plain_paragraphs() {
        let blocks = import_plain("one\ntwo\n\nthree");
        assert_eq!(blocks.len(), 2);
        assert!(matches!(
            &blocks[0],
            DocxImportBlock::Paragraph { text } if text.contains("one") && text.contains("two")
        ));
    }
}
