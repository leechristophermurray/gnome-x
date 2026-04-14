// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Theme specification — version-independent value objects for theming.
//!
//! These types capture *what* the user wants (radii, opacity, tint) without
//! any knowledge of *how* it maps to CSS selectors on a given GNOME version.

use crate::DomainError;

/// A validated border-radius in pixels (0–48).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radius(f64);

impl Radius {
    pub fn new(px: f64) -> Result<Self, DomainError> {
        if !(0.0..=48.0).contains(&px) {
            return Err(DomainError::InvalidRadius(px));
        }
        Ok(Self(px))
    }

    pub fn as_px(&self) -> f64 {
        self.0
    }

    pub fn as_i32(&self) -> i32 {
        self.0 as i32
    }
}

/// A validated opacity (stored as 0.0–1.0 fraction, constructed from 0–100%).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opacity(f64);

impl Opacity {
    pub fn from_percent(pct: f64) -> Result<Self, DomainError> {
        if !(0.0..=100.0).contains(&pct) {
            return Err(DomainError::InvalidOpacity(pct));
        }
        Ok(Self(pct / 100.0))
    }

    pub fn from_fraction(f: f64) -> Result<Self, DomainError> {
        if !(0.0..=1.0).contains(&f) {
            return Err(DomainError::InvalidOpacity(f * 100.0));
        }
        Ok(Self(f))
    }

    pub fn as_fraction(&self) -> f64 {
        self.0
    }

    pub fn as_percent(&self) -> f64 {
        self.0 * 100.0
    }
}

/// A validated `#rrggbb` hex color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexColor(String);

impl HexColor {
    pub fn new(hex: &str) -> Result<Self, DomainError> {
        let hex = hex.trim();
        if hex.len() != 7
            || !hex.starts_with('#')
            || !hex[1..].chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(DomainError::InvalidColor(hex.into()));
        }
        Ok(Self(hex.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_rgb(&self) -> (u8, u8, u8) {
        let r = u8::from_str_radix(&self.0[1..3], 16).unwrap_or(0);
        let g = u8::from_str_radix(&self.0[3..5], 16).unwrap_or(0);
        let b = u8::from_str_radix(&self.0[5..7], 16).unwrap_or(0);
        (r, g, b)
    }
}

/// Panel customization values.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelSpec {
    pub radius: Radius,
    pub opacity: Opacity,
    pub tint: HexColor,
}

/// Dash customization values.
#[derive(Debug, Clone, PartialEq)]
pub struct DashSpec {
    pub opacity: Opacity,
}

/// Accent tint customization values.
#[derive(Debug, Clone, PartialEq)]
pub struct TintSpec {
    pub accent_hex: HexColor,
    pub intensity: Opacity,
}

/// A complete, version-independent theme specification.
///
/// Contains all the values the user has set via the Theme Builder.
/// The version-specific CSS adapters in the infra layer interpret these
/// values using the correct selectors for the running GNOME version.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSpec {
    pub window_radius: Radius,
    pub element_radius: Radius,
    pub panel: PanelSpec,
    pub dash: DashSpec,
    pub tint: TintSpec,
    pub overview_blur: bool,
}

impl ThemeSpec {
    pub fn defaults() -> Self {
        Self {
            window_radius: Radius(12.0),
            element_radius: Radius(6.0),
            panel: PanelSpec {
                radius: Radius(0.0),
                opacity: Opacity(0.8),
                tint: HexColor("#1a1a1e".into()),
            },
            dash: DashSpec {
                opacity: Opacity(0.7),
            },
            tint: TintSpec {
                accent_hex: HexColor("#3584e4".into()),
                intensity: Opacity(0.05),
            },
            overview_blur: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radius_valid() {
        assert!(Radius::new(0.0).is_ok());
        assert!(Radius::new(12.0).is_ok());
        assert!(Radius::new(48.0).is_ok());
    }

    #[test]
    fn radius_invalid() {
        assert!(Radius::new(-1.0).is_err());
        assert!(Radius::new(49.0).is_err());
    }

    #[test]
    fn opacity_valid() {
        assert!(Opacity::from_percent(0.0).is_ok());
        assert!(Opacity::from_percent(50.0).is_ok());
        assert!(Opacity::from_percent(100.0).is_ok());
        assert_eq!(Opacity::from_percent(80.0).unwrap().as_fraction(), 0.8);
    }

    #[test]
    fn opacity_invalid() {
        assert!(Opacity::from_percent(-1.0).is_err());
        assert!(Opacity::from_percent(101.0).is_err());
    }

    #[test]
    fn hex_color_valid() {
        let c = HexColor::new("#3584e4").unwrap();
        assert_eq!(c.as_str(), "#3584e4");
        assert_eq!(c.to_rgb(), (0x35, 0x84, 0xe4));
    }

    #[test]
    fn hex_color_invalid() {
        assert!(HexColor::new("red").is_err());
        assert!(HexColor::new("#12345").is_err());
        assert!(HexColor::new("#gggggg").is_err());
    }

    #[test]
    fn theme_spec_defaults() {
        let spec = ThemeSpec::defaults();
        assert_eq!(spec.window_radius.as_i32(), 12);
        assert_eq!(spec.panel.opacity.as_percent(), 80.0);
    }
}
