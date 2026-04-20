// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Widget-style CSS fixtures (GXF-041, GXF-042, GXF-043).
//!
//! `WidgetStyleSpec` exposes three opt-in knobs for restoring
//! traditional widget affordances against the flat Adwaita baseline.
//! These tests pin three invariants:
//!
//! 1. **Opt-in.** All knobs at 0 yields zero widget-style CSS.
//! 2. **Version-universal.** Rules must fire on every supported
//!    GNOME version — the complaint "buttons look flat" doesn't
//!    care which version we detected.
//! 3. **Scope-safe.** The button rule must skip `.flat`,
//!    `.suggested-action`, and `.destructive-action` so we don't
//!    re-frame widgets Libadwaita deliberately draws flat (flat
//!    toolbar buttons, pill-shaped suggested-action primaries).

use gnomex_domain::{Opacity, ShellVersion, ThemeSpec, WidgetStyleSpec};
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
fn widget_style_zero_defaults_emit_no_rules() {
    let spec = ThemeSpec::defaults();
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        assert!(
            !css.gtk_css.contains("Widget style"),
            "{}: default spec emitted a Widget Style section — must be opt-in.\n----\n{}",
            generator.version_label(),
            css.gtk_css,
        );
        assert!(!css.gtk_css.contains("linear-gradient("));
    }
}

#[test]
fn widget_input_inset_emits_scheme_specific_entries() {
    let mut spec = ThemeSpec::defaults();
    spec.widget_style.input_inset = Opacity::from_fraction(0.5).unwrap();
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        let gtk = &css.gtk_css;
        assert!(
            gtk.contains("entry, entry.flat, spinbutton, spinbutton.flat"),
            "{}: input-inset selector missing\n----\n{}",
            generator.version_label(),
            gtk,
        );
        // Both scheme blocks must be present — inputs should look
        // depressed under both light and dark colour schemes.
        assert!(
            gtk.contains("mix(@view_bg_color, black,")
                && gtk.contains("mix(@view_bg_color, white,"),
            "{}: input-inset must emit both light-mode and dark-mode shifts",
            generator.version_label(),
        );
    }
}

#[test]
fn widget_button_raise_skips_flat_and_accent_variants() {
    let mut spec = ThemeSpec::defaults();
    spec.widget_style.button_raise = Opacity::from_fraction(0.6).unwrap();
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        let gtk = &css.gtk_css;
        // The selector must carry all three :not() filters; otherwise
        // we re-frame toolbar flat buttons and re-border the
        // accent-filled suggested-action primary.
        assert!(
            gtk.contains("button:not(.flat):not(.suggested-action):not(.destructive-action)"),
            "{}: button-raise selector missing or lacks scope filters\n----\n{}",
            generator.version_label(),
            gtk,
        );
        assert!(
            gtk.contains(":active"),
            "{}: button-raise missing :active pressed state",
            generator.version_label(),
        );
    }
}

#[test]
fn widget_headerbar_gradient_emits_both_schemes() {
    let mut spec = ThemeSpec::defaults();
    spec.widget_style.headerbar_gradient = Opacity::from_fraction(0.4).unwrap();
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        let gtk = &css.gtk_css;
        assert!(
            gtk.contains("headerbar, .toolbar"),
            "{}: gradient rule must cover both headerbars and toolbars",
            generator.version_label(),
        );
        assert!(
            gtk.contains("linear-gradient(to bottom,"),
            "{}: linear-gradient missing",
            generator.version_label(),
        );
        // Light darkens at the bottom, dark lightens at the top — so
        // both variants must contain mix-with-black *and* mix-with-white
        // somewhere in the gradient block.
        assert!(gtk.contains("mix(@headerbar_bg_color, black,"));
        assert!(gtk.contains("mix(@headerbar_bg_color, white,"));
    }
}

#[test]
fn widget_style_all_three_knobs_compose() {
    let mut spec = ThemeSpec::defaults();
    spec.widget_style = WidgetStyleSpec {
        input_inset: Opacity::from_fraction(0.5).unwrap(),
        button_raise: Opacity::from_fraction(0.5).unwrap(),
        headerbar_gradient: Opacity::from_fraction(0.5).unwrap(),
    };
    let generator = create_css_generator(&ShellVersion::new(47, 0));
    let css = generator.generate(&spec).unwrap();
    let gtk = &css.gtk_css;
    assert!(gtk.contains("entry, entry.flat"));
    assert!(gtk.contains("button:not(.flat)"));
    assert!(gtk.contains("linear-gradient("));
}
