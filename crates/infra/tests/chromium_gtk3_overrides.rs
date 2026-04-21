// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration test for GXF-020: Chromium/Electron GTK3 CSS sidecar.
//!
//! Pins the contract between `ChromiumThemer` and the GTK3 snippet it
//! emits alongside the manifest:
//!
//! 1. The snippet contains the `"GNOME X"` conflict-detector marker.
//! 2. Every scoped Chromium native-chrome selector is present
//!    (scrollbar, menu/menuitem, filechooser).
//! 3. The `ExternalThemeSpec.accent` hex (and its RGB triple) is
//!    embedded verbatim so the rules actually tint to the user's
//!    accent rather than a stale default.
//! 4. Scheme flips produce different output (so a user toggling
//!    dark/light isn't stuck with the wrong surface colour).
//!
//! These invariants sit in `tests/` rather than `#[cfg(test)] mod tests`
//! so they gate the public `build_chromium_gtk3_css` surface the way a
//! downstream caller would see it.

use gnomex_domain::{ColorScheme, ExternalThemeSpec, HexColor};
use gnomex_infra::build_chromium_gtk3_css;

fn dark_spec() -> ExternalThemeSpec {
    ExternalThemeSpec {
        accent: HexColor::new("#e66100").unwrap(), // Adwaita orange
        panel_tint: HexColor::new("#1a1a1e").unwrap(),
        color_scheme: ColorScheme::Dark,
    }
}

fn light_spec() -> ExternalThemeSpec {
    ExternalThemeSpec {
        accent: HexColor::new("#3584e4").unwrap(), // Adwaita blue
        panel_tint: HexColor::new("#f6f5f4").unwrap(),
        color_scheme: ColorScheme::Light,
    }
}

#[test]
fn snippet_carries_managed_region_marker() {
    let css = build_chromium_gtk3_css(&dark_spec());
    assert!(
        css.contains("GNOME X"),
        "Chromium GTK3 sidecar missing `GNOME X` marker — conflict \
         detector (PR #40) will flag the file as user-authored on every \
         subsequent apply:\n{css}",
    );
}

#[test]
fn snippet_targets_only_chromium_native_chrome_selectors() {
    // This test pins the scope boundary: Chromium's native-chrome
    // surfaces are just three things. If we drift into generic
    // `headerbar`/`entry`/`button` styling we're re-implementing the
    // GTK3 pipeline (`theme_css::gtk3`) and leaking into every GTK3
    // app on the system.
    let css = build_chromium_gtk3_css(&dark_spec());

    // In-scope selectors MUST be present.
    for selector in [
        "scrollbar",
        "scrollbar slider",
        "menu",
        "menuitem",
        "filechooser",
    ] {
        assert!(
            css.contains(selector),
            "expected scoped selector `{selector}` in Chromium GTK3 \
             sidecar:\n{css}",
        );
    }
}

#[test]
fn snippet_embeds_accent_hex_from_spec_dark() {
    let css = build_chromium_gtk3_css(&dark_spec());
    assert!(
        css.contains("#e66100"),
        "dark sidecar didn't embed accent hex `#e66100`:\n{css}",
    );
    // `to_rgb` of #e66100 is (230, 97, 0) — appears formatted as
    // decimal triples inside `rgba(...)` hover/active rules.
    assert!(
        css.contains("230, 97, 0"),
        "dark sidecar didn't embed accent RGB triple:\n{css}",
    );
}

#[test]
fn snippet_embeds_accent_hex_from_spec_light() {
    let css = build_chromium_gtk3_css(&light_spec());
    assert!(
        css.contains("#3584e4"),
        "light sidecar didn't embed accent hex `#3584e4`:\n{css}",
    );
    assert!(
        css.contains("53, 132, 228"),
        "light sidecar didn't embed accent RGB triple:\n{css}",
    );
}

#[test]
fn snippet_surface_palette_flips_with_scheme() {
    let dark = build_chromium_gtk3_css(&dark_spec());
    let light = build_chromium_gtk3_css(&light_spec());

    // Dark surface = #242424 (libadwaita window bg dark).
    assert!(
        dark.contains("#242424"),
        "dark sidecar missing expected surface hex #242424:\n{dark}",
    );
    // Light surface = #fafafa.
    assert!(
        light.contains("#fafafa"),
        "light sidecar missing expected surface hex #fafafa:\n{light}",
    );

    assert_ne!(
        dark, light,
        "scheme flip produced byte-identical output — surface palette \
         probably didn't switch",
    );
}

#[test]
fn snippet_does_not_leak_generic_gtk3_selectors() {
    // Opposite of the scope test: if we start styling widgets outside
    // Chromium's native chrome we're racing with
    // `theme_css::gtk3::generate_gtk3_css` for ownership of the same
    // selectors, and every GTK3 app on the system picks up our tweaks
    // whether or not the user wanted them.
    let css = build_chromium_gtk3_css(&dark_spec());
    for wide_selector in [
        "headerbar",
        ".titlebar",
        "window.csd",
        "button:not(.flat)",
        "@define-color theme_bg_color",
        "@define-color window_bg_color",
    ] {
        assert!(
            !css.contains(wide_selector),
            "Chromium sidecar leaked out-of-scope selector \
             `{wide_selector}` — this belongs in the main GTK3 \
             pipeline, not the Chromium sidecar:\n{css}",
        );
    }
}
