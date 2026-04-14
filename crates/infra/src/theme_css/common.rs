// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared CSS generation helpers used across all GNOME version adapters.

use gnomex_domain::{color::blend, ThemeSpec};

/// Generate the GTK4/Libadwaita `@define-color` tinting block.
/// This is identical across GNOME versions since it targets GTK, not Shell.
pub fn gtk_tint_css(spec: &ThemeSpec) -> String {
    let tint_pct = spec.tint.intensity.as_fraction();

    format!(
        r#"/* Accent-tinted surfaces */
@media (prefers-color-scheme: light) {{
    @define-color window_bg_color mix(#fafafb, @accent_bg_color, {tl});
    @define-color view_bg_color mix(#ffffff, @accent_bg_color, {tl});
    @define-color headerbar_bg_color mix(#ffffff, @accent_bg_color, {tl});
    @define-color headerbar_backdrop_color mix(#fafafb, @accent_bg_color, {tl});
    @define-color popover_bg_color mix(#ffffff, @accent_bg_color, {tl});
    @define-color dialog_bg_color mix(#fafafb, @accent_bg_color, {tl});
    @define-color card_bg_color mix(#ffffff, @accent_bg_color, {tcl});
    @define-color sidebar_bg_color mix(#ebebed, @accent_bg_color, {tl});
}}

@media (prefers-color-scheme: dark) {{
    @define-color window_bg_color mix(#222226, @accent_bg_color, {td});
    @define-color view_bg_color mix(#1d1d20, @accent_bg_color, {td});
    @define-color headerbar_bg_color mix(#2e2e32, @accent_bg_color, {td});
    @define-color headerbar_backdrop_color mix(#222226, @accent_bg_color, {td});
    @define-color popover_bg_color mix(#36363a, @accent_bg_color, {td});
    @define-color dialog_bg_color mix(#36363a, @accent_bg_color, {td});
    @define-color card_bg_color mix(rgba(255, 255, 255, 0.08), @accent_bg_color, {td});
    @define-color sidebar_bg_color mix(#2e2e32, @accent_bg_color, {td});
}}
"#,
        tl = tint_pct + 0.025,
        tcl = tint_pct * 0.6,
        td = tint_pct,
    )
}

/// Generate border-radius rules for GTK4 elements.
pub fn gtk_radius_css(spec: &ThemeSpec) -> String {
    let wr = spec.window_radius.as_i32();
    let er = spec.element_radius.as_i32();
    format!(
        r#"/* Border radius */
window.background {{ border-radius: {wr}px; }}
window.dialog {{ border-radius: {wr}px; }}
button {{ border-radius: {er}px; }}
entry {{ border-radius: {er}px; }}
.card {{ border-radius: {er}px; }}
popover > contents {{ border-radius: {er}px; }}
"#
    )
}

/// Compute accent-tinted shell surface colors.
pub struct ShellSurfaces {
    pub panel: String,
    pub dash: String,
    pub osd: String,
    pub search: String,
}

/// Dark-mode base colors for shell surfaces.
pub fn tint_shell_surfaces(spec: &ThemeSpec) -> ShellSurfaces {
    let accent = spec.tint.accent_hex.to_rgb();
    let pct = spec.tint.intensity.as_fraction() as f32;

    ShellSurfaces {
        panel: blend((0x24, 0x24, 0x28), accent, pct),
        dash: blend((0x30, 0x30, 0x34), accent, pct),
        osd: blend((0x30, 0x30, 0x34), accent, pct),
        search: blend((0x3a, 0x3a, 0x3e), accent, pct),
    }
}
