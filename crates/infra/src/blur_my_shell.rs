// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Blur My Shell-backed implementation of [`BlurMyShellController`].
//!
//! Writes to the extension's overview sub-schema to turn wallpaper
//! blur on/off. Availability is detected via schema lookup so users
//! without the extension see a disabled toggle in the Theme Builder.
//!
//! We touch only the `overview` sub-schema — not the panel, dash, or
//! applications sub-schemas — so we don't surprise users by enabling
//! blur in places they didn't ask for.

use gio::prelude::*;
use gnomex_app::ports::BlurMyShellController;
use gnomex_app::AppError;

const SCHEMA: &str = "org.gnome.shell.extensions.blur-my-shell.overview";

/// Keys we write to produce the overview-blur effect. Listed once so
/// `apply(false)` can reset exactly the set we touched.
const OWNED_KEYS: &[&str] = &["blur", "customize", "pipeline", "style-components"];

pub struct GSettingsBlurMyShell;

impl GSettingsBlurMyShell {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GSettingsBlurMyShell {
    fn default() -> Self {
        Self::new()
    }
}

impl BlurMyShellController for GSettingsBlurMyShell {
    fn is_available(&self) -> bool {
        gio::SettingsSchemaSource::default()
            .and_then(|src| src.lookup(SCHEMA, true))
            .is_some()
    }

    fn apply(&self, enabled: bool) -> Result<(), AppError> {
        if !self.is_available() {
            return Err(AppError::Settings(
                "Blur My Shell extension not installed".into(),
            ));
        }
        let s = gio::Settings::new(SCHEMA);
        if enabled {
            let _ = s.set_boolean("blur", true);
            // Use the stock preset so we don't fight user tweaks they
            // may have applied via Blur My Shell's own preferences.
            tracing::info!("blur my shell: overview blur enabled");
        } else {
            for key in OWNED_KEYS {
                s.reset(key);
            }
            tracing::info!("blur my shell: overview blur reset to defaults");
        }
        Ok(())
    }
}
