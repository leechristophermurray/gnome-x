// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Nautilus-specific CSS fixture (GXF-023).
//!
//! Nautilus has a non-trivial widget tree — the left navigation sidebar,
//! the path-bar embedded in the headerbar, and the main content view each
//! consume a different Adwaita token. This test pins the tokens we must
//! emit so a future refactor of the theme CSS generator cannot silently
//! break Nautilus without a loud test failure.
//!
//! The left sidebar in particular relies on `sidebar_bg_color` rendering
//! *behind* Nautilus's own semi-transparent overlay. If the token
//! disappears or is no longer defined in both light and dark scopes, the
//! sidebar falls back to the default Adwaita colour and our accent tint
//! stops taking effect on the sidebar — a visually jarring regression.

use gnomex_domain::{HexColor, Opacity, ShellVersion, SidebarSpec, ThemeSpec};
use gnomex_infra::theme_css::create_css_generator;

/// Tokens Nautilus's widget tree consumes from our generated GTK CSS.
///
/// | Nautilus widget        | Adwaita token we emit        |
/// |------------------------|------------------------------|
/// | `.navigation-sidebar`  | `sidebar_bg_color`           |
/// | path-bar (in headerbar)| `headerbar_bg_color`         |
/// | main content view      | `view_bg_color`              |
/// | rubber-band / cards    | `card_bg_color`              |
/// | window chrome          | `window_bg_color`            |
const NAUTILUS_REQUIRED_TOKENS: &[&str] = &[
    "sidebar_bg_color",
    "headerbar_bg_color",
    "view_bg_color",
    "card_bg_color",
    "window_bg_color",
];

fn all_supported_versions() -> Vec<ShellVersion> {
    vec![
        ShellVersion::new(45, 0),
        ShellVersion::new(46, 0),
        ShellVersion::new(47, 0),
        ShellVersion::new(50, 0),
    ]
}

#[test]
fn nautilus_required_tokens_are_defined_in_light_and_dark() {
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator
            .generate(&ThemeSpec::defaults())
            .expect("CSS generation must succeed with defaults");

        let (light, dark) = split_light_dark_scopes(&css.gtk_css).unwrap_or_else(|| {
            panic!(
                "{}: GTK CSS does not contain both light and dark @media scopes\n----\n{}",
                generator.version_label(),
                css.gtk_css,
            )
        });

        for token in NAUTILUS_REQUIRED_TOKENS {
            let needle = format!("@define-color {token}");
            assert!(
                light.contains(&needle),
                "{}: light scope is missing `{needle}` — Nautilus will fall back to the default Adwaita colour.\n----\n{light}",
                generator.version_label(),
            );
            assert!(
                dark.contains(&needle),
                "{}: dark scope is missing `{needle}` — Nautilus will fall back to the default Adwaita colour.\n----\n{dark}",
                generator.version_label(),
            );
        }
    }
}

#[test]
fn nautilus_sidebar_uses_accent_blend_not_hardcoded_grey() {
    // Nautilus's navigation sidebar is what users notice first when a
    // theme "doesn't look applied". The tint formula must reference
    // `@accent_bg_color` so the sidebar re-tints when the user switches
    // accents, rather than being baked to a grey.
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&ThemeSpec::defaults()).unwrap();

        let sidebar_line = css
            .gtk_css
            .lines()
            .find(|l| l.contains("@define-color sidebar_bg_color"))
            .unwrap_or_else(|| {
                panic!(
                    "{}: no `sidebar_bg_color` definition at all\n----\n{}",
                    generator.version_label(),
                    css.gtk_css,
                )
            });

        assert!(
            sidebar_line.contains("@accent_bg_color"),
            "{}: sidebar_bg_color does not blend with @accent_bg_color — line was `{sidebar_line}`",
            generator.version_label(),
        );
        assert!(
            sidebar_line.contains("mix("),
            "{}: sidebar_bg_color is not a `mix()` expression — accent intensity won't scale. Line: `{sidebar_line}`",
            generator.version_label(),
        );
    }
}

#[test]
fn nautilus_content_view_contrast_separate_from_window() {
    // Nautilus's file list sits in a `view` widget; users expect it to be
    // subtly brighter than the window chrome behind it so folder rows are
    // legible. The generator must emit a distinct `view_bg_color` token
    // rather than aliasing it to `window_bg_color`.
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&ThemeSpec::defaults()).unwrap();
        let gtk = &css.gtk_css;

        let window_def = first_line_containing(gtk, "@define-color window_bg_color");
        let view_def = first_line_containing(gtk, "@define-color view_bg_color");

        let (Some(window_def), Some(view_def)) = (window_def, view_def) else {
            panic!(
                "{}: window_bg_color or view_bg_color missing from GTK CSS\n----\n{}",
                generator.version_label(),
                gtk,
            );
        };

        assert_ne!(
            strip_base_color(window_def),
            strip_base_color(view_def),
            "{}: window_bg_color and view_bg_color share the same base — Nautilus rows will be indistinguishable from chrome.",
            generator.version_label(),
        );
    }
}

#[test]
fn nautilus_card_border_consistent_with_element_radius() {
    // Nautilus uses card-styled sidebars for "Starred", "Recent" etc.
    // The card border-radius must follow `element_radius` so it matches
    // buttons/entries in the toolbar — otherwise the sidebar feels
    // visually disjoint.
    let spec = ThemeSpec::defaults();
    let expected_radius = format!(".card {{ border-radius: {}px; }}", spec.element_radius.as_i32());

    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        assert!(
            css.gtk_css.contains(&expected_radius),
            "{}: `.card` border-radius rule does not match element_radius ({}px)\n----\n{}",
            generator.version_label(),
            spec.element_radius.as_i32(),
            css.gtk_css,
        );
    }
}

#[test]
fn nautilus_sidebar_opacity_wraps_mix_in_alpha_when_under_one() {
    // Opacity == 1.0 must stay a plain mix() (readable CSS);
    // Opacity < 1.0 must wrap the mix() in alpha(mix(...), pct).
    // This is the core semi-transparency feature users tune to let a
    // blurred wallpaper bleed into the Nautilus sidebar.
    let mut spec = ThemeSpec::defaults();
    spec.sidebar = SidebarSpec {
        opacity: Opacity::from_fraction(0.7).unwrap(),
        fg_override: None,
    };

    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        let sidebar_line = css
            .gtk_css
            .lines()
            .find(|l| l.contains("@define-color sidebar_bg_color"))
            .unwrap();
        assert!(
            sidebar_line.contains("alpha("),
            "{}: sidebar_bg_color with opacity<1.0 must be alpha()-wrapped. Line: `{sidebar_line}`",
            generator.version_label(),
        );
        assert!(
            sidebar_line.contains("0.700"),
            "{}: sidebar_bg_color alpha value missing. Line: `{sidebar_line}`",
            generator.version_label(),
        );
    }
}

#[test]
fn nautilus_sidebar_opacity_one_emits_plain_mix() {
    // Regression guard: when the user leaves sidebar opacity at default
    // (1.0), we must not emit an alpha() wrapper — otherwise every theme
    // applies a one-line diff versus historical output.
    let spec = ThemeSpec::defaults();
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        let sidebar_line = css
            .gtk_css
            .lines()
            .find(|l| l.contains("@define-color sidebar_bg_color"))
            .unwrap();
        assert!(
            !sidebar_line.contains("alpha("),
            "{}: opacity=1.0 must not wrap in alpha(). Line: `{sidebar_line}`",
            generator.version_label(),
        );
    }
}

#[test]
fn nautilus_sidebar_fg_override_emits_when_set() {
    // When the user picks a sidebar text colour, we must emit
    // `@define-color sidebar_fg_color #rrggbb;` so Nautilus actually
    // picks it up. When the override is None, no line is emitted and
    // Nautilus falls back to Adwaita's scheme-dependent default.
    let mut spec = ThemeSpec::defaults();
    spec.sidebar = SidebarSpec {
        opacity: Opacity::from_fraction(1.0).unwrap(),
        fg_override: Some(HexColor::new("#ff00aa").unwrap()),
    };

    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&spec).unwrap();
        assert!(
            css.gtk_css.contains("@define-color sidebar_fg_color #ff00aa"),
            "{}: sidebar_fg_color override missing from GTK CSS\n----\n{}",
            generator.version_label(),
            css.gtk_css,
        );
    }

    // And with the default (None), no sidebar_fg_color line is emitted.
    let default_spec = ThemeSpec::defaults();
    for version in all_supported_versions() {
        let generator = create_css_generator(&version);
        let css = generator.generate(&default_spec).unwrap();
        assert!(
            !css.gtk_css.contains("@define-color sidebar_fg_color"),
            "{}: sidebar_fg_color emitted when fg_override is None — would clobber Adwaita defaults.",
            generator.version_label(),
        );
    }
}

// -- helpers -----------------------------------------------------------

fn split_light_dark_scopes(gtk_css: &str) -> Option<(String, String)> {
    let light_start = gtk_css.find("@media (prefers-color-scheme: light)")?;
    let dark_start = gtk_css.find("@media (prefers-color-scheme: dark)")?;
    if dark_start <= light_start {
        return None;
    }
    Some((
        gtk_css[light_start..dark_start].to_string(),
        gtk_css[dark_start..].to_string(),
    ))
}

fn first_line_containing<'a>(haystack: &'a str, needle: &str) -> Option<&'a str> {
    haystack.lines().find(|l| l.contains(needle))
}

/// Drop everything up to and including the first `mix(` open-paren so two
/// token definitions can be compared by their base colour. Without this
/// the alpha/blend fractions vary token-to-token and always look unequal.
fn strip_base_color(line: &str) -> String {
    let after_mix = line.split_once("mix(").map(|(_, r)| r).unwrap_or(line);
    let first_arg = after_mix.split(',').next().unwrap_or("").trim();
    first_arg.to_string()
}
