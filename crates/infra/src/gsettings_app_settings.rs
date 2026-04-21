// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! GSettings-backed implementation of [`AppSettings`] for the
//! `io.github.gnomex.GnomeX` schema.
//!
//! We deliberately allowlist the pack-relevant keys rather than dumping
//! every key in the schema: window geometry and other session state
//! doesn't belong in a portable Experience Pack.

use gio::glib::Variant;
use gio::prelude::*;
use gnomex_app::ports::AppSettings;
use gnomex_app::AppError;
use gnomex_domain::GSettingOverride;

const SCHEMA: &str = "io.github.gnomex.GnomeX";

/// Keys that round-trip through a pack. Order here is also the order
/// they're written to the TOML, giving packs a stable diff.
const PACK_KEYS: &[&str] = &[
    // Theme Builder — dimensions
    "tb-window-radius",
    "tb-element-radius",
    "tb-panel-radius",
    "tb-headerbar-height",
    "tb-inset-border",
    "tb-card-border-width",
    "tb-focus-ring-width",
    "tb-notification-radius",
    // Theme Builder — opacities / intensities
    "tb-panel-opacity",
    "tb-dash-opacity",
    "tb-tint-intensity",
    "tb-headerbar-shadow",
    "tb-separator-opacity",
    "tb-notification-opacity",
    // Theme Builder — colors
    "tb-panel-tint",
    // Theme Builder — booleans
    "tb-overview-blur",
    "tb-circular-buttons",
    "tb-show-window-shadow",
    "tb-combo-inset",
    "tb-floating-dock-enabled",
    // Theme Builder — scaling (GXF-050/051/053)
    "tb-scaling-text-factor",
    "tb-scaling-monitor-framebuffer",
    "tb-scaling-x11-fractional",
    "tb-scaling-per-app-overrides",
    // Accent scheduling
    "scheduled-accent-enabled",
    "day-accent-color",
    "night-accent-color",
    "use-sunrise-sunset",
    "scheduled-panel-enabled",
    "day-panel-tint",
    "night-panel-tint",
    // Cross-cutting toggles
    "shared-tinting-enabled",
    "use-wallpaper-accent",
    "disable-version-validation",
];

pub struct GSettingsAppSettings;

impl GSettingsAppSettings {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GSettingsAppSettings {
    fn default() -> Self {
        Self::new()
    }
}

impl AppSettings for GSettingsAppSettings {
    fn snapshot_pack_settings(&self) -> Result<Vec<GSettingOverride>, AppError> {
        let settings = gio::Settings::new(SCHEMA);
        let schema = settings
            .settings_schema()
            .ok_or_else(|| AppError::Settings(format!("schema {SCHEMA} not found")))?;

        let mut out = Vec::with_capacity(PACK_KEYS.len());
        for key in PACK_KEYS {
            if !schema.has_key(key) {
                // Schema upgrade may have removed a key — skip silently.
                continue;
            }
            let variant = settings.value(key);
            out.push(GSettingOverride {
                key: (*key).to_owned(),
                value: variant.print(true).to_string(),
            });
        }
        Ok(out)
    }

    fn apply_overrides(&self, overrides: &[GSettingOverride]) -> Result<(), AppError> {
        let settings = gio::Settings::new(SCHEMA);
        let schema = match settings.settings_schema() {
            Some(s) => s,
            None => return Err(AppError::Settings(format!("schema {SCHEMA} not found"))),
        };

        for o in overrides {
            if !schema.has_key(&o.key) {
                tracing::warn!("pack apply: unknown key '{}' — skipped", o.key);
                continue;
            }
            let key = schema.key(&o.key);
            let variant_type = key.value_type();
            match Variant::parse(Some(&variant_type), &o.value) {
                Ok(v) => {
                    if let Err(e) = settings.set_value(&o.key, &v) {
                        tracing::warn!("pack apply: set '{}' failed: {e}", o.key);
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "pack apply: can't parse value for '{}' ({}): {e}",
                        o.key,
                        o.value
                    );
                }
            }
        }
        Ok(())
    }
}
