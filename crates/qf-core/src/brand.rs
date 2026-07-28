//! WPAI product branding constants for Quillforge.
//!
//! Source of visual/voice law: repository `brand/` (synced from Wizard Productions AI Studio).
//! Product name remains Quillforge; studio identity is WPAI.

/// Legal studio name.
pub const STUDIO_LEGAL: &str = "Wizard Productions AI Studio";

/// Short studio mark (preferred in product chrome).
pub const STUDIO_SHORT: &str = "WPAI";

/// Expanded short form used in formal copy.
pub const STUDIO_MARK: &str = "W.P.A.I";

/// Product name.
pub const PRODUCT_NAME: &str = "Quillforge";

/// Product line within the studio.
pub const PRODUCT_DIVISION: &str = "Software";

/// Studio tagline.
pub const STUDIO_TAGLINE: &str = "Forging the future of creative media";

/// Brand thesis (use sparingly, never dilute).
pub const STUDIO_THESIS: &str = "AI is in the name. The wizard is in the work.";

/// Human-directed disclosure badge line.
pub const HUMAN_DIRECTED_BADGE: &str = "Human-directed · AI-assisted";

/// Footer / disclosure (verbatim brand voice).
pub const STUDIO_DISCLOSURE: &str =
    "Made in open collaboration with AI tools; written into shape, directed, and finished by Rob Bulkley. AI is in the name — the wizard is in the work.";

/// Founder display credit.
pub const FOUNDER_CREDIT: &str = "Rob Bulkley (Mrwizard94)";

/// Public studio site.
pub const STUDIO_SITE: &str = "https://wpaistudio.net";

/// Storefront.
pub const STUDIO_STORE: &str = "https://wpaistudio.gumroad.com";

/// Contact.
pub const STUDIO_EMAIL: &str = "rob@wpaistudio.net";

/// Application identity string written into package metadata.
pub fn app_identity() -> String {
    format!(
        "{PRODUCT_NAME}/{APP_SEMVER} ({STUDIO_SHORT} {PRODUCT_DIVISION})"
    )
}

/// Semantic product version string (workspace crate version).
pub const APP_SEMVER: &str = env!("CARGO_PKG_VERSION");

/// One-line CLI banner.
pub fn cli_banner() -> String {
    format!("{PRODUCT_NAME} — {STUDIO_SHORT} · {STUDIO_TAGLINE}")
}

/// Forge palette: ember primary (sRGB).
pub const COLOR_EMBER: (u8, u8, u8) = (0xff, 0x7a, 0x26);
/// Forge palette: molten secondary.
pub const COLOR_MOLTEN: (u8, u8, u8) = (0xe8, 0x45, 0x1c);
/// Forge palette: coal black.
pub const COLOR_COAL: (u8, u8, u8) = (0x0a, 0x07, 0x06);
/// Forge palette: basalt near-black.
pub const COLOR_BASALT: (u8, u8, u8) = (0x12, 0x10, 0x0e);
/// Forge palette: ash text (high contrast on coal).
pub const COLOR_ASH: (u8, u8, u8) = (0xe8, 0xe2, 0xdc);
/// Forge palette: iron muted.
pub const COLOR_IRON: (u8, u8, u8) = (0x6b, 0x63, 0x5c);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brand_strings_nonempty_and_wpai_marked() {
        assert_eq!(STUDIO_SHORT, "WPAI");
        assert_eq!(STUDIO_MARK, "W.P.A.I");
        assert_eq!(PRODUCT_NAME, "Quillforge");
        assert!(app_identity().contains(STUDIO_SHORT));
        assert!(cli_banner().contains(STUDIO_SHORT));
        assert!(STUDIO_THESIS.contains("wizard"));
    }
}
