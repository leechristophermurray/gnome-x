// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! GSettings-backed implementation of [`MutterSettings`] (GXF-050, GXF-053).
//!
//! Two schemas in play:
//! - `org.gnome.mutter` — the `experimental-features` strv where
//!   `scale-monitor-framebuffer` and `x11-randr-fractional-scaling` live.
//! - `org.gnome.desktop.interface` — where `text-scaling-factor` lives.
//!
//! The strv write API deliberately replaces rather than merges so
//! callers can remove flags; the *apply* use-case higher up the stack
//! is responsible for reading the current value, flipping our managed
//! flags, and writing the result — preserving any flag the user set
//! via GNOME Tweaks (`variable-refresh-rate`, etc.).

use gio::prelude::*;
use gnomex_app::ports::MutterSettings;
use gnomex_app::AppError;

const MUTTER_SCHEMA: &str = "org.gnome.mutter";
const IFACE_SCHEMA: &str = "org.gnome.desktop.interface";
const EXPERIMENTAL_KEY: &str = "experimental-features";
const TEXT_SCALING_KEY: &str = "text-scaling-factor";

pub struct GSettingsMutter {
    has_mutter_schema: bool,
}

impl GSettingsMutter {
    pub fn new() -> Self {
        let has_mutter_schema = gio::SettingsSchemaSource::default()
            .and_then(|src| src.lookup(MUTTER_SCHEMA, true))
            .is_some();
        Self { has_mutter_schema }
    }
}

impl Default for GSettingsMutter {
    fn default() -> Self {
        Self::new()
    }
}

impl MutterSettings for GSettingsMutter {
    fn experimental_features(&self) -> Result<Vec<String>, AppError> {
        if !self.has_mutter_schema {
            // On non-GNOME systems this returns the empty list rather
            // than an error so the UI can still run in e.g. a build
            // sandbox.
            return Ok(Vec::new());
        }
        let settings = gio::Settings::new(MUTTER_SCHEMA);
        Ok(settings
            .strv(EXPERIMENTAL_KEY)
            .iter()
            .map(|s| s.as_str().to_owned())
            .collect())
    }

    fn set_experimental_features(&self, features: &[String]) -> Result<(), AppError> {
        if !self.has_mutter_schema {
            return Err(AppError::Settings(format!(
                "schema {MUTTER_SCHEMA} not installed; cannot write {EXPERIMENTAL_KEY}"
            )));
        }
        let settings = gio::Settings::new(MUTTER_SCHEMA);
        let as_strs: Vec<&str> = features.iter().map(|s| s.as_str()).collect();
        settings
            .set_strv(EXPERIMENTAL_KEY, as_strs)
            .map_err(|e| AppError::Settings(format!("set {EXPERIMENTAL_KEY}: {e}")))?;
        Ok(())
    }

    fn text_scaling_factor(&self) -> Result<f64, AppError> {
        let settings = gio::Settings::new(IFACE_SCHEMA);
        Ok(settings.double(TEXT_SCALING_KEY))
    }

    fn set_text_scaling_factor(&self, factor: f64) -> Result<(), AppError> {
        let settings = gio::Settings::new(IFACE_SCHEMA);
        settings
            .set_double(TEXT_SCALING_KEY, factor)
            .map_err(|e| AppError::Settings(format!("set {TEXT_SCALING_KEY}: {e}")))?;
        Ok(())
    }
}

// The pure reconcile function lives in `gnomex_app::use_cases::apply_theme`
// so both CLI and UI share the same implementation; see
// `reconcile_flags` there.
