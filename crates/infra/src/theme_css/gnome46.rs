// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! GNOME 46 CSS adapter.
//! Shell changes: file chooser portal, minor selector refinements.

use super::common::{gtk_radius_css, gtk_tint_css, tint_shell_surfaces};
use gnomex_app::ports::{ThemeCss, ThemeCssGenerator};
use gnomex_app::AppError;
use gnomex_domain::ThemeSpec;

pub struct Gnome46CssGenerator;

impl ThemeCssGenerator for Gnome46CssGenerator {
    fn version_label(&self) -> &str {
        "GNOME 46"
    }

    fn generate(&self, spec: &ThemeSpec) -> Result<ThemeCss, AppError> {
        Ok(ThemeCss {
            gtk_css: self.gtk(spec),
            shell_css: self.shell(spec),
        })
    }
}

impl Gnome46CssGenerator {
    fn gtk(&self, spec: &ThemeSpec) -> String {
        format!(
            "/* GNOME X — GTK4 overrides (GNOME 46) */\n\n{}\n{}",
            gtk_radius_css(spec),
            gtk_tint_css(spec),
        )
    }

    fn shell(&self, spec: &ThemeSpec) -> String {
        let s = tint_shell_surfaces(spec);
        let pr = spec.panel.radius.as_i32();
        let po = spec.panel.opacity.as_fraction();
        let do_ = spec.dash.opacity.as_fraction();

        // GNOME 46: same selectors as 45 with minor refinements
        format!(
            r#"/* GNOME X — Shell overrides (GNOME 46) */
@import url("resource:///org/gnome/shell/theme/gnome-shell.css");

#panel {{
    background-color: alpha({panel}, {po}) !important;
    border-radius: 0 0 {pr}px {pr}px;
}}

#panel .panel-button {{
    border-radius: {pr}px;
}}

#dash {{
    background-color: alpha({dash}, {do_});
    border-radius: 16px;
    padding: 6px;
    margin: 8px;
    border: 1px solid rgba(255, 255, 255, 0.06);
}}

.search-entry {{
    background-color: {search};
    border-radius: 18px;
}}

.popup-menu-content, .candidate-popup-content {{
    background-color: {osd};
}}

.events-button, .world-clocks-button, .weather-button, .message {{
    background-color: alpha({osd}, 0.9);
}}
"#,
            panel = s.panel,
            dash = s.dash,
            search = s.search,
            osd = s.osd,
        )
    }
}
