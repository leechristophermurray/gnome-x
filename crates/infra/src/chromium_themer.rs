// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Chromium-family adapter.
//!
//! Chromium removed user-stylesheet support in 2014, and its native
//! GTK integration on Linux only covers scrollbars. The only real
//! runtime theming surface left is a Chromium **unpacked theme
//! extension**: a folder with a `manifest.json` declaring theme colors
//! that the user loads once via `chrome://extensions` → *Load unpacked*.
//!
//! We generate that folder into `~/.local/share/gnome-x/chromium-theme/`
//! and keep it in sync whenever the accent or color scheme changes.
//! The user loads it once; each reapplication overwrites the file in
//! place, and the user reloads the extension to pick up new colors.
//!
//! We also detect which Chromium-family browsers are installed (Chrome,
//! Chromium, Brave, Edge, Vivaldi) just so the log message can point at
//! the right `chrome://extensions` URL set.
//!
//! ## GTK 3 overrides sidecar (GXF-020)
//!
//! When launched with `--ozone-platform=wayland` (or under GTK's X11
//! backend), Chromium-family browsers still link against **GTK 3** for
//! their native, non-web-content chrome: native scrollbars (when
//! `--use-gtk-scrollbars` or the default), right-click context menus,
//! and the native file-picker dialog. These surfaces read
//! `~/.config/gtk-3.0/gtk.css` like any other GTK3 app, but only a
//! narrow handful of selectors take effect on them. We emit a
//! purpose-built snippet (`chromium.gtk3.css`) alongside the manifest
//! containing just those selectors, tinted from the `ExternalThemeSpec`.
//! The snippet sits in our theme dir so users can `@import` it into
//! their own GTK3 override without fighting the main theme writer's
//! ownership of `gtk-3.0/gtk.css` (GXF-001). Scope is deliberately tiny
//! — scrollbar track/thumb, context menu bg/fg, file-chooser window bg
//! — so we don't re-implement the whole GTK3 pipeline
//! (that's `theme_css::gtk3`).

use gnomex_app::ports::ExternalAppThemer;
use gnomex_app::AppError;
use gnomex_domain::{ColorScheme, ExternalThemeSpec};
use serde_json::{json, Value};
use std::path::PathBuf;

const THEME_SUBDIR: &str = ".local/share/gnome-x/chromium-theme";
const MANIFEST_NAME: &str = "manifest.json";
const GTK3_OVERRIDES_NAME: &str = "chromium.gtk3.css";

/// Header embedded in the Chromium GTK3 sidecar. Contains the literal
/// `"GNOME X"` substring the conflict detector (PR #40) greps for so
/// the file is recognised as managed rather than a user override.
const CHROMIUM_GTK3_HEADER: &str =
    "/* GNOME X — Chromium/Electron GTK3 overrides (GXF-020) */";

struct ChromiumFlavor {
    name: &'static str,
    config_dir: &'static str,
}

const FLAVORS: &[ChromiumFlavor] = &[
    ChromiumFlavor { name: "Google Chrome",    config_dir: "google-chrome" },
    ChromiumFlavor { name: "Chromium",         config_dir: "chromium" },
    ChromiumFlavor { name: "Brave",            config_dir: "BraveSoftware/Brave-Browser" },
    ChromiumFlavor { name: "Microsoft Edge",   config_dir: "microsoft-edge" },
    ChromiumFlavor { name: "Vivaldi",          config_dir: "vivaldi" },
];

pub struct ChromiumThemer {
    home: PathBuf,
}

impl ChromiumThemer {
    pub fn new() -> Self {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        Self { home }
    }

    fn theme_dir(&self) -> PathBuf {
        self.home.join(THEME_SUBDIR)
    }

    fn detect_flavors(&self) -> Vec<&'static str> {
        FLAVORS
            .iter()
            .filter(|f| self.home.join(".config").join(f.config_dir).is_dir())
            .map(|f| f.name)
            .collect()
    }
}

impl Default for ChromiumThemer {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalAppThemer for ChromiumThemer {
    fn name(&self) -> &str {
        "chromium"
    }

    fn apply(&self, spec: &ExternalThemeSpec) -> Result<(), AppError> {
        let flavors = self.detect_flavors();
        if flavors.is_empty() {
            tracing::debug!("chromium: no browser installations detected");
            return Ok(());
        }
        let dir = self.theme_dir();
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::Settings(format!("mkdir {}: {e}", dir.display())))?;

        let manifest = build_manifest(spec);
        let manifest_path = dir.join(MANIFEST_NAME);
        let pretty = serde_json::to_string_pretty(&manifest)
            .map_err(|e| AppError::Settings(format!("serialize manifest: {e}")))?;
        std::fs::write(&manifest_path, pretty)
            .map_err(|e| AppError::Settings(format!("write {}: {e}", manifest_path.display())))?;

        // Also emit a tiny GTK3 sidecar for the native chrome bits
        // Chromium still renders via GTK3 (scrollbars, context menus,
        // file-picker). Users opt in by `@import`ing it from their own
        // `~/.config/gtk-3.0/gtk.css` — we don't touch that file here
        // because `FilesystemThemeWriter` owns it.
        let gtk3_path = dir.join(GTK3_OVERRIDES_NAME);
        let gtk3_css = build_chromium_gtk3_css(spec);
        std::fs::write(&gtk3_path, &gtk3_css)
            .map_err(|e| AppError::Settings(format!("write {}: {e}", gtk3_path.display())))?;

        tracing::info!(
            "chromium: wrote theme to {} (detected: {})",
            dir.display(),
            flavors.join(", ")
        );
        tracing::info!(
            "chromium: load once via chrome://extensions → Developer mode → Load unpacked → {}",
            dir.display()
        );
        tracing::info!(
            "chromium: GTK3 sidecar at {} — `@import url(\"file://{}\");` from ~/.config/gtk-3.0/gtk.css to style native scrollbars/context menus/file-pickers",
            gtk3_path.display(),
            gtk3_path.display(),
        );
        Ok(())
    }

    fn reset(&self) -> Result<(), AppError> {
        let dir = self.theme_dir();
        if dir.exists() {
            std::fs::remove_dir_all(&dir)
                .map_err(|e| AppError::Settings(format!("rm {}: {e}", dir.display())))?;
            tracing::info!("chromium: removed {}", dir.display());
        }
        Ok(())
    }
}

/// Build a Chromium unpacked-theme `manifest.json` payload.
/// Colors are stored as `[r, g, b]` triples per Chromium's theme spec.
/// Reference: https://developer.chrome.com/docs/extensions/reference/manifest/theme
///
/// Palette strategy: match libadwaita neutrals for the frame, toolbar,
/// and NTP background. The user's accent is only applied to button
/// backgrounds and NTP links — the same restraint libadwaita shows.
/// This keeps browsers looking like "GNOME X apps" rather than flooding
/// the chrome with the panel tint.
fn build_manifest(spec: &ExternalThemeSpec) -> Value {
    let accent = spec.accent.to_rgb();
    let accent_rgb = [accent.0, accent.1, accent.2];

    let (chrome, toolbar, ntp_bg, fg, fg_dim) = if spec.color_scheme.is_dark() {
        (
            [46u8, 46, 46],   // #2e2e2e — libadwaita headerbar dark
            [36u8, 36, 36],   // #242424 — libadwaita window bg dark
            [36u8, 36, 36],
            [237u8, 237, 237],
            [154u8, 154, 154],
        )
    } else {
        (
            [235u8, 235, 235],
            [250u8, 250, 250],
            [250u8, 250, 250],
            [46u8, 46, 46],
            [107u8, 107, 107],
        )
    };

    json!({
        "manifest_version": 3,
        "name": "GNOME X Accent",
        "version": "1.0.0",
        "description": "Generated by GNOME X. libadwaita-neutral chrome with your accent on links and buttons.",
        "theme": {
            "colors": {
                // Window frame (titlebar + tab strip area) — neutral.
                "frame":                        chrome,
                "frame_inactive":               chrome,
                "frame_incognito":              chrome,
                "frame_incognito_inactive":     chrome,
                // Toolbar (address bar row) — slightly darker/lighter than frame.
                "toolbar":                      toolbar,
                "toolbar_text":                 fg,
                "toolbar_button_icon":          fg,
                // Tabs — active follows toolbar, inactive dims.
                "tab_text":                     fg,
                "tab_background_text":          fg_dim,
                "tab_background_text_inactive": fg_dim,
                // Omnibox (address bar) — neutral.
                "omnibox_background":           toolbar,
                "omnibox_text":                 fg,
                // Bookmarks bar.
                "bookmark_text":                fg,
                // New Tab Page — neutral bg, accent link.
                "ntp_background":               ntp_bg,
                "ntp_text":                     fg,
                "ntp_link":                     accent_rgb,
                // The one place accent floods: primary button bg.
                "button_background":            accent_rgb,
            },
            "properties": {
                "ntp_background_alignment": "center"
            }
        }
    })
}

/// Build a focused GTK3 CSS snippet covering the three surfaces
/// Chromium's native (non-web-content) chrome honours: scrollbars,
/// right-click context menus, and the native file-picker dialog.
///
/// We intentionally do **not** target `headerbar`, `entry`, `button`,
/// or tab selectors — Chromium renders those itself from the manifest
/// theme. Touching them here either no-ops or leaks styling into other
/// GTK3 apps. The rules use only selectors that GTK 3.24 accepts and
/// that Chromium's embedded GTK widgets actually instantiate.
///
/// Colour derivation:
/// * **Surface** (context-menu / file-picker bg) = libadwaita-neutral
///   window bg for the active scheme, matching the manifest's `frame`.
/// * **Headerbar** (scrollbar trough) = slightly darker/lighter than
///   surface, matching the manifest's `toolbar`.
/// * **Accent** (scrollbar thumb, focus ring) = the user's accent hex.
///
/// See `theme_css::gtk3::generate_gtk3_css` for the full GTK3 pipeline
/// — the selectors here overlap in concept but are deliberately
/// narrower and opt-in (via `@import`) so they don't pollute the
/// system-wide GTK3 override.
pub fn build_chromium_gtk3_css(spec: &ExternalThemeSpec) -> String {
    let accent = spec.accent.as_str();
    let (accent_r, accent_g, accent_b) = spec.accent.to_rgb();
    let (surface, surface_fg, headerbar) = palette(spec.color_scheme);

    format!(
        r#"{header}
/* Scope: native GTK3 widgets Chromium-family browsers (Chrome, Brave,
   Edge, Electron under --ozone-platform=wayland) still instantiate
   outside their web-content render pipeline. Pulled from
   ExternalThemeSpec: accent={accent}, scheme={scheme}. */

/* ---- Scrollbars (trough + slider/thumb) ---- */
/* GTK3 Chromium honours `scrollbar` and `scrollbar slider`. The
   `contents trough` tree is the GTK 3.24 widget path. */
scrollbar, scrollbar.vertical, scrollbar.horizontal {{
    background-color: {headerbar};
}}
scrollbar trough {{
    background-color: {headerbar};
    border-radius: 0;
}}
scrollbar slider {{
    background-color: rgba({accent_r}, {accent_g}, {accent_b}, 0.55);
    border: 2px solid transparent;
    border-radius: 8px;
    min-width: 6px;
    min-height: 6px;
}}
scrollbar slider:hover {{
    background-color: rgba({accent_r}, {accent_g}, {accent_b}, 0.80);
}}
scrollbar slider:active {{
    background-color: {accent};
}}

/* ---- Right-click context menus ---- */
/* Chromium opens context menus via GtkMenu; `menu` and `menuitem` are
   the GTK 3.24 selectors for the classic Xlib-backed menu widget. */
menu, .menu, menu > arrow, .context-menu {{
    background-color: {surface};
    color: {surface_fg};
    border: 1px solid alpha({surface_fg}, 0.15);
}}
menuitem, .menuitem {{
    background-color: transparent;
    color: {surface_fg};
    padding: 4px 8px;
}}
menuitem:hover, menuitem:focus, .menuitem:hover {{
    background-color: {accent};
    color: #ffffff;
}}
menu separator, .menu separator {{
    background-color: alpha({surface_fg}, 0.12);
    min-height: 1px;
}}

/* ---- Native file picker (GtkFileChooserDialog) ---- */
/* Chromium's "Save As" / "Open File" opens a GTK3 native dialog. The
   outer window + its .dialog-vbox control the overall background. */
filechooser, .filechooser, window.dialog.background {{
    background-color: {surface};
    color: {surface_fg};
}}
filechooser .sidebar, filechooser placessidebar {{
    background-color: {headerbar};
    color: {surface_fg};
}}
filechooser .sidebar row:selected,
filechooser placessidebar row:selected {{
    background-color: {accent};
    color: #ffffff;
}}

/* ---- Focus ring (URL-bar-adjacent dialogs) ---- */
*:focus {{
    outline-color: {accent};
}}
"#,
        header = CHROMIUM_GTK3_HEADER,
        scheme = if spec.color_scheme.is_dark() { "dark" } else { "light" },
    )
}

/// Return `(surface_bg, surface_fg, headerbar_bg)` for the given scheme.
/// Mirrors the manifest palette so the GTK3 sidecar blends with the
/// Chromium-extension theme rather than diverging.
fn palette(scheme: ColorScheme) -> (&'static str, &'static str, &'static str) {
    if scheme.is_dark() {
        // #242424 window bg, #ededed fg, #2e2e2e headerbar — matches
        // libadwaita dark and the manifest's `frame`/`toolbar`.
        ("#242424", "#ededed", "#2e2e2e")
    } else {
        ("#fafafa", "#2e2e2e", "#ebebeb")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gnomex_domain::HexColor;

    fn spec() -> ExternalThemeSpec {
        ExternalThemeSpec {
            accent: HexColor::new("#3584e4").unwrap(),
            panel_tint: HexColor::new("#1a1a1e").unwrap(),
            color_scheme: ColorScheme::Dark,
        }
    }

    #[test]
    fn manifest_has_required_theme_keys() {
        let m = build_manifest(&spec());
        assert_eq!(m["manifest_version"], 3);
        let colors = &m["theme"]["colors"];
        for key in &["frame", "toolbar", "ntp_link", "button_background"] {
            assert!(colors.get(*key).is_some(), "missing {key}");
        }
    }

    #[test]
    fn accent_rgb_round_trips() {
        let m = build_manifest(&spec());
        let link = &m["theme"]["colors"]["ntp_link"];
        assert_eq!(link[0], 0x35);
        assert_eq!(link[1], 0x84);
        assert_eq!(link[2], 0xe4);
    }

    #[test]
    fn dark_vs_light_flips_foreground() {
        let mut s = spec();
        let dark = build_manifest(&s);
        s.color_scheme = ColorScheme::Light;
        let light = build_manifest(&s);
        assert_ne!(
            dark["theme"]["colors"]["bookmark_text"],
            light["theme"]["colors"]["bookmark_text"]
        );
    }

    #[test]
    fn gtk3_css_contains_gnome_x_marker() {
        let css = build_chromium_gtk3_css(&spec());
        assert!(
            css.contains("GNOME X"),
            "Chromium GTK3 sidecar missing managed-region marker; \
             conflict detector (PR #40) will false-positive on every write.\n{css}",
        );
    }

    #[test]
    fn gtk3_css_targets_chromium_native_chrome_selectors() {
        let css = build_chromium_gtk3_css(&spec());
        // Scope pins: the three surfaces Chromium's non-web-content
        // chrome actually exposes to GTK3 styling.
        for selector in ["scrollbar", "scrollbar slider", "menu", "menuitem", "filechooser"] {
            assert!(
                css.contains(selector),
                "Chromium GTK3 sidecar missing `{selector}` — \
                 user-visible native chrome will render un-themed.\n{css}",
            );
        }
    }

    #[test]
    fn gtk3_css_embeds_accent_hex_from_spec() {
        let css = build_chromium_gtk3_css(&spec());
        // Accent shows up both as the literal hex (slider:active,
        // focus outline) and as an RGB triple (translucent hovers).
        assert!(
            css.contains("#3584e4"),
            "Chromium GTK3 sidecar didn't embed the spec's accent hex"
        );
        assert!(
            css.contains("53, 132, 228"),
            "Chromium GTK3 sidecar didn't embed the spec's accent RGB triple"
        );
    }

    #[test]
    fn gtk3_css_palette_flips_with_scheme() {
        let mut s = spec();
        let dark = build_chromium_gtk3_css(&s);
        s.color_scheme = ColorScheme::Light;
        let light = build_chromium_gtk3_css(&s);
        // Dark surface is the libadwaita window bg (#242424); light is
        // the Adwaita near-white (#fafafa). If the palette doesn't flip
        // users get a dark-on-light or light-on-dark context menu.
        assert!(dark.contains("#242424"), "dark palette missing surface hex");
        assert!(light.contains("#fafafa"), "light palette missing surface hex");
        assert_ne!(dark, light, "scheme flip must actually change the output");
    }
}
