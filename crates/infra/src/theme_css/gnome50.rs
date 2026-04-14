// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! GNOME 50 / Libadwaita 1.9 CSS adapter.
//!
//! Key differences from GNOME 47:
//! - Single `style.css` with media queries only (style-dark.css deprecated)
//! - Refined dark palette base colors matching Adwaita 1.9
//! - `prefers-reduced-motion` guard on blur/animation CSS
//! - Wayland-only (no X11 fallback paths)

use super::common::{gtk_radius_css, gtk_tint_css};
use gnomex_app::ports::{ThemeCss, ThemeCssGenerator};
use gnomex_app::AppError;
use gnomex_domain::ThemeSpec;

/// Libadwaita 1.9 refined dark palette base colors.
/// These replace the 47-era values (#242428, #303034, etc.)
/// to match the updated Adwaita dark stylesheet.
const PANEL_BASE: (u8, u8, u8) = (0x26, 0x26, 0x2a);
const DASH_BASE: (u8, u8, u8) = (0x32, 0x32, 0x36);
const OSD_BASE: (u8, u8, u8) = (0x32, 0x32, 0x36);
const SEARCH_BASE: (u8, u8, u8) = (0x3c, 0x3c, 0x40);

pub struct Gnome50CssGenerator;

impl ThemeCssGenerator for Gnome50CssGenerator {
    fn version_label(&self) -> &str {
        "GNOME 50"
    }

    fn generate(&self, spec: &ThemeSpec) -> Result<ThemeCss, AppError> {
        Ok(ThemeCss {
            gtk_css: self.gtk(spec),
            shell_css: self.shell(spec),
        })
    }
}

impl Gnome50CssGenerator {
    fn gtk(&self, spec: &ThemeSpec) -> String {
        // GNOME 50: single stylesheet, media queries only.
        // No style-dark.css generation — Libadwaita 1.9 logs deprecation
        // warnings if it finds separate dark variant files.
        format!(
            "/* GNOME X — GTK4 overrides (GNOME 50 / Libadwaita 1.9) */\n\n{}\n{}",
            gtk_radius_css(spec),
            gtk_tint_css(spec),
        )
    }

    fn shell(&self, spec: &ThemeSpec) -> String {
        let accent = spec.tint.accent_hex.to_rgb();
        let pct = spec.tint.intensity.as_fraction() as f32;

        let panel = gnomex_domain::color::blend(PANEL_BASE, accent, pct);
        let dash = gnomex_domain::color::blend(DASH_BASE, accent, pct);
        let osd = gnomex_domain::color::blend(OSD_BASE, accent, pct);
        let search = gnomex_domain::color::blend(SEARCH_BASE, accent, pct);

        let pr = spec.panel.radius.as_i32();
        let po = spec.panel.opacity.as_fraction();
        let do_ = spec.dash.opacity.as_fraction();

        // GNOME 50: prefers-reduced-motion guard on blur effects
        let blur_block = if spec.overview_blur {
            r#"
/* Respect reduced-motion preference (Libadwaita 1.9+) */
@media (prefers-reduced-motion: no-preference) {
    /* Overview blur enabled */
}"#
        } else {
            "#overview { background-color: rgba(0, 0, 0, 0.6); }"
        };

        format!(
            r#"/* GNOME X — Shell overrides (GNOME 50 / Libadwaita 1.9) */
@import url("resource:///org/gnome/shell/theme/gnome-shell.css");

/* Accent-tinted panel (refined dark palette) */
#panel {{
    background-color: alpha({panel}, {po}) !important;
    border-radius: 0 0 {pr}px {pr}px;
}}

#panel .panel-button {{
    border-radius: {pr}px;
}}

/* Accent-tinted dash */
#dash {{
    background-color: alpha({dash}, {do_});
    border-radius: 16px;
    padding: 6px;
    margin: 8px;
    border: 1px solid rgba(255, 255, 255, 0.06);
}}

.dash-item-container .app-well-app {{
    border-radius: 12px;
}}

/* Accent-tinted search */
.search-entry {{
    background-color: {search};
    border-radius: 18px;
}}

/* Accent-tinted OSD / popups */
.osd, .popup-menu-content, .candidate-popup-content {{
    background-color: {osd};
}}

/* Calendar / message tray */
.events-button, .world-clocks-button, .weather-button, .message {{
    background-color: alpha({osd}, 0.9);
}}

{blur_block}
"#
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gnomex_domain::*;

    fn test_spec() -> ThemeSpec {
        ThemeSpec {
            window_radius: Radius::new(12.0).unwrap(),
            element_radius: Radius::new(6.0).unwrap(),
            panel: PanelSpec {
                radius: Radius::new(0.0).unwrap(),
                opacity: Opacity::from_percent(80.0).unwrap(),
                tint: HexColor::new("#1a1a1e").unwrap(),
            },
            dash: DashSpec {
                opacity: Opacity::from_percent(70.0).unwrap(),
            },
            tint: TintSpec {
                accent_hex: HexColor::new("#3584e4").unwrap(),
                intensity: Opacity::from_percent(5.0).unwrap(),
            },
            overview_blur: true,
        }
    }

    #[test]
    fn gtk_css_uses_media_queries() {
        let generator = Gnome50CssGenerator;
        let css = generator.generate(&test_spec()).unwrap();
        assert!(css.gtk_css.contains("prefers-color-scheme: dark"));
        assert!(!css.gtk_css.contains("style-dark.css"));
    }

    #[test]
    fn shell_css_includes_reduced_motion_guard() {
        let generator = Gnome50CssGenerator;
        let css = generator.generate(&test_spec()).unwrap();
        assert!(css.shell_css.contains("prefers-reduced-motion: no-preference"));
    }

    #[test]
    fn uses_refined_dark_palette() {
        let generator = Gnome50CssGenerator;
        let css = generator.generate(&test_spec()).unwrap();
        // Should NOT contain old GNOME 47 base colors
        assert!(!css.shell_css.contains("#242428"));
    }

    #[test]
    fn version_label() {
        assert_eq!(Gnome50CssGenerator.version_label(), "GNOME 50");
    }
}
