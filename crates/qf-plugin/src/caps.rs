//! Capability string constants (manifest + host enforcement).

/// Register custom block types.
pub const CAP_BLOCK_CUSTOM_REGISTER: &str = "block.custom.register";
/// Transform export IR / formats.
pub const CAP_EXPORT_TRANSFORM: &str = "export.transform";
/// Contribute search providers.
pub const CAP_SEARCH_PROVIDER: &str = "search.provider";
/// AI provider (still requires content permission elsewhere).
pub const CAP_AI_PROVIDER: &str = "ai.provider";
/// Theme tokens.
pub const CAP_THEME_PROVIDER: &str = "theme.provider";
/// Read document projection via host.
pub const CAP_DOCUMENT_READ: &str = "document.read";
/// Propose operations (not direct apply).
pub const CAP_DOCUMENT_PROPOSE_OPS: &str = "document.propose_ops";
/// Replace unknown payloads (dangerous).
pub const CAP_UNKNOWN_REPLACE: &str = "document.unknown.replace";
/// UI panel contribution.
pub const CAP_UI_PANEL: &str = "ui.panel";
