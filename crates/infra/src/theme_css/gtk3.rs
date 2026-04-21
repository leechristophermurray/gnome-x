// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! GTK3 CSS generator (GXF-001).
//!
//! GTK3's token namespace and widget tree diverge meaningfully from
//! GTK4/Libadwaita. Previously the theme writer emitted byte-identical
//! CSS to both `gtk-4.0/gtk.css` and `gtk-3.0/gtk.css`, which made the
//! GTK3 override a mostly-no-op and sometimes actively worse than
//! vanilla Adwaita (GTK3 would see a pile of undefined `@window_bg_color`
//! / `@card_bg_color` references and GTK4-only selectors like
//! `.navigation-sidebar`, `.card`, `splitview`, `popover > contents`).
//!
//! GTK 3.24 is the last stable GTK3 release line; GNOME X targets only
//! that — there is no need for per-GNOME-version variants the way
//! Adwaita has. A single generator covers every Flatpak'd Chromium,
//! Electron app, and legacy GNOME utility that still renders under GTK3.
//!
//! ## ThemeSpec knob translation
//!
//! | Knob                                     | GTK3 mapping                                                              |
//! |------------------------------------------|---------------------------------------------------------------------------|
//! | `tint.accent_hex` / `tint.intensity`     | `@theme_bg_color`, `@theme_base_color`, `@theme_selected_bg_color`        |
//! | `window_radius` / `element_radius`       | `.titlebar`, `window`, `button`, `entry` `border-radius`                  |
//! | `headerbar.min_height`                   | `headerbar`, `.titlebar` `min-height`                                     |
//! | `headerbar.shadow_intensity`             | `headerbar` `box-shadow` inset                                            |
//! | `headerbar.circular_buttons`             | `headerbar button.titlebutton` border-radius                              |
//! | `window_frame.show_shadow`               | `window.csd` `box-shadow: none;` when disabled                            |
//! | `window_frame.inset_border`              | `window.csd` inset `box-shadow` border                                    |
//! | `insets.card_border_width`               | `frame.flat, frame, list.content` (GTK3 has no `.card`)                   |
//! | `insets.separator_opacity`               | `separator` `opacity`                                                     |
//! | `insets.focus_ring_width`                | `*:focus` `outline-width` (GTK3 uses `:focus`, not `:focus-visible`)      |
//! | `widget_style.input_inset`               | `entry, spinbutton` (same selectors)                                      |
//! | `widget_style.button_raise`              | `button:not(.flat)` (no `.suggested-action`/`.destructive-action` in GTK3)|
//! | `widget_style.headerbar_gradient`        | `headerbar, .titlebar, .toolbar` `linear-gradient`                        |
//! | `widget_colors.*`                        | `@theme_*` token overrides via `@define-color`                            |
//! | `foreground.window_fg`                   | `@theme_fg_color`                                                         |
//! | `foreground.view_fg`                     | `@theme_text_color`                                                       |
//! | `foreground.headerbar_fg`                | `headerbar { color: ... }` (GTK3 has no dedicated token)                  |
//! | `sidebar.fg_override`                    | `.sidebar, .nautilus-list-view .sidebar { color: ... }`                   |
//! | `sidebar.opacity`                        | `.sidebar` background alpha                                               |
//! | `layers.headerbar_bottom`                | `headerbar, .titlebar { border-bottom: ... }`                             |
//! | `layers.sidebar_divider`                 | `.sidebar { border-right: ... }` (no `.navigation-sidebar`)               |
//! | `layers.content_contrast`                | n/a — GTK3 lacks the `.content-pane`/`splitview` tree                     |
//! | `status_colors.*`                        | `@error_color`, `@warning_color`, `@success_color` token overrides        |
//! | `notifications.*`                        | n/a — GTK3 apps get shell-side notifications, not CSS-styled              |
//! | `overview_blur`, `panel.*`, `dash.*`     | n/a — these are Shell-only                                                |
//! | `headerbar_backdrop_color`, `card_bg_color`, `popover_bg_color` | n/a — GTK3 doesn't expose these tokens         |
//!
//! ### Dark variant handling
//!
//! GTK 3.24 does **not** support `@media (prefers-color-scheme: ...)`.
//! The generator emits a single variant keyed to the panel-tint
//! luminance (same heuristic the external themers use). If the user
//! wants dark GTK3 apps they must also set
//! `org.gnome.desktop.interface gtk-application-prefer-dark-theme`
//! or install a dedicated GTK3 dark theme — that's a user-level
//! setting, not something we write into `gtk-3.0/gtk.css`.

use gnomex_app::ports::{ThemeCss, ThemeCssGenerator};
use gnomex_app::AppError;
use gnomex_domain::{color::blend, ColorScheme, HexColor, ThemeSpec};

/// Header comment embedded in every GTK3 stylesheet we emit. Contains
/// the literal `"GNOME X"` substring the conflict detector greps for.
const GTK3_HEADER: &str = "/* GNOME X — GTK3 overrides */";

/// CSS generator for GTK 3.24+.
///
/// Produces output targeting the `@theme_bg_color` / `@theme_base_color`
/// token namespace and GTK3-only widget selectors. See the module
/// docstring for the full `ThemeSpec` translation table.
pub struct Gtk3CssGenerator;

impl ThemeCssGenerator for Gtk3CssGenerator {
    fn version_label(&self) -> &str {
        "GTK 3.24+"
    }

    fn generate(&self, spec: &ThemeSpec) -> Result<ThemeCss, AppError> {
        Ok(ThemeCss {
            gtk_css: String::new(),
            gtk3_css: generate_gtk3_css(spec),
            shell_css: String::new(),
        })
    }
}

/// Build the full GTK3 stylesheet for `spec`.
///
/// Exposed as a free function so the per-GNOME-version generators can
/// fill their `ThemeCss::gtk3_css` field without constructing a
/// `Gtk3CssGenerator` + re-dispatching through the trait.
pub fn generate_gtk3_css(spec: &ThemeSpec) -> String {
    let scheme = scheme_from_panel_tint(&spec.panel.tint);
    let mut out = String::new();
    out.push_str(GTK3_HEADER);
    out.push('\n');
    out.push_str("/* Generated for GTK 3.24+ — @theme_* token namespace. */\n");
    out.push_str("/* Single-variant output keyed to panel-tint luminance */\n");
    out.push_str("/* (GTK3 has no colour-scheme media query). */\n\n");

    out.push_str(&gtk3_tint_css(spec, scheme));
    out.push('\n');
    out.push_str(&gtk3_radius_css(spec));
    out.push('\n');
    out.push_str(&gtk3_csd_css(spec));
    out.push('\n');
    out.push_str(&gtk3_layer_separation_css(spec));
    out.push_str(&gtk3_widget_style_css(spec));
    out.push_str(&gtk3_foreground_css(spec));
    out.push_str(&gtk3_sidebar_css(spec));
    out.push_str(&gtk3_widget_color_overrides_css(spec));
    out.push_str(&gtk3_status_colors_css(spec));

    out
}

/// Derive a light/dark variant flag from the panel tint luminance.
/// GTK3 has no native colour-scheme media query, so we pick one at
/// generation time (same heuristic the external-app themers use).
fn scheme_from_panel_tint(hex: &HexColor) -> ColorScheme {
    let (r, g, b) = hex.to_rgb();
    let luma = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    if luma < 128.0 {
        ColorScheme::Dark
    } else {
        ColorScheme::Light
    }
}

/// Emit the GTK3 accent-tint block against the legacy `@theme_*`
/// namespace. Uses hex-blended bases rather than `mix(@accent_bg_color, ...)`
/// because GTK3 does not expose `@accent_bg_color` (that's a
/// Libadwaita-only named colour).
fn gtk3_tint_css(spec: &ThemeSpec, scheme: ColorScheme) -> String {
    let accent = spec.tint.accent_hex.to_rgb();
    let pct = spec.tint.intensity.as_fraction() as f32;

    let (window_base, view_base, headerbar_base, sidebar_base): (
        (u8, u8, u8),
        (u8, u8, u8),
        (u8, u8, u8),
        (u8, u8, u8),
    ) = if scheme.is_dark() {
        // GTK3 Adwaita-dark base palette.
        (
            (0x24, 0x24, 0x28),
            (0x1d, 0x1d, 0x20),
            (0x2e, 0x2e, 0x32),
            (0x2a, 0x2a, 0x2e),
        )
    } else {
        // GTK3 Adwaita light base palette.
        (
            (0xfa, 0xfa, 0xfb),
            (0xff, 0xff, 0xff),
            (0xeb, 0xeb, 0xed),
            (0xf0, 0xf0, 0xf2),
        )
    };

    let window_bg = blend(window_base, accent, pct);
    let view_bg = blend(view_base, accent, pct);
    let headerbar_bg = blend(headerbar_base, accent, pct);
    let sidebar_bg = blend(sidebar_base, accent, pct);
    let selected_bg = spec.tint.accent_hex.as_str();

    format!(
        r#"/* Accent-tinted surfaces */
@define-color theme_bg_color {window_bg};
@define-color theme_base_color {view_bg};
@define-color theme_selected_bg_color {selected_bg};
@define-color headerbar_bg_color {headerbar_bg};
@define-color sidebar_bg_color {sidebar_bg};
"#,
    )
}

/// Border-radius rules against GTK3 selectors (`.titlebar` instead of
/// `headerbar`-only, no `.card`, no `popover > contents`).
fn gtk3_radius_css(spec: &ThemeSpec) -> String {
    let wr = spec.window_radius.as_i32();
    let er = spec.element_radius.as_i32();
    format!(
        r#"/* Border radius */
window, window.background {{ border-radius: {wr}px; }}
.titlebar, headerbar {{ border-radius: {wr}px {wr}px 0 0; }}
button {{ border-radius: {er}px; }}
entry, spinbutton {{ border-radius: {er}px; }}
"#
    )
}

/// Headerbar / CSD rules translated to GTK3.
/// GTK3 uses `.titlebar` alongside `headerbar`, has no
/// `:focus-visible` pseudo (uses `:focus` instead), and has no
/// `.card` class. Combo-box insets use `GtkComboBox` not `.combo`.
fn gtk3_csd_css(spec: &ThemeSpec) -> String {
    let hb = &spec.headerbar;
    let wf = &spec.window_frame;
    let insets = &spec.insets;

    let hb_height = hb.min_height.as_i32();
    let hb_shadow = hb.shadow_intensity.as_fraction();

    let titlebar_btn_css = if hb.circular_buttons {
        "headerbar button.titlebutton, .titlebar button.titlebutton {\n\
         \x20   border-radius: 50%;\n\
         \x20   min-width: 14px;\n\
         \x20   min-height: 14px;\n\
         \x20   padding: 2px;\n\
         }"
    } else {
        ""
    };

    let window_shadow = if wf.show_shadow {
        ""
    } else {
        "window, window.csd, .window-frame {\n\
         \x20   box-shadow: none;\n\
         \x20   margin: 0;\n\
         }"
    };

    let inset_border = wf.inset_border.as_i32();
    let inset_css = if inset_border > 0 {
        format!(
            "window.csd {{ box-shadow: inset 0 0 0 {inset_border}px @borders; }}"
        )
    } else {
        String::new()
    };

    // GTK3 has no `.card` — the nearest legacy analogue is `frame` /
    // `list.content`, which sidebar-style groups use.
    let card_border = insets.card_border_width.as_i32();
    let sep_opacity = insets.separator_opacity.as_fraction();
    let focus_width = insets.focus_ring_width.as_i32();

    let combo_css = if !insets.combo_inset {
        "combobox button, combobox entry { border: none; box-shadow: none; }"
    } else {
        ""
    };

    format!(
        r#"/* Headerbar CSD */
headerbar, .titlebar {{
    min-height: {hb_height}px;
    box-shadow: inset 0 -1px alpha(black, {hb_shadow});
}}

{titlebar_btn_css}

{window_shadow}

{inset_css}

/* "Card"-like frames (GTK3 has no `.card` class) */
frame.flat, list.content {{ border: {card_border}px solid @borders; }}

/* Separator visibility */
separator {{ opacity: {sep_opacity}; }}

/* Focus ring (GTK3 uses :focus, not :focus-visible) */
*:focus {{ outline-width: {focus_width}px; }}

{combo_css}
"#
    )
}

/// Layer-separation translated to GTK3 selectors.
/// `content_contrast` is intentionally dropped: GTK3 has no
/// `.content-pane` / `splitview` widget tree, so there is nothing
/// sensible to target that wouldn't re-tint every `.view`.
fn gtk3_layer_separation_css(spec: &ThemeSpec) -> String {
    let hb = spec.layers.headerbar_bottom.as_i32();
    let sb = spec.layers.sidebar_divider.as_i32();

    if hb == 0 && sb == 0 {
        return String::new();
    }

    let mut out = String::from("/* Layer separation */\n");

    if hb > 0 {
        out.push_str(&format!(
            "headerbar, .titlebar {{ border-bottom: {hb}px solid @borders; }}\n\
             headerbar.flat, .titlebar.flat {{ border-bottom: {hb}px solid @borders; }}\n",
        ));
    }

    if sb > 0 {
        // GTK3 has no `.navigation-sidebar` — `.sidebar` is the common
        // class used by Nautilus (GTK3 era), GIMP, etc.
        out.push_str(&format!(
            ".sidebar, .nautilus-list-view .sidebar {{ border-right: {sb}px solid @borders; }}\n",
        ));
    }

    out
}

/// Restore-traditional-chrome knobs mapped to GTK3 selectors.
/// Button :not() filter drops the Libadwaita-only classes
/// `.suggested-action` / `.destructive-action` and keeps only the one
/// GTK3 actually uses (`.flat`).
fn gtk3_widget_style_css(spec: &ThemeSpec) -> String {
    let input = spec.widget_style.input_inset.as_fraction();
    let button = spec.widget_style.button_raise.as_fraction();
    let gradient = spec.widget_style.headerbar_gradient.as_fraction();

    if input <= f64::EPSILON && button <= f64::EPSILON && gradient <= f64::EPSILON {
        return String::new();
    }

    let mut out = String::from("/* Widget style */\n");

    if input > f64::EPSILON {
        let border_alpha = 0.08 + 0.22 * input;
        let inset_shadow = 0.10 * input;
        out.push_str(&format!(
            "entry, entry.flat, spinbutton, spinbutton.flat {{\n\
             \x20   border: 1px solid alpha(currentColor, {border_alpha:.3});\n\
             \x20   box-shadow: inset 0 1px 0 alpha(black, {inset_shadow:.3});\n\
             }}\n",
        ));
    }

    if button > f64::EPSILON {
        let border_alpha = 0.10 + 0.25 * button;
        let shadow_alpha = 0.08 + 0.15 * button;
        out.push_str(&format!(
            "button:not(.flat) {{\n\
             \x20   border: 1px solid alpha(currentColor, {border_alpha:.3});\n\
             \x20   box-shadow: 0 1px 0 alpha(black, {shadow_alpha:.3});\n\
             }}\n\
             button:not(.flat):active {{\n\
             \x20   box-shadow: inset 0 1px 0 alpha(black, {shadow_alpha:.3});\n\
             }}\n",
        ));
    }

    if gradient > f64::EPSILON {
        let delta = 0.03 + 0.06 * gradient;
        out.push_str(&format!(
            "headerbar, .titlebar, toolbar, .toolbar {{\n\
             \x20   background-image: linear-gradient(to bottom,\n\
             \x20       shade(@headerbar_bg_color, {top:.3}),\n\
             \x20       shade(@headerbar_bg_color, {bot:.3}));\n\
             }}\n",
            top = 1.0 + delta,
            bot = 1.0 - delta,
        ));
    }

    out
}

/// Foreground overrides against the GTK3 `@theme_fg_color` /
/// `@theme_text_color` tokens. `headerbar_fg` has no dedicated GTK3
/// token so it is applied directly to the `headerbar` selector.
fn gtk3_foreground_css(spec: &ThemeSpec) -> String {
    let fg = &spec.foreground;
    let mut out = String::new();
    let mut body = String::new();

    if let Some(c) = &fg.window_fg {
        body.push_str(&format!(
            "@define-color theme_fg_color {};\n",
            c.as_str()
        ));
    }
    if let Some(c) = &fg.view_fg {
        body.push_str(&format!(
            "@define-color theme_text_color {};\n",
            c.as_str()
        ));
    }
    if let Some(c) = &fg.headerbar_fg {
        body.push_str(&format!(
            "headerbar, .titlebar {{ color: {}; }}\n",
            c.as_str()
        ));
    }
    if let Some(c) = &fg.headerbar_border {
        body.push_str(&format!(
            "headerbar, .titlebar {{ border-color: {}; }}\n",
            c.as_str()
        ));
    }

    if !body.is_empty() {
        out.push_str("/* Foreground overrides */\n");
        out.push_str(&body);
    }
    out
}

/// Sidebar-specific rules translated to GTK3's `.sidebar` class
/// (used by Nautilus-GTK3, GIMP, Inkscape-3, etc.) — **not**
/// `.navigation-sidebar`, which is GTK4-only.
fn gtk3_sidebar_css(spec: &ThemeSpec) -> String {
    let opacity = spec.sidebar.opacity.as_fraction();
    let mut out = String::new();
    let mut body = String::new();

    if opacity < 0.999 {
        body.push_str(&format!(
            ".sidebar, .nautilus-list-view .sidebar {{\n\
             \x20   background-color: alpha(@sidebar_bg_color, {opacity:.3});\n\
             }}\n",
        ));
    }

    if let Some(c) = &spec.sidebar.fg_override {
        body.push_str(&format!(
            ".sidebar, .nautilus-list-view .sidebar {{ color: {}; }}\n",
            c.as_str()
        ));
    }

    if !body.is_empty() {
        out.push_str("/* Sidebar */\n");
        out.push_str(&body);
    }
    out
}

/// Per-widget colour overrides against the GTK3 token namespace.
/// GTK3 has no `prefers-color-scheme` so `*_light` vs `*_dark` fields
/// are combined — whichever variant matches the currently-chosen
/// scheme wins. See `scheme_from_panel_tint`.
fn gtk3_widget_color_overrides_css(spec: &ThemeSpec) -> String {
    let w = &spec.widget_colors;
    if w.is_empty() {
        return String::new();
    }

    let scheme = scheme_from_panel_tint(&spec.panel.tint);
    let is_dark = scheme.is_dark();

    fn pick<'a>(
        light: &'a Option<HexColor>,
        dark: &'a Option<HexColor>,
        is_dark: bool,
    ) -> Option<&'a HexColor> {
        if is_dark {
            dark.as_ref().or(light.as_ref())
        } else {
            light.as_ref().or(dark.as_ref())
        }
    }

    let button = pick(&w.button_bg_light, &w.button_bg_dark, is_dark);
    let entry = pick(&w.entry_bg_light, &w.entry_bg_dark, is_dark);
    let headerbar = pick(&w.headerbar_bg_light, &w.headerbar_bg_dark, is_dark);
    let sidebar = pick(&w.sidebar_bg_light, &w.sidebar_bg_dark, is_dark);

    let mut out = String::from("/* Per-widget colour overrides */\n");
    if let Some(c) = button {
        out.push_str(&format!(
            "button:not(.flat) {{ background-color: {}; background-image: none; }}\n",
            c.as_str()
        ));
    }
    if let Some(c) = entry {
        out.push_str(&format!(
            "entry, spinbutton {{ background-color: {}; background-image: none; }}\n",
            c.as_str()
        ));
    }
    if let Some(c) = headerbar {
        out.push_str(&format!(
            "@define-color headerbar_bg_color {};\n",
            c.as_str()
        ));
        out.push_str(&format!(
            "headerbar, .titlebar {{ background-color: {}; background-image: none; }}\n",
            c.as_str()
        ));
    }
    if let Some(c) = sidebar {
        out.push_str(&format!(
            "@define-color sidebar_bg_color {};\n",
            c.as_str()
        ));
        out.push_str(&format!(
            ".sidebar {{ background-color: {}; }}\n",
            c.as_str()
        ));
    }

    out
}

/// Semantic status-colour overrides against the GTK3 `@error_color`,
/// `@warning_color`, `@success_color` named-colour tokens.
fn gtk3_status_colors_css(spec: &ThemeSpec) -> String {
    let sc = &spec.status_colors;
    let mut body = String::new();
    if let Some(c) = &sc.destructive {
        body.push_str(&format!(
            "@define-color destructive_color {};\n",
            c.as_str()
        ));
    }
    if let Some(c) = &sc.success {
        body.push_str(&format!("@define-color success_color {};\n", c.as_str()));
    }
    if let Some(c) = &sc.warning {
        body.push_str(&format!("@define-color warning_color {};\n", c.as_str()));
    }
    if let Some(c) = &sc.error {
        body.push_str(&format!("@define-color error_color {};\n", c.as_str()));
    }
    if body.is_empty() {
        return String::new();
    }
    let mut out = String::from("/* Semantic status colors */\n");
    out.push_str(&body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use gnomex_domain::ThemeSpec;

    #[test]
    fn default_spec_contains_gnome_x_marker() {
        let css = generate_gtk3_css(&ThemeSpec::defaults());
        assert!(
            css.contains("GNOME X"),
            "GTK3 output is missing the managed-region marker; conflict detector will false-positive every user:\n{css}"
        );
    }

    #[test]
    fn default_spec_emits_theme_bg_color_not_window_bg_color() {
        let css = generate_gtk3_css(&ThemeSpec::defaults());
        assert!(
            css.contains("@define-color theme_bg_color"),
            "GTK3 output missing `@theme_bg_color` — the legacy namespace is the only one GTK3 honours.\n{css}",
        );
        assert!(
            !css.contains("@define-color window_bg_color"),
            "GTK3 output contains GTK4-only `@window_bg_color`; should be `@theme_bg_color`.\n{css}",
        );
    }

    #[test]
    fn default_spec_does_not_emit_gtk4_only_selectors() {
        // These selectors are GTK4/Libadwaita-only and cause GTK3 to
        // log CSS-parser warnings at every widget instantiation.
        let css = generate_gtk3_css(&ThemeSpec::defaults());
        for selector in [".navigation-sidebar", ".card ", "splitview", "popover > contents"] {
            assert!(
                !css.contains(selector),
                "GTK3 output leaked GTK4-only selector `{selector}`:\n{css}",
            );
        }
    }

    #[test]
    fn version_label_is_gtk3() {
        assert_eq!(Gtk3CssGenerator.version_label(), "GTK 3.24+");
    }
}
