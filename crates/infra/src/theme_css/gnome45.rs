// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! GNOME 45 CSS adapter.
//! Shell changes: panel reworked with .panel-button, overview restructured.

use super::common::{
    gtk_color_overrides_css, gtk_csd_css, gtk_layer_separation_css, gtk_radius_css, gtk_tint_css,
    gtk_widget_color_overrides_css, gtk_widget_style_css, tint_shell_surfaces,
};
use super::gtk3::generate_gtk3_css;
use gnomex_app::ports::{ThemeCss, ThemeCssGenerator};
use gnomex_app::AppError;
use gnomex_domain::ThemeSpec;

pub struct Gnome45CssGenerator;

impl ThemeCssGenerator for Gnome45CssGenerator {
    fn version_label(&self) -> &str {
        "GNOME 45"
    }

    fn generate(&self, spec: &ThemeSpec) -> Result<ThemeCss, AppError> {
        Ok(ThemeCss {
            gtk_css: self.gtk(spec),
            gtk3_css: generate_gtk3_css(spec),
            shell_css: self.shell(spec),
        })
    }
}

impl Gnome45CssGenerator {
    fn gtk(&self, spec: &ThemeSpec) -> String {
        format!(
            "/* GNOME X — GTK4 overrides */\n\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
            gtk_radius_css(spec),
            gtk_csd_css(spec),
            gtk_layer_separation_css(spec),
            gtk_widget_style_css(spec),
            gtk_color_overrides_css(spec),
            gtk_tint_css(spec),
            gtk_widget_color_overrides_css(spec),
        )
    }

    fn shell(&self, spec: &ThemeSpec) -> String {
        let s = tint_shell_surfaces(spec);
        let pr = spec.panel.radius.as_i32();
        let po = spec.panel.opacity.as_fraction();
        let do_ = spec.dash.opacity.as_fraction();

        // GNOME 45: #panel uses .panel-button, dash is #dash
        format!(
            r#"/* GNOME X — Shell overrides (GNOME 45) */
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

.popup-menu-content {{
    background-color: {osd};
}}
"#,
            panel = s.panel,
            dash = s.dash,
            search = s.search,
            osd = s.osd,
        )
    }
}
