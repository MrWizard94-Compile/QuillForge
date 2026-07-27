//! Deterministic export pipelines.

use qf_blocks::BlockBody;
use qf_kernel::CanonicalState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExportFormat {
    Markdown,
    Html,
    Pdf,
    PlainText,
    Docx,
}

#[derive(Clone, Debug, Default)]
pub struct ExportConfig {
    pub include_timestamps: bool,
}

#[derive(Clone, Debug)]
pub struct ExportRequest {
    pub format: ExportFormat,
    pub config: ExportConfig,
}

#[derive(Clone, Debug)]
pub struct ExportOutput {
    pub bytes: Vec<u8>,
    pub content_hash: qf_core::Hash256,
}

pub trait Exporter: Send + Sync {
    fn export(&self, state: &CanonicalState, req: &ExportRequest) -> Result<ExportOutput, String>;
}

pub struct CoreExporter;

impl Exporter for CoreExporter {
    fn export(&self, state: &CanonicalState, req: &ExportRequest) -> Result<ExportOutput, String> {
        let text = match req.format {
            ExportFormat::Markdown | ExportFormat::PlainText => export_text(state, req.format),
            ExportFormat::Html => export_html(state),
            ExportFormat::Pdf | ExportFormat::Docx => {
                // Foundation: textual placeholder; full writers in later phases.
                export_text(state, ExportFormat::PlainText)
            }
        };
        let bytes = text.into_bytes();
        let content_hash = qf_core::Hash256::sha256(&bytes);
        Ok(ExportOutput {
            bytes,
            content_hash,
        })
    }
}

fn export_text(state: &CanonicalState, format: ExportFormat) -> String {
    let mut out = String::new();
    for sid in &state.document.section_order {
        let Some(sec) = state.document.sections.get(sid) else {
            continue;
        };
        for bid in &sec.blocks {
            let Some(block) = state.document.blocks.get(bid) else {
                continue;
            };
            match &block.body {
                BlockBody::Heading(h) => {
                    if format == ExportFormat::Markdown {
                        out.push_str(&"#".repeat(h.level as usize));
                        out.push(' ');
                    }
                    out.push_str(&h.spans.iter().map(|s| s.text.as_str()).collect::<String>());
                    out.push('\n');
                }
                BlockBody::Paragraph(p) => {
                    out.push_str(&p.spans.iter().map(|s| s.text.as_str()).collect::<String>());
                    out.push('\n');
                }
                BlockBody::Unknown(u) => {
                    if format == ExportFormat::Markdown {
                        out.push_str(&format!(
                            "\n<!-- qf-unknown type=\"{}\" payload-b64=\"{}\" -->\n",
                            u.original_type,
                            b64(&u.original_payload)
                        ));
                    } else {
                        out.push_str(&format!("[unknown:{}]\n", u.original_type));
                    }
                }
                BlockBody::PageBreak => out.push_str("\n---\n"),
                _ => {
                    if let Some(t) = state.document.project_text(*bid) {
                        out.push_str(&t);
                        out.push('\n');
                    }
                }
            }
        }
    }
    out
}

fn export_html(state: &CanonicalState) -> String {
    let mut out = String::from("<!DOCTYPE html><html><body>\n");
    for sid in &state.document.section_order {
        let Some(sec) = state.document.sections.get(sid) else {
            continue;
        };
        for bid in &sec.blocks {
            let Some(block) = state.document.blocks.get(bid) else {
                continue;
            };
            match &block.body {
                BlockBody::Paragraph(p) => {
                    let t: String = p.spans.iter().map(|s| s.text.as_str()).collect();
                    out.push_str(&format!("<p>{}</p>\n", html_escape(&t)));
                }
                BlockBody::Unknown(u) => {
                    out.push_str(&format!(
                        "<div data-qf-unknown-type=\"{}\" data-qf-payload-b64=\"{}\"></div>\n",
                        html_escape(&u.original_type),
                        b64(&u.original_payload)
                    ));
                }
                _ => {}
            }
        }
    }
    out.push_str("</body></html>\n");
    out
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn b64(data: &[u8]) -> String {
    // Minimal base64 for foundation (not constant-time; fine for export markers).
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (a << 16) | (b << 8) | c;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}
