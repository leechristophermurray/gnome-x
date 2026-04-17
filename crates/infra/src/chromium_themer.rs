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

use gnomex_app::ports::ExternalAppThemer;
use gnomex_app::AppError;
use gnomex_domain::ExternalThemeSpec;
use serde_json::{json, Value};
use std::path::PathBuf;

const THEME_SUBDIR: &str = ".local/share/gnome-x/chromium-theme";
const MANIFEST_NAME: &str = "manifest.json";

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

        tracing::info!(
            "chromium: wrote theme to {} (detected: {})",
            dir.display(),
            flavors.join(", ")
        );
        tracing::info!(
            "chromium: load once via chrome://extensions → Developer mode → Load unpacked → {}",
            dir.display()
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

#[cfg(test)]
mod tests {
    use super::*;
    use gnomex_domain::{ColorScheme, HexColor};

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
}
