// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Layer separation CSS fixtures (GXF-032, GXF-044).
//!
//! The `LayerSeparationSpec` exposes three user-opt-in knobs for
//! visually separating the headerbar, sidebar, and content layers of
//! a Libadwaita window. These tests pin two invariants:
//!
//! 1. **Opt-in, not opt-out.** When all three knobs are at their
//!    defaults, the generator must emit *zero* layer-separation CSS
//!    so users who haven't asked for this get the Adwaita default
//!    behaviour unchanged.
//! 2. **Rules fire on all supported GNOME versions.** A user's
//!    "Headerbar bottom border = 2px" expectation must hold regardless
//!    of whether they're on GNOME 45, 46, 47+, or 50+.

use gnomex_domain::{LayerSeparationSpec, Opacity, Radius, ShellVersion, ThemeSpec};
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
fn layer_separation_zero_defaults_emit_no_rules() {
    // Regression guard: a user who has never touched the layer knobs
    // must get byte-identical GTK CSS (minus the new header-comment
    // section) compared to pre-LayerSeparationSpec output.
    let spec = ThemeSpec::defaults();
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        assert!(
            !css.gtk_css.contains("Layer separation"),
            "{}: default spec emitted a Layer Separation section — this must be opt-in.\n----\n{}",
            generator.version_label(),
            css.gtk_css,
        );
        assert!(
            !css.gtk_css.contains("border-bottom:"),
            "{}: default spec emitted a border-bottom rule.",
            generator.version_label(),
        );
        assert!(
            !css.gtk_css.contains("border-right:"),
            "{}: default spec emitted a border-right rule.",
            generator.version_label(),
        );
    }
}

#[test]
fn layer_headerbar_bottom_emits_border_rule() {
    let mut spec = ThemeSpec::defaults();
    spec.layers = LayerSeparationSpec {
        headerbar_bottom: Radius::new(2.0).unwrap(),
        sidebar_divider: Radius::new(0.0).unwrap(),
        content_contrast: Opacity::from_fraction(0.0).unwrap(),
    };
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        assert!(
            css.gtk_css
                .contains("headerbar { border-bottom: 2px solid @borders; }"),
            "{}: missing headerbar bottom border\n----\n{}",
            generator.version_label(),
            css.gtk_css,
        );
        // Flat headerbars should also pick up the rule — otherwise
        // AdwHeaderBar.flat-style windows stay visually merged.
        assert!(
            css.gtk_css
                .contains("headerbar.flat { border-bottom: 2px solid @borders; }"),
            "{}: missing headerbar.flat bottom border",
            generator.version_label(),
        );
    }
}

#[test]
fn layer_sidebar_divider_targets_nautilus_and_splitview() {
    // The divider must apply to both the Libadwaita split-view
    // (`.sidebar-pane`) and the raw `.navigation-sidebar` class used
    // by Nautilus / Files / Disks — otherwise the knob only works in
    // half the apps that have a sidebar.
    let mut spec = ThemeSpec::defaults();
    spec.layers.sidebar_divider = Radius::new(1.0).unwrap();
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        assert!(
            css.gtk_css.contains(".sidebar-pane")
                && css.gtk_css.contains(".navigation-sidebar")
                && css.gtk_css.contains("border-right: 1px solid @borders"),
            "{}: sidebar-divider must target both .sidebar-pane and .navigation-sidebar\n----\n{}",
            generator.version_label(),
            css.gtk_css,
        );
    }
}

#[test]
fn layer_content_contrast_emits_scheme_specific_tints() {
    // Content contrast darkens in light mode and lightens in dark mode,
    // so the content column always reads as "above" the window chrome.
    // Both scheme variants must be present.
    let mut spec = ThemeSpec::defaults();
    spec.layers.content_contrast = Opacity::from_fraction(0.08).unwrap();
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        assert!(
            css.gtk_css
                .contains("mix(@view_bg_color, black, 0.080)"),
            "{}: light-mode content contrast tint missing\n----\n{}",
            generator.version_label(),
            css.gtk_css,
        );
        assert!(
            css.gtk_css
                .contains("mix(@view_bg_color, white, 0.080)"),
            "{}: dark-mode content contrast tint missing",
            generator.version_label(),
        );
    }
}

#[test]
fn layer_separation_all_three_knobs_compose() {
    // Dialling all three simultaneously must produce all three rules
    // without any of them cancelling the others out.
    let mut spec = ThemeSpec::defaults();
    spec.layers = LayerSeparationSpec {
        headerbar_bottom: Radius::new(1.0).unwrap(),
        sidebar_divider: Radius::new(2.0).unwrap(),
        content_contrast: Opacity::from_fraction(0.1).unwrap(),
    };
    let generator = create_css_generator(&ShellVersion::new(47, 0));
    let css = generator.generate(&spec).unwrap();
    let gtk = &css.gtk_css;
    assert!(gtk.contains("headerbar { border-bottom: 1px solid @borders; }"));
    assert!(gtk.contains("border-right: 2px solid @borders"));
    assert!(gtk.contains("mix(@view_bg_color, black, 0.100)"));
    assert!(gtk.contains("mix(@view_bg_color, white, 0.100)"));
}
