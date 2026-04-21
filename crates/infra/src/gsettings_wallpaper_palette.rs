// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! GSettings-backed [`WallpaperPaletteProvider`] adapter.
//!
//! Reads `io.github.gnomex.GnomeX cached-wallpaper-palette`, which
//! the `experienced` daemon populates on every wallpaper change.
//! Each entry is `"HEX:ACCENT_ID"`; we only care about the hex.

use gio::prelude::*;
use gnomex_app::ports::WallpaperPaletteProvider;
use gnomex_domain::HexColor;

const SCHEMA_ID: &str = "io.github.gnomex.GnomeX";
const KEY: &str = "cached-wallpaper-palette";

pub struct GSettingsWallpaperPaletteProvider;

impl GSettingsWallpaperPaletteProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GSettingsWallpaperPaletteProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl WallpaperPaletteProvider for GSettingsWallpaperPaletteProvider {
    fn top3(&self) -> Option<[HexColor; 3]> {
        // Guard the read: GSettings `new` on an unknown schema
        // aborts the process, so confirm the schema is installed
        // before touching it. In a fresh checkout without running
        // `install.sh`, the daemon may not have populated the key
        // either, which we treat as "palette not ready".
        let installed = gio::SettingsSchemaSource::default()
            .and_then(|src| src.lookup(SCHEMA_ID, true))
            .is_some();
        if !installed {
            return None;
        }
        let settings = gio::Settings::new(SCHEMA_ID);
        let entries = settings.strv(KEY);
        let mut out: Vec<HexColor> = Vec::with_capacity(3);
        for entry in entries.iter() {
            let s = entry.as_str();
            // Format is `"#rrggbb:accent_id"`; drop the suffix.
            let hex = s.split(':').next().unwrap_or(s);
            if let Ok(h) = HexColor::new(hex) {
                out.push(h);
                if out.len() == 3 {
                    break;
                }
            }
        }
        if out.len() < 3 {
            return None;
        }
        let [a, b, c] = [out[0].clone(), out[1].clone(), out[2].clone()];
        Some([a, b, c])
    }
}
