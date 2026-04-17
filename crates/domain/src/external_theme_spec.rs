// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Cross-application theme contract.
//!
//! `ExternalThemeSpec` is the minimal, stable surface that third-party
//! applications (Chromium-family browsers, VS Code-family editors, ...)
//! consume. It is a narrow projection of the rich GNOME-side `ThemeSpec`
//! so that changes to GNOME-only concepts (panel radius, dash opacity,
//! ...) don't ripple out into unrelated adapters.

use crate::HexColor;

/// Light or dark appearance. Mirrors the GNOME `color-scheme` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorScheme {
    Light,
    Dark,
}

impl ColorScheme {
    /// Parse the GSettings `color-scheme` string.
    /// `"prefer-dark"` → Dark, `"prefer-light"` or `"default"` → Light.
    pub fn from_gsettings(value: &str) -> Self {
        if value.eq_ignore_ascii_case("prefer-dark") {
            Self::Dark
        } else {
            Self::Light
        }
    }

    pub fn is_dark(self) -> bool {
        matches!(self, Self::Dark)
    }
}

/// The projection of GNOME theme state that external applications can
/// meaningfully consume: accent color, a background tint, and the
/// light/dark preference.
#[derive(Debug, Clone)]
pub struct ExternalThemeSpec {
    pub accent: HexColor,
    pub panel_tint: HexColor,
    pub color_scheme: ColorScheme,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_scheme_parses_gsettings() {
        assert_eq!(ColorScheme::from_gsettings("prefer-dark"), ColorScheme::Dark);
        assert_eq!(ColorScheme::from_gsettings("prefer-light"), ColorScheme::Light);
        assert_eq!(ColorScheme::from_gsettings("default"), ColorScheme::Light);
        assert_eq!(ColorScheme::from_gsettings(""), ColorScheme::Light);
    }
}
