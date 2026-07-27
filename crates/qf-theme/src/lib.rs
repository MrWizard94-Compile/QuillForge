//! Theme tokens for UI and render.

use qf_core::ColorId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
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

impl Theme {
    pub fn light_default() -> Self {
        let mut colors = BTreeMap::new();
        colors.insert(
            0,
            Rgba {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
        );
        colors.insert(
            1,
            Rgba {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
        );
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

    pub fn resolve(&self, id: ColorId) -> Option<&Rgba> {
        self.colors.colors.get(&id.0)
    }
}
