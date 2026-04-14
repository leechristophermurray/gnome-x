// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! GNOME 47+ CSS adapter.
//! Shell changes: accent-color support, updated OSD/notification selectors.
//! This is also the fallback for unrecognised future versions.

use super::common::{gtk_radius_css, gtk_tint_css, tint_shell_surfaces};
use gnomex_app::ports::{ThemeCss, ThemeCssGenerator};
use gnomex_app::AppError;
use gnomex_domain::ThemeSpec;

pub struct Gnome47CssGenerator;

impl ThemeCssGenerator for Gnome47CssGenerator {
    fn version_label(&self) -> &str {
        "GNOME 47+"
    }

    fn generate(&self, spec: &ThemeSpec) -> Result<ThemeCss, AppError> {
        Ok(ThemeCss {
            gtk_css: self.gtk(spec),
            shell_css: self.shell(spec),
        })
    }
}

impl Gnome47CssGenerator {
    fn gtk(&self, spec: &ThemeSpec) -> String {
        format!(
            "/* GNOME X — GTK4 overrides (GNOME 47+) */\n\n{}\n{}",
            gtk_radius_css(spec),
            gtk_tint_css(spec),
        )
    }

    fn shell(&self, spec: &ThemeSpec) -> String {
        let s = tint_shell_surfaces(spec);
        let pr = spec.panel.radius.as_i32();
        let po = spec.panel.opacity.as_fraction();
        let do_ = spec.dash.opacity.as_fraction();

        let blur = if spec.overview_blur {
            "/* Overview blur enabled via shell settings */"
        } else {
            "#overview { background-color: rgba(0, 0, 0, 0.6); }"
        };

        // GNOME 47+: native accent-color support, updated notification/OSD
        format!(
            r#"/* GNOME X — Shell overrides (GNOME 47+) */
@import url("resource:///org/gnome/shell/theme/gnome-shell.css");

/* Accent-tinted panel */
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

{blur}
"#,
            panel = s.panel,
            dash = s.dash,
            search = s.search,
            osd = s.osd,
        )
    }
}
