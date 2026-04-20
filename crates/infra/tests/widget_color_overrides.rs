// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Per-widget, per-scheme colour override fixtures (GXF-040).
//!
//! `WidgetColorOverrides` lets the user pin button / entry / headerbar /
//! sidebar backgrounds for light mode and dark mode independently. The
//! critical invariants this test pins:
//!
//! 1. **Opt-in.** No fields set = zero override CSS emitted, so default
//!    themes stay byte-compatible.
//! 2. **Last-wins ordering.** The override block must appear *after*
//!    the accent-tint block so a later `@define-color` redefinition
//!    actually beats the tint default. (CSS `@define-color` is
//!    last-wins in Libadwaita.)
//! 3. **Scheme isolation.** A light-only override must not leak into
//!    the dark block, and vice-versa. Otherwise users who set a warm
//!    button in light mode get the same warm button fighting a dark
//!    wallpaper at night.
//! 4. **Version-universal.** Overrides fire on every supported GNOME
//!    version — the feature shouldn't regress on version bumps.

use gnomex_domain::{HexColor, ShellVersion, ThemeSpec, WidgetColorOverrides};
use gnomex_infra::theme_css::create_css_generator;

fn all_supported_versions() -> Vec<ShellVersion> {
    vec![
        ShellVersion::new(45, 0),
        ShellVersion::new(46, 0),
        ShellVersion::new(47, 0),
        ShellVersion::new(50, 0),
    ]
}

#[test]
fn widget_colors_default_emits_no_override_block() {
    let spec = ThemeSpec::defaults();
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        assert!(
            !css.gtk_css.contains("Per-widget colour overrides"),
            "{}: default spec emitted an override block — must be opt-in",
            generator.version_label(),
        );
    }
}

#[test]
fn widget_colors_light_only_does_not_leak_into_dark() {
    let mut spec = ThemeSpec::defaults();
    spec.widget_colors = WidgetColorOverrides {
        button_bg_light: Some(HexColor::new("#ffaa22").unwrap()),
        ..Default::default()
    };
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        let (light, dark) = split_scopes_after_override(&css.gtk_css)
            .unwrap_or_else(|| panic!("{}: scopes missing", generator.version_label()));
        assert!(
            light.contains("@define-color button_bg_color #ffaa22"),
            "{}: light-mode override missing\n----\n{light}",
            generator.version_label(),
        );
        assert!(
            !dark.contains("button_bg_color #ffaa22"),
            "{}: light-mode override leaked into dark scope\n----\n{dark}",
            generator.version_label(),
        );
    }
}

#[test]
fn widget_colors_all_widgets_emit_all_eight_slots() {
    let mut spec = ThemeSpec::defaults();
    spec.widget_colors = WidgetColorOverrides {
        button_bg_light: Some(HexColor::new("#ff0001").unwrap()),
        button_bg_dark: Some(HexColor::new("#ff0002").unwrap()),
        entry_bg_light: Some(HexColor::new("#ff0003").unwrap()),
        entry_bg_dark: Some(HexColor::new("#ff0004").unwrap()),
        headerbar_bg_light: Some(HexColor::new("#ff0005").unwrap()),
        headerbar_bg_dark: Some(HexColor::new("#ff0006").unwrap()),
        sidebar_bg_light: Some(HexColor::new("#ff0007").unwrap()),
        sidebar_bg_dark: Some(HexColor::new("#ff0008").unwrap()),
    };
    let generator = create_css_generator(&ShellVersion::new(47, 0));
    let css = generator.generate(&spec).unwrap();
    let gtk = &css.gtk_css;
    for hex in &[
        "#ff0001", "#ff0002", "#ff0003", "#ff0004", "#ff0005", "#ff0006", "#ff0007", "#ff0008",
    ] {
        assert!(
            gtk.contains(hex),
            "missing override hex `{hex}`\n----\n{gtk}",
        );
    }
}

#[test]
fn widget_colors_override_block_appears_after_tint_block() {
    // If the override block lands *before* the tint block, the tint's
    // later `@define-color sidebar_bg_color mix(...)` wins and the
    // user's override silently does nothing. Pin the ordering.
    let mut spec = ThemeSpec::defaults();
    spec.widget_colors.sidebar_bg_light = Some(HexColor::new("#deadbe").unwrap());
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        let gtk = &css.gtk_css;

        let tint_pos = gtk
            .find("/* Accent-tinted surfaces */")
            .unwrap_or_else(|| panic!("{}: accent tint block missing", generator.version_label()));
        let override_pos = gtk
            .find("/* Per-widget colour overrides */")
            .unwrap_or_else(|| panic!("{}: override block missing", generator.version_label()));

        assert!(
            override_pos > tint_pos,
            "{}: override block (pos {override_pos}) must come AFTER tint block (pos {tint_pos}) — @define-color is last-wins",
            generator.version_label(),
        );
    }
}

// -- helpers ------------------------------------------------------------

/// Isolate the override block's two `@media` scopes so we can assert
/// one scheme contains the override and the other doesn't.
fn split_scopes_after_override(gtk: &str) -> Option<(String, String)> {
    let start = gtk.find("/* Per-widget colour overrides */")?;
    let block = &gtk[start..];
    let light = block.find("prefers-color-scheme: light")?;
    let dark = block.find("prefers-color-scheme: dark");
    let light_slice = if let Some(dark_pos) = dark {
        &block[light..dark_pos]
    } else {
        &block[light..]
    };
    let dark_slice = if let Some(dark_pos) = dark {
        &block[dark_pos..]
    } else {
        ""
    };
    Some((light_slice.to_string(), dark_slice.to_string()))
}
