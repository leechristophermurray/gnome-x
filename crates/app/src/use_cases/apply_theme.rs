// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::ports::{AppearanceSettings, ExternalAppThemer, ThemeCssGenerator, ThemeWriter};
use crate::AppError;
use gnomex_domain::{ExternalThemeSpec, ThemeSpec};
use std::sync::Arc;

const CUSTOM_THEME_NAME: &str = "GNOME-X-Custom";

/// Use case: apply or reset a ThemeSpec via version-appropriate CSS generation.
///
/// External app themers (Chromium, VS Code, ...) are registered alongside
/// the core GNOME writers. On `apply()` they receive a projection of the
/// full ThemeSpec; failures in individual themers are logged but do not
/// abort the operation — third-party app integration is best-effort.
pub struct ApplyThemeUseCase {
    generator: Arc<dyn ThemeCssGenerator>,
    writer: Arc<dyn ThemeWriter>,
    appearance: Arc<dyn AppearanceSettings>,
    external_themers: Vec<Arc<dyn ExternalAppThemer>>,
}

impl ApplyThemeUseCase {
    pub fn new(
        generator: Arc<dyn ThemeCssGenerator>,
        writer: Arc<dyn ThemeWriter>,
        appearance: Arc<dyn AppearanceSettings>,
    ) -> Self {
        Self {
            generator,
            writer,
            appearance,
            external_themers: Vec::new(),
        }
    }

    /// Register an external app themer. Multiple themers may be registered
    /// (one per app family). Order is preserved for logging purposes.
    pub fn with_external_themer(mut self, themer: Arc<dyn ExternalAppThemer>) -> Self {
        self.external_themers.push(themer);
        self
    }

    /// Generate version-specific CSS and write it to disk. Then fan out to
    /// every registered external-app themer with a projection of `spec`.
    pub fn apply(&self, spec: &ThemeSpec) -> Result<(), AppError> {
        let css = self.generator.generate(spec)?;
        self.writer.write_gtk_css(&css.gtk_css)?;
        self.writer.write_shell_css(&css.shell_css, CUSTOM_THEME_NAME)?;
        self.appearance.set_shell_theme(CUSTOM_THEME_NAME).ok();

        let external_spec = external_spec_from(spec);
        self.fan_out_apply(&external_spec);
        Ok(())
    }

    /// Fan out only the external-app side without touching GNOME CSS.
    /// Used by the daemon when the accent changes on a schedule and we
    /// want other apps to track it without regenerating the shell theme.
    pub fn apply_external(&self, spec: &ExternalThemeSpec) {
        self.fan_out_apply(spec);
    }

    /// Remove all custom CSS and reset shell theme. Also clears every
    /// external-app themer's region.
    pub fn reset(&self) -> Result<(), AppError> {
        self.writer.clear_overrides()?;
        self.appearance.set_shell_theme("").ok();
        for t in &self.external_themers {
            if let Err(e) = t.reset() {
                tracing::warn!("external themer '{}' reset failed: {e}", t.name());
            }
        }
        Ok(())
    }

    /// The GNOME version this generator targets.
    pub fn detected_version(&self) -> &str {
        self.generator.version_label()
    }

    fn fan_out_apply(&self, spec: &ExternalThemeSpec) {
        for t in &self.external_themers {
            if let Err(e) = t.apply(spec) {
                tracing::warn!("external themer '{}' apply failed: {e}", t.name());
            }
        }
    }
}

/// Project the rich GNOME-side `ThemeSpec` down to the narrow
/// `ExternalThemeSpec` third-party apps consume. Uses the accent tint
/// baked into `spec.tint` and the panel tint baked into `spec.panel`.
/// Color scheme is inferred from the panel tint luminance; callers that
/// already know the current `color-scheme` should use `apply_external`
/// with a precise spec instead.
fn external_spec_from(spec: &ThemeSpec) -> ExternalThemeSpec {
    let panel_tint = spec.panel.tint.clone();
    let (r, g, b) = panel_tint.to_rgb();
    let luma = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    let color_scheme = if luma < 128.0 {
        gnomex_domain::ColorScheme::Dark
    } else {
        gnomex_domain::ColorScheme::Light
    };
    ExternalThemeSpec {
        accent: spec.tint.accent_hex.clone(),
        panel_tint,
        color_scheme,
    }
}
