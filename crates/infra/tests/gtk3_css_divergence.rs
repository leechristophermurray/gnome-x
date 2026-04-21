// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! GTK3 CSS divergence fixtures (GXF-001).
//!
//! Previously `ThemeCss` had a single `gtk_css` string that the writer
//! dropped into *both* `gtk-4.0/gtk.css` and `gtk-3.0/gtk.css`. GTK3
//! reads a different token namespace (`@theme_bg_color` rather than
//! `@window_bg_color`) and a different widget tree (`.sidebar` rather
//! than `.navigation-sidebar`, no `.card`, no `splitview`, no
//! `popover > contents`), so the byte-identical output was mostly a
//! no-op and sometimes actively worse than vanilla Adwaita.
//!
//! These tests pin the core invariants of the divergence:
//!
//! 1. **Marker present.** Every generator's GTK3 output contains the
//!    literal `"GNOME X"` substring so the conflict detector doesn't
//!    false-positive-flag our own file the moment a user applies a theme.
//! 2. **Legacy tokens.** The GTK3 output defines the `@theme_*` tokens
//!    GTK3 actually honours, not the Libadwaita-only `@window_*` tokens.
//! 3. **No GTK4-only selectors.** The GTK3 output does NOT reference
//!    `.navigation-sidebar`, `.card` (as a standalone class),
//!    `splitview`, or `popover > contents`.
//! 4. **Divergence from GTK4.** The GTK3 stylesheet must not equal the
//!    GTK4 stylesheet — if it did, we'd have regressed on this issue.

use gnomex_domain::{HexColor, ShellVersion, ThemeSpec, WidgetColorOverrides};
use gnomex_infra::theme_css::create_css_generator;
use gnomex_infra::theme_css::gtk3::Gtk3CssGenerator;
use gnomex_app::ports::ThemeCssGenerator;

fn all_supported_versions() -> Vec<ShellVersion> {
    vec![
        ShellVersion::new(45, 0),
        ShellVersion::new(46, 0),
        ShellVersion::new(47, 0),
        ShellVersion::new(50, 0),
    ]
}

#[test]
fn gtk3_css_contains_managed_region_marker() {
    // If this assertion ever fails, the conflict detector will raise a
    // spurious "unmanaged gtk.css" warning for *every* user the moment
    // they apply a theme. The marker is load-bearing.
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&ThemeSpec::defaults()).unwrap();
        assert!(
            css.gtk3_css.contains("GNOME X"),
            "{}: gtk3_css is missing the `GNOME X` managed-region marker — \
             conflict detector will false-positive every user.\n----\n{}",
            generator.version_label(),
            css.gtk3_css,
        );
    }
}

#[test]
fn gtk3_css_uses_legacy_theme_tokens() {
    // `@theme_bg_color` / `@theme_base_color` are the tokens GTK3
    // actually honours. The Libadwaita `@window_bg_color` / `@view_bg_color`
    // pair is a no-op under GTK3.
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&ThemeSpec::defaults()).unwrap();
        assert!(
            css.gtk3_css.contains("@define-color theme_bg_color"),
            "{}: gtk3_css does not define `@theme_bg_color`.\n----\n{}",
            generator.version_label(),
            css.gtk3_css,
        );
        assert!(
            css.gtk3_css.contains("@define-color theme_base_color"),
            "{}: gtk3_css does not define `@theme_base_color`.\n----\n{}",
            generator.version_label(),
            css.gtk3_css,
        );
        assert!(
            css.gtk3_css.contains("@define-color theme_selected_bg_color"),
            "{}: gtk3_css does not define `@theme_selected_bg_color`.\n----\n{}",
            generator.version_label(),
            css.gtk3_css,
        );
    }
}

#[test]
fn gtk3_css_omits_libadwaita_only_tokens() {
    // `@window_bg_color`, `@view_bg_color`, `@accent_bg_color`,
    // `@card_bg_color`, `@popover_bg_color` are Libadwaita-only named
    // colours; GTK3 logs CSS-parser warnings for every undefined one.
    let forbidden_define = [
        "@define-color window_bg_color",
        "@define-color view_bg_color",
        "@define-color card_bg_color",
        "@define-color popover_bg_color",
        "@define-color headerbar_backdrop_color",
    ];
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&ThemeSpec::defaults()).unwrap();
        for token in forbidden_define {
            assert!(
                !css.gtk3_css.contains(token),
                "{}: gtk3_css emits GTK4-only token `{token}`.\n----\n{}",
                generator.version_label(),
                css.gtk3_css,
            );
        }
    }
}

#[test]
fn gtk3_css_does_not_reference_gtk4_only_selectors() {
    // These selectors exist only in GTK4/Libadwaita. Emitting them
    // to GTK3 pollutes parser logs and achieves nothing.
    let forbidden_selectors = [
        ".navigation-sidebar",
        "splitview",
        "popover > contents",
    ];
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&ThemeSpec::defaults()).unwrap();
        for selector in forbidden_selectors {
            assert!(
                !css.gtk3_css.contains(selector),
                "{}: gtk3_css references GTK4-only selector `{selector}`.\n----\n{}",
                generator.version_label(),
                css.gtk3_css,
            );
        }
        // `.card` is tolerated inside documentation strings like
        // "GTK3 has no `.card` class", but must not appear as a CSS
        // selector on its own line.
        for line in css.gtk3_css.lines() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("/*") || trimmed.starts_with("*") {
                continue;
            }
            assert!(
                !trimmed.starts_with(".card "),
                "{}: gtk3_css uses `.card` as a selector (GTK3 has no `.card` class). Line: `{line}`\n----\n{}",
                generator.version_label(),
                css.gtk3_css,
            );
        }
    }
}

#[test]
fn gtk3_css_diverges_from_gtk4_css() {
    // The core promise of this feature: the two payloads are
    // different. If this assertion ever fails we've regressed all the
    // way back to byte-identical dual rendering.
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&ThemeSpec::defaults()).unwrap();
        assert_ne!(
            css.gtk_css, css.gtk3_css,
            "{}: gtk_css and gtk3_css are byte-identical — this is the bug GXF-001 fixes.",
            generator.version_label(),
        );
    }
}

#[test]
fn gtk3_css_translates_radius_knobs() {
    let spec = ThemeSpec::defaults();
    let window_r = spec.window_radius.as_i32();
    let element_r = spec.element_radius.as_i32();
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        assert!(
            css.gtk3_css.contains(&format!("border-radius: {window_r}px")),
            "{}: window radius {window_r}px missing from gtk3_css",
            generator.version_label(),
        );
        assert!(
            css.gtk3_css
                .contains(&format!("button {{ border-radius: {element_r}px; }}")),
            "{}: button border-radius ({element_r}px) missing from gtk3_css\n----\n{}",
            generator.version_label(),
            css.gtk3_css,
        );
    }
}

#[test]
fn gtk3_css_translates_headerbar_knobs() {
    let spec = ThemeSpec::defaults();
    let hb_height = spec.headerbar.min_height.as_i32();
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        // Must target both `headerbar` and `.titlebar` — GTK3 apps
        // written before GTK 3.10 still use `.titlebar`.
        assert!(
            css.gtk3_css.contains("headerbar, .titlebar"),
            "{}: gtk3_css headerbar selector missing `.titlebar` fallback\n----\n{}",
            generator.version_label(),
            css.gtk3_css,
        );
        assert!(
            css.gtk3_css
                .contains(&format!("min-height: {hb_height}px")),
            "{}: gtk3_css missing headerbar min-height {hb_height}px",
            generator.version_label(),
        );
    }
}

#[test]
fn gtk3_css_translates_sidebar_fg_override_to_gtk3_selector() {
    // The user's sidebar fg colour must hit `.sidebar` (the GTK3 class),
    // NOT `.navigation-sidebar` (the GTK4 class). If it hit only the
    // GTK4 class under GTK3 the user's colour would be silently ignored.
    let mut spec = ThemeSpec::defaults();
    spec.sidebar.fg_override = Some(HexColor::new("#abcdef").unwrap());
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        assert!(
            css.gtk3_css.contains(".sidebar") && css.gtk3_css.contains("#abcdef"),
            "{}: gtk3_css sidebar fg override missing\n----\n{}",
            generator.version_label(),
            css.gtk3_css,
        );
        // And the GTK4-only class must NOT be used.
        assert!(
            !css.gtk3_css.contains(".navigation-sidebar"),
            "{}: gtk3_css sidebar fg leaked onto GTK4-only class",
            generator.version_label(),
        );
    }
}

#[test]
fn gtk3_css_translates_layer_separation_to_gtk3_selectors() {
    let mut spec = ThemeSpec::defaults();
    spec.layers.headerbar_bottom = gnomex_domain::Radius::new(2.0).unwrap();
    spec.layers.sidebar_divider = gnomex_domain::Radius::new(1.0).unwrap();
    // content_contrast is intentionally left at 0 — GTK3 has no
    // `.content-pane` / `splitview` tree to attach contrast to.
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        assert!(
            css.gtk3_css.contains("headerbar, .titlebar { border-bottom: 2px"),
            "{}: gtk3_css headerbar-bottom divider missing\n----\n{}",
            generator.version_label(),
            css.gtk3_css,
        );
        assert!(
            css.gtk3_css.contains(".sidebar") && css.gtk3_css.contains("border-right: 1px"),
            "{}: gtk3_css sidebar divider missing\n----\n{}",
            generator.version_label(),
            css.gtk3_css,
        );
    }
}

#[test]
fn gtk3_css_widget_style_uses_gtk3_button_selector_without_libadwaita_filters() {
    // GTK3's button widget has `.flat` but not `.suggested-action` /
    // `.destructive-action` (those are Libadwaita classes). The
    // button-raise selector must therefore NOT carry those filters
    // (they'd silently match nothing in GTK3 and confuse future readers).
    let mut spec = ThemeSpec::defaults();
    spec.widget_style.button_raise = gnomex_domain::Opacity::from_fraction(0.5).unwrap();
    let generator = Gtk3CssGenerator;
    let css = generator.generate(&spec).unwrap();
    assert!(
        css.gtk3_css.contains("button:not(.flat)"),
        "gtk3_css button-raise selector missing `button:not(.flat)`\n----\n{}",
        css.gtk3_css,
    );
    assert!(
        !css.gtk3_css.contains(":not(.suggested-action)"),
        "gtk3_css leaks Libadwaita-only `.suggested-action` filter\n----\n{}",
        css.gtk3_css,
    );
    assert!(
        !css.gtk3_css.contains(":not(.destructive-action)"),
        "gtk3_css leaks Libadwaita-only `.destructive-action` filter\n----\n{}",
        css.gtk3_css,
    );
}

#[test]
fn gtk3_css_widget_colors_collapse_scheme_variants() {
    // GTK 3.24 has no `prefers-color-scheme` media query, so the
    // generator picks one variant based on the panel-tint luminance.
    // With the default dark panel tint, the dark-variant overrides win.
    let mut spec = ThemeSpec::defaults();
    spec.widget_colors = WidgetColorOverrides {
        button_bg_light: Some(HexColor::new("#ffffff").unwrap()),
        button_bg_dark: Some(HexColor::new("#111111").unwrap()),
        ..Default::default()
    };
    let generator = Gtk3CssGenerator;
    let css = generator.generate(&spec).unwrap();

    assert!(
        css.gtk3_css.contains("#111111"),
        "gtk3_css widget-colour override should pick dark-variant under default dark panel tint\n----\n{}",
        css.gtk3_css,
    );
    // GTK3 output has no @media guards for prefers-color-scheme —
    // pinning this invariant prevents a future refactor from silently
    // adding one (which would parse-error under GTK 3.24).
    assert!(
        !css.gtk3_css.contains("prefers-color-scheme"),
        "gtk3_css must not emit prefers-color-scheme guards — GTK 3.24 does not support them.\n----\n{}",
        css.gtk3_css,
    );
}

#[test]
fn gtk3_css_is_emitted_even_when_panel_tint_is_light() {
    // Regression guard: the scheme heuristic must not accidentally
    // short-circuit the generator to an empty string when the user
    // picks a pale panel tint.
    let mut spec = ThemeSpec::defaults();
    spec.panel.tint = HexColor::new("#f5f5f5").unwrap();
    let generator = Gtk3CssGenerator;
    let css = generator.generate(&spec).unwrap();
    assert!(
        css.gtk3_css.contains("GNOME X"),
        "GTK3 marker missing under light panel tint"
    );
    assert!(
        css.gtk3_css.contains("@define-color theme_bg_color"),
        "GTK3 tint tokens missing under light panel tint"
    );
}
