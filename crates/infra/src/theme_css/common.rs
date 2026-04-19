// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared CSS generation helpers used across all GNOME version adapters.

use gnomex_domain::{color::blend, HexColor, ThemeSpec};

/// Generate the GTK4/Libadwaita `@define-color` tinting block.
/// This is identical across GNOME versions since it targets GTK, not Shell.
pub fn gtk_tint_css(spec: &ThemeSpec) -> String {
    let tint_pct = spec.tint.intensity.as_fraction();
    let sidebar_alpha = spec.sidebar.opacity.as_fraction();

    let tl = tint_pct + 0.025;
    let tcl = tint_pct * 0.6;
    let td = tint_pct;

    let sidebar_light = sidebar_color_expr(&format!("mix(#ebebed, @accent_bg_color, {tl})"), sidebar_alpha);
    let sidebar_dark = sidebar_color_expr(&format!("mix(#2e2e32, @accent_bg_color, {td})"), sidebar_alpha);

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
    @define-color sidebar_bg_color {sidebar_light};
}}

@media (prefers-color-scheme: dark) {{
    @define-color window_bg_color mix(#222226, @accent_bg_color, {td});
    @define-color view_bg_color mix(#1d1d20, @accent_bg_color, {td});
    @define-color headerbar_bg_color mix(#2e2e32, @accent_bg_color, {td});
    @define-color headerbar_backdrop_color mix(#222226, @accent_bg_color, {td});
    @define-color popover_bg_color mix(#36363a, @accent_bg_color, {td});
    @define-color dialog_bg_color mix(#36363a, @accent_bg_color, {td});
    @define-color card_bg_color mix(rgba(255, 255, 255, 0.08), @accent_bg_color, {td});
    @define-color sidebar_bg_color {sidebar_dark};
}}
"#,
    )
}

/// Wrap a sidebar background expression in `alpha()` iff the user has
/// dialed opacity below fully opaque. Emitting the raw `mix()` when
/// opacity == 1.0 keeps the generated CSS readable.
fn sidebar_color_expr(mix_expr: &str, opacity: f64) -> String {
    if opacity >= 0.999 {
        mix_expr.to_string()
    } else {
        format!("alpha({mix_expr}, {opacity:.3})")
    }
}

/// Generate border-radius rules for GTK4 elements.
pub fn gtk_radius_css(spec: &ThemeSpec) -> String {
    let wr = spec.window_radius.as_i32();
    let er = spec.element_radius.as_i32();
    format!(
        r#"/* Border radius */
:root {{ --window-radius: {wr}px; }}
window.background {{ border-radius: {wr}px; }}
window.dialog {{ border-radius: {wr}px; }}
button {{ border-radius: {er}px; }}
entry {{ border-radius: {er}px; }}
.card {{ border-radius: {er}px; }}
popover > contents {{ border-radius: {er}px; }}
"#
    )
}

/// Generate CSS for headerbar, window frame, and visual inset customizations.
pub fn gtk_csd_css(spec: &ThemeSpec) -> String {
    let hb = &spec.headerbar;
    let wf = &spec.window_frame;
    let insets = &spec.insets;

    let hb_height = hb.min_height.as_i32();
    let hb_shadow = hb.shadow_intensity.as_fraction();

    let titlebar_btn_css = if hb.circular_buttons {
        "headerbar button.titlebutton {\n    border-radius: 50%;\n    min-width: 14px;\n    min-height: 14px;\n    padding: 2px;\n}"
    } else {
        ""
    };

    let window_shadow = if wf.show_shadow {
        ""
    } else {
        "window, window.csd, .window-frame {\n    box-shadow: none;\n    margin: 0;\n}"
    };

    let inset_border = wf.inset_border.as_i32();
    let inset_css = if inset_border > 0 {
        format!(
            "window.csd {{ box-shadow: inset 0 0 0 {inset_border}px @borders; }}"
        )
    } else {
        String::new()
    };

    let card_border = insets.card_border_width.as_i32();
    let sep_opacity = insets.separator_opacity.as_fraction();
    let focus_width = insets.focus_ring_width.as_i32();

    let combo_css = if !insets.combo_inset {
        "button.combo, .dropdown { border: none; box-shadow: none; }"
    } else {
        ""
    };

    format!(
        r#"/* Headerbar CSD */
headerbar {{
    min-height: {hb_height}px;
    box-shadow: inset 0 -1px alpha(black, {hb_shadow});
}}

{titlebar_btn_css}

{window_shadow}

{inset_css}

/* Card borders */
.card {{ border: {card_border}px solid @borders; }}

/* Separator visibility */
separator {{ opacity: {sep_opacity}; }}

/* Focus ring */
*:focus-visible {{ outline-width: {focus_width}px; }}

{combo_css}
"#
    )
}

/// Generate CSS for foreground/text and semantic status color overrides.
pub fn gtk_color_overrides_css(spec: &ThemeSpec) -> String {
    let mut css = String::new();
    let fg = &spec.foreground;
    let sc = &spec.status_colors;

    fn color_line(name: &str, val: &Option<HexColor>) -> String {
        match val {
            Some(c) => format!("@define-color {name} {hex};\n", hex = c.as_str()),
            None => String::new(),
        }
    }

    css.push_str("/* Foreground overrides */\n");
    css.push_str(&color_line("window_fg_color", &fg.window_fg));
    css.push_str(&color_line("view_fg_color", &fg.view_fg));
    css.push_str(&color_line("headerbar_fg_color", &fg.headerbar_fg));
    css.push_str(&color_line("headerbar_border_color", &fg.headerbar_border));
    css.push_str(&color_line("sidebar_fg_color", &spec.sidebar.fg_override));

    css.push_str("\n/* Semantic status colors */\n");
    css.push_str(&color_line("destructive_bg_color", &sc.destructive));
    css.push_str(&color_line("success_bg_color", &sc.success));
    css.push_str(&color_line("warning_bg_color", &sc.warning));
    css.push_str(&color_line("error_bg_color", &sc.error));

    css
}

/// Generate shell CSS for notification/calendar styling.
pub fn shell_notification_css(spec: &ThemeSpec) -> String {
    let nr = spec.notifications.radius.as_i32();
    let no = spec.notifications.opacity.as_fraction();

    format!(
        r#"/* Notification / calendar styling */
.notification-banner {{
    border-radius: {nr}px;
    background-color: alpha(@osd_bg_color, {no});
}}

.calendar, .world-clocks-button, .weather-button, .events-button {{
    border-radius: {nr}px;
}}
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
