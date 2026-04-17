// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Dash to Dock-backed implementation of [`FloatingDockController`].
//!
//! When the user toggles *Floating Dock* in the Theme Builder we write
//! a curated preset to `org.gnome.shell.extensions.dash-to-dock`. When
//! they toggle it back off we `reset()` each key so the user's (and
//! the extension's) defaults come back — we never store a custom
//! "previous value" because the settings belong to Dash to Dock, not
//! to us.

use gio::prelude::*;
use gnomex_app::ports::FloatingDockController;
use gnomex_app::AppError;

const SCHEMA: &str = "org.gnome.shell.extensions.dash-to-dock";

/// Keys we write to produce the floating look. Listed once so `apply(false)`
/// can reset exactly the set we touched.
const OWNED_KEYS: &[&str] = &[
    "extend-height",
    "dock-fixed",
    "intellihide",
    "transparency-mode",
    "background-opacity",
    "custom-theme-shrink",
    "dash-max-icon-size",
];

pub struct GSettingsFloatingDock;

impl GSettingsFloatingDock {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GSettingsFloatingDock {
    fn default() -> Self {
        Self::new()
    }
}

impl FloatingDockController for GSettingsFloatingDock {
    fn is_available(&self) -> bool {
        gio::SettingsSchemaSource::default()
            .and_then(|src| src.lookup(SCHEMA, true))
            .is_some()
    }

    fn apply(&self, enabled: bool) -> Result<(), AppError> {
        if !self.is_available() {
            return Err(AppError::Settings(
                "Dash to Dock extension not installed".into(),
            ));
        }
        let s = gio::Settings::new(SCHEMA);

        if enabled {
            // Floating preset: detach from the edge, intellihide,
            // moderately transparent, slightly compact icon size.
            let _ = s.set_boolean("extend-height", false);
            let _ = s.set_boolean("dock-fixed", false);
            let _ = s.set_boolean("intellihide", true);
            let _ = s.set_string("transparency-mode", "FIXED");
            let _ = s.set_double("background-opacity", 0.7);
            let _ = s.set_boolean("custom-theme-shrink", true);
            let _ = s.set_int("dash-max-icon-size", 48);
            tracing::info!("floating dock: preset applied");
        } else {
            for key in OWNED_KEYS {
                s.reset(key);
            }
            tracing::info!("floating dock: defaults restored");
        }
        Ok(())
    }
}
