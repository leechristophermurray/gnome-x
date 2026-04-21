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
    gen_mock: Arc<dyn ThemeCssGenerator>,
    writer: Arc<dyn ThemeWriter>,
    appearance: Arc<dyn AppearanceSettings>,
    external_themers: Vec<Arc<dyn ExternalAppThemer>>,
}

impl ApplyThemeUseCase {
    pub fn new(
        gen_mock: Arc<dyn ThemeCssGenerator>,
        writer: Arc<dyn ThemeWriter>,
        appearance: Arc<dyn AppearanceSettings>,
    ) -> Self {
        Self {
            gen_mock,
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
        let css = self.gen_mock.generate(spec)?;
        self.writer.write_gtk_css(&css.gtk_css, &css.gtk3_css)?;
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

    /// The GNOME version this gen_mock targets.
    pub fn detected_version(&self) -> &str {
        self.gen_mock.version_label()
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
#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{
        sample_theme_spec, MockAppearance, MockCssGenerator, MockExternalThemer,
        MockThemeWriter,
    };

    fn make() -> (
        ApplyThemeUseCase,
        std::sync::Arc<MockCssGenerator>,
        std::sync::Arc<MockThemeWriter>,
        std::sync::Arc<MockAppearance>,
    ) {
        let gen_mock = MockCssGenerator::new();
        let writer = MockThemeWriter::new();
        let appearance = MockAppearance::new();
        let uc = ApplyThemeUseCase::new(gen_mock.clone(), writer.clone(), appearance.clone());
        (uc, gen_mock, writer, appearance)
    }

    #[test]
    fn apply_routes_through_generator_writer_and_appearance() {
        let (uc, gen_mock, writer, appearance) = make();
        uc.apply(&sample_theme_spec()).unwrap();

        // Generator saw the spec.
        assert!(gen_mock.last_spec.lock().unwrap().is_some());
        // Writer got both CSS payloads with the custom theme name.
        let calls = writer.calls.lock().unwrap().clone();
        assert_eq!(calls, vec!["write_gtk_css", "write_shell_css(GNOME-X-Custom)"]);
        // Shell theme was pointed at our custom theme.
        assert_eq!(
            appearance.shell_theme.lock().unwrap().as_str(),
            "GNOME-X-Custom"
        );
    }

    #[test]
    fn reset_clears_overrides_and_unsets_shell_theme() {
        let (uc, _gen_mock, writer, appearance) = make();
        *appearance.shell_theme.lock().unwrap() = "GNOME-X-Custom".into();

        uc.reset().unwrap();

        assert!(writer.calls.lock().unwrap().contains(&"clear_overrides".into()));
        assert_eq!(appearance.shell_theme.lock().unwrap().as_str(), "");
    }

    #[test]
    fn apply_fans_out_to_every_registered_external_themer() {
        let gen_mock = MockCssGenerator::new();
        let writer = MockThemeWriter::new();
        let appearance = MockAppearance::new();
        let vscode = MockExternalThemer::new("vscode");
        let chromium = MockExternalThemer::new("chromium");

        let uc = ApplyThemeUseCase::new(gen_mock, writer, appearance)
            .with_external_themer(vscode.clone())
            .with_external_themer(chromium.clone());

        uc.apply(&sample_theme_spec()).unwrap();

        assert_eq!(vscode.applies.lock().unwrap().len(), 1);
        assert_eq!(chromium.applies.lock().unwrap().len(), 1);
        // And both see the same spec.
        let v = &vscode.applies.lock().unwrap()[0];
        let c = &chromium.applies.lock().unwrap()[0];
        assert_eq!(v.accent, c.accent);
        assert_eq!(v.panel_tint, c.panel_tint);
    }

    #[test]
    fn apply_external_bypasses_the_css_generator() {
        let (uc, gen_mock, writer, _appearance) = make();
        let spec = ExternalThemeSpec {
            accent: gnomex_domain::HexColor::new("#ff0000").unwrap(),
            panel_tint: gnomex_domain::HexColor::new("#1a1a1e").unwrap(),
            color_scheme: gnomex_domain::ColorScheme::Dark,
        };
        uc.apply_external(&spec);

        // No CSS generation, no disk writes — only the fan-out.
        assert!(gen_mock.last_spec.lock().unwrap().is_none());
        assert!(writer.calls.lock().unwrap().is_empty());
    }

    #[test]
    fn reset_also_resets_every_external_themer() {
        let gen_mock = MockCssGenerator::new();
        let writer = MockThemeWriter::new();
        let appearance = MockAppearance::new();
        let vscode = MockExternalThemer::new("vscode");
        let chromium = MockExternalThemer::new("chromium");

        let uc = ApplyThemeUseCase::new(gen_mock, writer, appearance)
            .with_external_themer(vscode.clone())
            .with_external_themer(chromium.clone());

        uc.reset().unwrap();

        assert_eq!(*vscode.resets.lock().unwrap(), 1);
        assert_eq!(*chromium.resets.lock().unwrap(), 1);
    }

    #[test]
    fn external_spec_derives_color_scheme_from_panel_luminance() {
        let mut spec = sample_theme_spec();
        // Default panel.tint is #1a1a1e — dark.
        let ext = external_spec_from(&spec);
        assert!(ext.color_scheme.is_dark());

        // Swap to a bright panel tint — should flip to light.
        spec.panel.tint = gnomex_domain::HexColor::new("#f5f5f5").unwrap();
        let ext = external_spec_from(&spec);
        assert!(!ext.color_scheme.is_dark());
    }
}

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
