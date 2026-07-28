//! Theme tokens for UI and render — includes WPAI forge dark brand theme.

use qf_core::{
    ColorId, COLOR_ASH, COLOR_BASALT, COLOR_COAL, COLOR_EMBER, COLOR_IRON, COLOR_MOLTEN,
    PRODUCT_NAME, STUDIO_SHORT,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn from_tuple(t: (u8, u8, u8)) -> Self {
        Self::rgb(t.0, t.1, t.2)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ColorPalette {
    pub colors: BTreeMap<u32, Rgba>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TypeScale {
    pub body_pt: f32,
    pub h1_pt: f32,
    pub h2_pt: f32,
    pub code_pt: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Theme {
    pub name: String,
    pub colors: ColorPalette,
    pub typography: TypeScale,
}

/// Semantic color roles used by UI/render (stable ids).
pub mod role {
    pub const TEXT: u32 = 0;
    pub const SURFACE: u32 = 1;
    pub const ACCENT: u32 = 2;
    pub const ACCENT_HOT: u32 = 3;
    pub const MUTED: u32 = 4;
    pub const SURFACE_RAISED: u32 = 5;
}

impl Theme {
    /// Neutral light theme (tests / accessibility fallback).
    pub fn light_default() -> Self {
        let mut colors = BTreeMap::new();
        colors.insert(role::TEXT, Rgba::rgb(0, 0, 0));
        colors.insert(role::SURFACE, Rgba::rgb(255, 255, 255));
        colors.insert(role::ACCENT, Rgba::rgb(0x33, 0x66, 0xcc));
        colors.insert(role::ACCENT_HOT, Rgba::rgb(0x22, 0x44, 0xaa));
        colors.insert(role::MUTED, Rgba::rgb(0x66, 0x66, 0x66));
        colors.insert(role::SURFACE_RAISED, Rgba::rgb(0xf5, 0xf5, 0xf5));
        Self {
            name: "light".into(),
            colors: ColorPalette { colors },
            typography: TypeScale {
                body_pt: 12.0,
                h1_pt: 24.0,
                h2_pt: 18.0,
                code_pt: 11.0,
            },
        }
    }

    /// Official WPAI forge aesthetic: coal / basalt surfaces, ember accents.
    ///
    /// Visual law: `brand/` and studio Brand kit (ember-on-black, industrial forge).
    pub fn wpai_forge_dark() -> Self {
        let mut colors = BTreeMap::new();
        colors.insert(role::TEXT, Rgba::from_tuple(COLOR_ASH));
        colors.insert(role::SURFACE, Rgba::from_tuple(COLOR_COAL));
        colors.insert(role::ACCENT, Rgba::from_tuple(COLOR_EMBER));
        colors.insert(role::ACCENT_HOT, Rgba::from_tuple(COLOR_MOLTEN));
        colors.insert(role::MUTED, Rgba::from_tuple(COLOR_IRON));
        colors.insert(role::SURFACE_RAISED, Rgba::from_tuple(COLOR_BASALT));
        Self {
            name: format!("{STUDIO_SHORT}-forge-dark"),
            colors: ColorPalette { colors },
            typography: TypeScale {
                body_pt: 12.0,
                h1_pt: 26.0,
                h2_pt: 18.0,
                code_pt: 11.0,
            },
        }
    }

    /// Document workspace chrome: paper page + light rails (Word-class familiarity).
    ///
    /// Body text black on white; rails soft gray; **accent remains WPAI ember**
    /// (not Office blue). See `docs/28-product-surface.md`.
    pub fn document_workspace_light() -> Self {
        let mut colors = BTreeMap::new();
        colors.insert(role::TEXT, Rgba::rgb(0x1a, 0x1a, 0x1a));
        colors.insert(role::SURFACE, Rgba::rgb(0xf3, 0xf2, 0xf1)); // app rail
        colors.insert(role::ACCENT, Rgba::from_tuple(COLOR_EMBER));
        colors.insert(role::ACCENT_HOT, Rgba::from_tuple(COLOR_MOLTEN));
        colors.insert(role::MUTED, Rgba::rgb(0x60, 0x5e, 0x5c));
        colors.insert(role::SURFACE_RAISED, Rgba::rgb(0xff, 0xff, 0xff)); // paper
        Self {
            name: "document-workspace-light".into(),
            colors: ColorPalette { colors },
            typography: TypeScale {
                body_pt: 11.0,
                h1_pt: 20.0,
                h2_pt: 16.0,
                code_pt: 10.0,
            },
        }
    }

    /// Default **editor shell** theme: document workspace (Word-class paper).
    /// Brand/about/splash continue to use [`Theme::wpai_forge_dark`].
    pub fn product_default() -> Self {
        Self::document_workspace_light()
    }

    pub fn display_label(&self) -> String {
        format!("{PRODUCT_NAME} · {STUDIO_SHORT} · {}", self.name)
    }

    pub fn resolve(&self, id: ColorId) -> Option<&Rgba> {
        self.colors.colors.get(&id.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wpai_forge_theme_uses_ember_on_coal() {
        let t = Theme::wpai_forge_dark();
        assert!(t.name.contains("WPAI") || t.name.contains("forge"));
        let surface = t.resolve(ColorId(role::SURFACE)).unwrap();
        assert_eq!((surface.r, surface.g, surface.b), COLOR_COAL);
        let accent = t.resolve(ColorId(role::ACCENT)).unwrap();
        assert_eq!((accent.r, accent.g, accent.b), COLOR_EMBER);
        assert!(t.display_label().contains(STUDIO_SHORT));
    }

    #[test]
    fn product_default_is_document_workspace() {
        let t = Theme::product_default();
        assert_eq!(t.name, Theme::document_workspace_light().name);
        let paper = t.resolve(ColorId(role::SURFACE_RAISED)).unwrap();
        assert_eq!((paper.r, paper.g, paper.b), (255, 255, 255));
        let accent = t.resolve(ColorId(role::ACCENT)).unwrap();
        assert_eq!((accent.r, accent.g, accent.b), COLOR_EMBER);
    }

    #[test]
    fn document_workspace_is_not_office_blue() {
        let t = Theme::document_workspace_light();
        let accent = t.resolve(ColorId(role::ACCENT)).unwrap();
        // Office-like blue would be ~0x2b579a — we require WPAI ember.
        assert_ne!(accent.b, accent.r);
        assert_eq!(accent.r, COLOR_EMBER.0);
    }
}
