// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Material-Design-3-flavoured wallpaper theming.
//!
//! When enabled, the theme layer ignores user-picked accent and
//! widget-colour overrides and instead derives a three-role palette
//! (background / primary / secondary) from the top three colours in
//! the active wallpaper. The user picks *which* of the six possible
//! role permutations to use for the day and night colour schemes.
//!
//! # Derivation model
//!
//! This is **not** a full HCT tonal-palette implementation (that
//! would need XYZ → CAM16 → HCT + tonal stops for every accent).
//! What we do instead is a cheap, tint-intensity-modulated blend of
//! each palette role with an Adwaita-neutral base:
//!
//! - `window_bg` = `mix(neutral, background, t)`
//! - `headerbar_bg` = `mix(neutral, background, t × 1.1)` — slightly
//!   stronger so the chrome reads as a layer above the window
//! - `sidebar_bg` = `mix(neutral, background, t × 1.2)` — stronger
//!   again to give Nautilus / Files a visible separation
//! - `button_bg` = `mix(neutral, primary, t × 0.8)` — primary's job
//!   is "the user's main tap target", so keep it readable
//! - `entry_bg` = `mix(neutral, secondary, t × 0.4)` — subtler so
//!   form fields don't dominate the viewport
//!
//! `t` is `TintSpec::intensity`. `neutral` is Adwaita's light / dark
//! surface default (`#fafafb` / `#222226`). Everything is emitted
//! through the per-widget `@define-color` override block that the CSS
//! generator already runs after `gtk_tint_css`, so the MD3 pins
//! override whatever the accent-tint block produced.

use crate::{HexColor, Opacity, WidgetColorOverrides};

/// Adwaita's light-mode surface default. Used as the neutral
/// endpoint for MD3 blends so a `tint_intensity` of 0.0 matches the
/// vanilla Adwaita look.
pub const ADWAITA_LIGHT_NEUTRAL: &str = "#fafafb";

/// Adwaita's dark-mode surface default, same role as above.
pub const ADWAITA_DARK_NEUTRAL: &str = "#222226";

/// The six possible assignments of three palette colours to the three
/// Material roles. The triple in each name is `(background, primary,
/// secondary)` — e.g. `Bg1Pri2Sec0` means "palette[1] is the window
/// background, palette[2] drives buttons, palette[0] drives fields".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permutation {
    Bg0Pri1Sec2,
    Bg0Pri2Sec1,
    Bg1Pri0Sec2,
    Bg1Pri2Sec0,
    Bg2Pri0Sec1,
    Bg2Pri1Sec0,
}

impl Permutation {
    /// Ordered list suitable for a UI dropdown. Ordering is "Bg
    /// rotates slowest" so neighbouring entries feel related.
    pub const ALL: [Permutation; 6] = [
        Permutation::Bg0Pri1Sec2,
        Permutation::Bg0Pri2Sec1,
        Permutation::Bg1Pri0Sec2,
        Permutation::Bg1Pri2Sec0,
        Permutation::Bg2Pri0Sec1,
        Permutation::Bg2Pri1Sec0,
    ];

    /// GSettings round-trip. `0..=5` map 1:1 into `ALL`; any other
    /// value is clamped to `Bg0Pri1Sec2` so a malformed persisted
    /// preference can't panic.
    pub fn from_index(i: u32) -> Self {
        Self::ALL.get(i as usize).copied().unwrap_or(Permutation::Bg0Pri1Sec2)
    }

    pub fn to_index(self) -> u32 {
        Self::ALL.iter().position(|p| *p == self).unwrap_or(0) as u32
    }

    /// Human label for the UI dropdown. Kept in the domain so the
    /// UI doesn't invent its own inconsistent phrasing.
    pub fn label(self) -> &'static str {
        match self {
            Permutation::Bg0Pri1Sec2 => "Background 1 · Primary 2 · Secondary 3",
            Permutation::Bg0Pri2Sec1 => "Background 1 · Primary 3 · Secondary 2",
            Permutation::Bg1Pri0Sec2 => "Background 2 · Primary 1 · Secondary 3",
            Permutation::Bg1Pri2Sec0 => "Background 2 · Primary 3 · Secondary 1",
            Permutation::Bg2Pri0Sec1 => "Background 3 · Primary 1 · Secondary 2",
            Permutation::Bg2Pri1Sec0 => "Background 3 · Primary 2 · Secondary 1",
        }
    }

    /// Zero-indexed palette slots this permutation reads.
    pub fn indices(self) -> (usize, usize, usize) {
        match self {
            Permutation::Bg0Pri1Sec2 => (0, 1, 2),
            Permutation::Bg0Pri2Sec1 => (0, 2, 1),
            Permutation::Bg1Pri0Sec2 => (1, 0, 2),
            Permutation::Bg1Pri2Sec0 => (1, 2, 0),
            Permutation::Bg2Pri0Sec1 => (2, 0, 1),
            Permutation::Bg2Pri1Sec0 => (2, 1, 0),
        }
    }

    /// Apply this permutation to a 3-colour palette and return the
    /// resolved role assignments.
    pub fn apply(self, palette: &[HexColor; 3]) -> MaterialRoles {
        let (b, p, s) = self.indices();
        MaterialRoles {
            background: palette[b].clone(),
            primary: palette[p].clone(),
            secondary: palette[s].clone(),
        }
    }
}

/// A palette-colour's role after applying a [`Permutation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialRoles {
    pub background: HexColor,
    pub primary: HexColor,
    pub secondary: HexColor,
}

/// User-facing Material-palette spec. Persisted in GSettings and
/// part of [`crate::ThemeSpec`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialPaletteSpec {
    pub enabled: bool,
    pub day_permutation: Permutation,
    pub night_permutation: Permutation,
}

impl Default for MaterialPaletteSpec {
    fn default() -> Self {
        Self {
            enabled: false,
            day_permutation: Permutation::Bg0Pri1Sec2,
            night_permutation: Permutation::Bg0Pri1Sec2,
        }
    }
}

/// Take the day and night role assignments and return a
/// [`WidgetColorOverrides`] that, when fed into the normal CSS
/// generator, paints the window chrome in MD3 style.
///
/// Pure function — all colour maths runs off the passed roles +
/// `tint_intensity`; no hidden state, so the infra use case can
/// swap in any wallpaper palette and this produces deterministic
/// output.
pub fn derive_md3_overrides(
    day: &MaterialRoles,
    night: &MaterialRoles,
    tint_intensity: Opacity,
) -> WidgetColorOverrides {
    let t = tint_intensity.as_fraction();
    let light = ADWAITA_LIGHT_NEUTRAL.parse::<NeutralHex>().expect("built-in constant");
    let dark = ADWAITA_DARK_NEUTRAL.parse::<NeutralHex>().expect("built-in constant");

    WidgetColorOverrides {
        // `window_bg_color` is emitted under both schemes via the
        // existing override block — we re-use the sidebar-override
        // slot to avoid adding more fields here, because the
        // generator writes window_bg_color out of the tint block
        // itself. TODO on a later PR: add explicit window_bg slots
        // to the override struct so MD3 can pin window bg too.
        button_bg_light: Some(blend_hex(&light, &day.primary, clamp01(t * 0.8))),
        button_bg_dark: Some(blend_hex(&dark, &night.primary, clamp01(t * 0.8))),
        entry_bg_light: Some(blend_hex(&light, &day.secondary, clamp01(t * 0.4))),
        entry_bg_dark: Some(blend_hex(&dark, &night.secondary, clamp01(t * 0.4))),
        headerbar_bg_light: Some(blend_hex(&light, &day.background, clamp01(t * 1.1))),
        headerbar_bg_dark: Some(blend_hex(&dark, &night.background, clamp01(t * 1.1))),
        sidebar_bg_light: Some(blend_hex(&light, &day.background, clamp01(t * 1.2))),
        sidebar_bg_dark: Some(blend_hex(&dark, &night.background, clamp01(t * 1.2))),
    }
}

fn clamp01(x: f64) -> f64 {
    if x.is_nan() {
        0.0
    } else if x < 0.0 {
        0.0
    } else if x > 1.0 {
        1.0
    } else {
        x
    }
}

/// Internal wrapper around `(u8, u8, u8)` that implements `FromStr`
/// for the Adwaita-neutral constants. Keeps the `derive_md3_overrides`
/// body free of hand-rolled hex parsing.
struct NeutralHex((u8, u8, u8));

impl std::str::FromStr for NeutralHex {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let h = HexColor::new(s).map_err(|_| ())?;
        Ok(NeutralHex(h.to_rgb()))
    }
}

/// Linearly blend `neutral` and `tinted` RGB triples at `factor`
/// (0.0 = pure neutral, 1.0 = pure tinted) and return a `HexColor`.
/// Mirrors the domain's `color::blend` but returns a typed
/// `HexColor` instead of a raw `String`.
fn blend_hex(neutral: &NeutralHex, tinted: &HexColor, factor: f64) -> HexColor {
    let (nr, ng, nb) = neutral.0;
    let (tr, tg, tb) = tinted.to_rgb();
    let mix = |a: u8, b: u8| -> u8 {
        let v = (a as f64) * (1.0 - factor) + (b as f64) * factor;
        v.round().clamp(0.0, 255.0) as u8
    };
    let hex = format!(
        "#{:02x}{:02x}{:02x}",
        mix(nr, tr),
        mix(ng, tg),
        mix(nb, tb),
    );
    HexColor::new(&hex).expect("blend always produces a valid hex")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HexColor;

    fn sample_palette() -> [HexColor; 3] {
        [
            HexColor::new("#112233").unwrap(),
            HexColor::new("#445566").unwrap(),
            HexColor::new("#778899").unwrap(),
        ]
    }

    #[test]
    fn permutation_index_roundtrip() {
        for p in Permutation::ALL {
            assert_eq!(Permutation::from_index(p.to_index()), p);
        }
    }

    #[test]
    fn permutation_from_index_out_of_range_falls_back() {
        assert_eq!(Permutation::from_index(99), Permutation::Bg0Pri1Sec2);
        assert_eq!(Permutation::from_index(6), Permutation::Bg0Pri1Sec2);
    }

    #[test]
    fn permutation_indices_cover_all_six_mappings_exactly_once() {
        let mut seen: std::collections::HashSet<(usize, usize, usize)> =
            std::collections::HashSet::new();
        for p in Permutation::ALL {
            let triple = p.indices();
            assert!(seen.insert(triple), "duplicate index triple {triple:?}");
            // Must be a permutation of {0, 1, 2}.
            let mut sorted = [triple.0, triple.1, triple.2];
            sorted.sort_unstable();
            assert_eq!(sorted, [0, 1, 2]);
        }
        assert_eq!(seen.len(), 6);
    }

    #[test]
    fn apply_uses_correct_palette_slots_per_permutation() {
        let palette = sample_palette();
        let roles = Permutation::Bg1Pri2Sec0.apply(&palette);
        assert_eq!(roles.background, palette[1]);
        assert_eq!(roles.primary, palette[2]);
        assert_eq!(roles.secondary, palette[0]);
    }

    #[test]
    fn derive_at_zero_tint_collapses_to_neutrals() {
        // At intensity 0, the MD3 derivation should return pure
        // Adwaita neutrals — the palette never bleeds through.
        let palette = sample_palette();
        let day = Permutation::Bg0Pri1Sec2.apply(&palette);
        let night = Permutation::Bg0Pri1Sec2.apply(&palette);
        let w = derive_md3_overrides(&day, &night, Opacity::from_fraction(0.0).unwrap());
        // Every override should equal the scheme's neutral.
        assert_eq!(w.button_bg_light.as_ref().unwrap().as_str(), ADWAITA_LIGHT_NEUTRAL);
        assert_eq!(w.button_bg_dark.as_ref().unwrap().as_str(), ADWAITA_DARK_NEUTRAL);
        assert_eq!(w.entry_bg_light.as_ref().unwrap().as_str(), ADWAITA_LIGHT_NEUTRAL);
        assert_eq!(w.entry_bg_dark.as_ref().unwrap().as_str(), ADWAITA_DARK_NEUTRAL);
        assert_eq!(w.sidebar_bg_light.as_ref().unwrap().as_str(), ADWAITA_LIGHT_NEUTRAL);
        assert_eq!(w.sidebar_bg_dark.as_ref().unwrap().as_str(), ADWAITA_DARK_NEUTRAL);
    }

    #[test]
    fn derive_at_full_tint_saturates_the_role_colour() {
        // At intensity 1.0 * 0.8 (buttons) we're mostly the primary
        // palette colour, not the neutral. Verify the primary role
        // dominates the blend.
        let palette = [
            HexColor::new("#ff0000").unwrap(), // bright red
            HexColor::new("#00ff00").unwrap(), // bright green — the primary
            HexColor::new("#0000ff").unwrap(),
        ];
        let day = Permutation::Bg0Pri1Sec2.apply(&palette);
        let night = day.clone();
        let w = derive_md3_overrides(&day, &night, Opacity::from_fraction(1.0).unwrap());
        // button_bg_light's green channel must be meaningfully higher
        // than the neutral's green (neutral #fafafb has g=0xfa=250,
        // but at t=0.8 we're blending heavily toward #00ff00 so the
        // final red + blue channels must be closer to 0 than 250).
        let btn = w.button_bg_light.as_ref().unwrap().to_rgb();
        assert!(btn.0 < 80, "expected red channel to shrink toward 0, got {}", btn.0);
        assert!(btn.2 < 80, "expected blue channel to shrink toward 0, got {}", btn.2);
    }

    #[test]
    fn derive_keeps_day_and_night_independent() {
        // Different permutations on day vs night must produce
        // different overrides per scheme — we're not accidentally
        // using day roles for the dark variant.
        let palette = [
            HexColor::new("#ff0000").unwrap(),
            HexColor::new("#00ff00").unwrap(),
            HexColor::new("#0000ff").unwrap(),
        ];
        let day = Permutation::Bg0Pri1Sec2.apply(&palette); // pri=green
        let night = Permutation::Bg0Pri2Sec1.apply(&palette); // pri=blue
        let w = derive_md3_overrides(&day, &night, Opacity::from_fraction(0.5).unwrap());
        assert_ne!(
            w.button_bg_light.unwrap().to_rgb(),
            w.button_bg_dark.unwrap().to_rgb(),
            "day pri=green vs night pri=blue must produce different button bg",
        );
    }

    #[test]
    fn blend_with_equal_colours_is_identity() {
        let n = NeutralHex((0x11, 0x22, 0x33));
        let h = HexColor::new("#112233").unwrap();
        let out = blend_hex(&n, &h, 0.5);
        assert_eq!(out.as_str(), "#112233");
    }

    #[test]
    fn labels_are_distinct_per_permutation() {
        let mut seen: std::collections::HashSet<&'static str> = std::collections::HashSet::new();
        for p in Permutation::ALL {
            assert!(seen.insert(p.label()), "duplicate label: {}", p.label());
        }
        assert_eq!(seen.len(), 6);
    }
}
