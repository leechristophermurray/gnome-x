// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::ports::{
    AppLauncherOverrides, AppearanceSettings, ExternalAppThemer, GdmThemer, IconThemeRecolorer,
    MutterSettings, RecolorOutcome, ThemeCssGenerator, ThemeWriter, WallpaperPaletteProvider,
};
use crate::AppError;
use gnomex_domain::{derive_md3_overrides, ExternalThemeSpec, ScalingSpec, ThemeSpec};
use std::sync::Arc;

/// Name of the custom shell theme directory GNOME X authors under
/// `~/.local/share/themes/`. Exposed so callers wiring the GDM
/// themer can forward the same name across the polkit boundary.
pub const CUSTOM_THEME_NAME: &str = "GNOME-X-Custom";

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
    /// Optional Mutter/text-scale adapter. Absent when the use case is
    /// exercised from a headless unit test with mocks only; present in
    /// production. When absent, `apply_scaling` is a no-op.
    mutter: Option<Arc<dyn MutterSettings>>,
    /// Optional per-app `.desktop` override adapter. Same treatment as
    /// `mutter` above.
    app_launcher: Option<Arc<dyn AppLauncherOverrides>>,
    /// Optional GDM (login-screen) themer. Requires polkit elevation;
    /// off by default. When present AND explicitly enabled per-call,
    /// the use case forwards the accent + theme-name to the GDM
    /// dconf database. See GXF-003.
    gdm: Option<Arc<dyn GdmThemer>>,
    /// Optional icon-theme recolouring adapter. Off by default so
    /// headless tests and users who prefer a hands-off icon theme
    /// don't see spurious shell-outs. When present AND the caller
    /// passes the accent id via `recolor_icons`, the adapter
    /// decides whether to shell out (Papirus) or no-op (Adwaita).
    icon_recolorer: Option<Arc<dyn IconThemeRecolorer>>,
    /// Optional wallpaper-palette provider. Required for the
    /// Material-palette branch (GXF material-theming); absent for
    /// headless tests and for any apply where the user hasn't
    /// enabled `spec.material_palette.enabled`.
    palette: Option<Arc<dyn WallpaperPaletteProvider>>,
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
            mutter: None,
            app_launcher: None,
            gdm: None,
            icon_recolorer: None,
            palette: None,
        }
    }

    /// Register an external app themer. Multiple themers may be registered
    /// (one per app family). Order is preserved for logging purposes.
    pub fn with_external_themer(mut self, themer: Arc<dyn ExternalAppThemer>) -> Self {
        self.external_themers.push(themer);
        self
    }

    /// Register the Mutter/text-scaling adapter. Without it,
    /// [`apply_scaling`] is a no-op so the `ApplyThemeUseCase` can
    /// still run in tests that don't exercise scaling.
    pub fn with_mutter_settings(mut self, mutter: Arc<dyn MutterSettings>) -> Self {
        self.mutter = Some(mutter);
        self
    }

    /// Register the per-app `.desktop` override adapter. Without it,
    /// [`apply_scaling`] skips per-app overrides (logs a debug line).
    pub fn with_app_launcher_overrides(
        mut self,
        launcher: Arc<dyn AppLauncherOverrides>,
    ) -> Self {
        self.app_launcher = Some(launcher);
        self
    }

    /// Register the GDM (login-screen) themer. Without it,
    /// [`apply_to_gdm`] is a no-op and the `also_gdm` caller flag is
    /// silently ignored — the themer is optional infrastructure so
    /// headless tests that don't care about GDM keep compiling.
    pub fn with_gdm_themer(mut self, gdm: Arc<dyn GdmThemer>) -> Self {
        self.gdm = Some(gdm);
        self
    }

    /// Register the icon-theme recolouring adapter. Without it,
    /// [`recolor_icons`] is a no-op.
    pub fn with_icon_recolorer(mut self, r: Arc<dyn IconThemeRecolorer>) -> Self {
        self.icon_recolorer = Some(r);
        self
    }

    /// Recolour the folder / accent icons of the currently-active
    /// icon theme to match `accent_id`. Safe to call when the
    /// recolorer port is absent — returns `Ok(None)`.
    ///
    /// The optional caller-provided flag `enabled` short-circuits
    /// the whole path when the user has turned the toggle off, so
    /// callers can pass the flag through without branching.
    pub fn recolor_icons(
        &self,
        accent_id: &str,
        enabled: bool,
    ) -> Result<Option<RecolorOutcome>, AppError> {
        if !enabled {
            return Ok(None);
        }
        let Some(r) = &self.icon_recolorer else {
            return Ok(None);
        };
        let outcome = r.recolor(accent_id)?;
        Ok(Some(outcome))
    }

    /// Register the wallpaper palette provider used by Material-
    /// palette theming. Without it, `spec.material_palette.enabled`
    /// is honoured only as "do nothing" — we never mint overrides
    /// from a palette we can't read. Headless unit tests skip this.
    pub fn with_palette_provider(mut self, p: Arc<dyn WallpaperPaletteProvider>) -> Self {
        self.palette = Some(p);
        self
    }

    /// Generate version-specific CSS and write it to disk. Then fan out to
    /// every registered external-app themer with a projection of `spec`.
    pub fn apply(&self, spec: &ThemeSpec) -> Result<(), AppError> {
        // Material-palette branch: when the user has opted in AND
        // we have both a palette provider and ≥3 cached wallpaper
        // colours, derive a fresh `widget_colors` override block
        // and feed it back through the normal CSS pipeline. All
        // other theme knobs pass through untouched.
        //
        // If either prerequisite is missing (provider unwired,
        // palette cache empty on first run), we quietly fall
        // through to the non-MD path so the user's theme still
        // applies — the UI's toggle hint is the place to surface
        // "palette not ready yet".
        let effective = if spec.material_palette.enabled {
            if let Some(derived) = self.derive_material_spec(spec) {
                derived
            } else {
                spec.clone()
            }
        } else {
            spec.clone()
        };

        let css = self.gen_mock.generate(&effective)?;
        self.writer.write_gtk_css(&css.gtk_css, &css.gtk3_css)?;
        self.writer.write_shell_css(&css.shell_css, CUSTOM_THEME_NAME)?;
        self.appearance.set_shell_theme(CUSTOM_THEME_NAME).ok();

        // Scaling adjacent to theming: a `ThemeSpec` carrying a
        // non-default `ScalingSpec` should produce the expected
        // GSettings + `.desktop` writes as part of the same Apply.
        // Failures here are logged but don't fail the whole apply —
        // the user's CSS theme is already on disk.
        if let Err(e) = self.apply_scaling(&effective.scaling) {
            tracing::warn!("apply_scaling failed: {e}");
        }

        let external_spec = external_spec_from(&effective);
        self.fan_out_apply(&external_spec);
        Ok(())
    }

    /// Clone `spec` and overwrite its `widget_colors` from the
    /// Material derivation. Returns `None` if the palette provider
    /// isn't wired or the wallpaper cache has fewer than three
    /// colours yet — caller falls back to the user-supplied overrides.
    ///
    /// Side-effect: also writes the nearest GNOME enum accent id to
    /// `org.gnome.desktop.interface accent-color` so Adwaita folder
    /// icons track the MD3 primary, and kicks the icon recolourer
    /// (Papirus / etc.) with that accent. Without this, MD3 changes
    /// the widget CSS but the accent-driven icon tokens (folder
    /// colour, sidebar icons, symbolic tinting) stay on whatever
    /// the user last set manually.
    fn derive_material_spec(&self, spec: &ThemeSpec) -> Option<ThemeSpec> {
        let provider = self.palette.as_ref()?;
        let palette = provider.top3()?;
        let day = spec.material_palette.day_permutation.apply(&palette);
        let night = spec.material_palette.night_permutation.apply(&palette);
        let overrides = derive_md3_overrides(&day, &night, spec.tint.intensity);
        let mut copy = spec.clone();
        copy.widget_colors = overrides;

        // Propagate the MD3 primary to the GNOME accent enum so
        // icon themes (Adwaita natively, Papirus via recolourer)
        // follow. The *day* primary wins the enum write — Adwaita's
        // dark variant of any accent id is a rehue of the same
        // base so we don't need a day/night split here.
        let (r, g, b) = day.primary.to_rgb();
        let accent_id = gnomex_domain::color::closest_gnome_accent_id(r, g, b);
        if let Err(e) = self.appearance.set_accent_color(&accent_id) {
            tracing::warn!("MD3: failed to write accent-color={accent_id}: {e}");
        }
        // Kick the icon recolorer with the same accent. Safe when
        // unwired; caller's `recolor_icons` toggle is bypassed here
        // because MD3 mode is an implicit opt-in to the whole
        // appearance.
        if let Err(e) = self.recolor_icons(&accent_id, true) {
            tracing::warn!("MD3: icon recolour failed: {e}");
        }

        tracing::info!(
            "material-palette applied (day={:?}, night={:?}, accent={accent_id})",
            spec.material_palette.day_permutation,
            spec.material_palette.night_permutation,
        );
        Some(copy)
    }

    /// Apply just the scaling portion of a theme spec. Safe to call
    /// when `mutter` / `app_launcher` ports are absent — it becomes a
    /// no-op rather than an error, so headless unit tests can
    /// continue to run.
    ///
    /// Semantics:
    /// - `text_scaling` is written only if non-default (1.0).
    /// - `experimental-features` is written only if the reconciled
    ///   list differs from what's currently set.
    /// - Each `per_app_overrides` entry is registered (writes a
    ///   `.desktop` file). Per-app failures are logged and skipped
    ///   rather than aborting the batch.
    pub fn apply_scaling(&self, scaling: &ScalingSpec) -> Result<(), AppError> {
        if let Some(m) = &self.mutter {
            if !scaling.text_scaling.is_default() {
                m.set_text_scaling_factor(scaling.text_scaling.as_f64())?;
            }
            let current = m.experimental_features()?;
            if let Some(new) = crate::use_cases::apply_theme::reconcile_flags(
                &current,
                scaling.scale_monitor_framebuffer,
                scaling.x11_randr_fractional_scaling,
            ) {
                m.set_experimental_features(&new)?;
            }
        }
        if let Some(launcher) = &self.app_launcher {
            for o in &scaling.per_app_overrides {
                if let Err(e) = launcher.register_override(o) {
                    tracing::warn!(
                        "per-app scaling override for '{}' failed: {e}",
                        o.app_id
                    );
                }
            }
        }
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

    /// Propagate the current theme name + accent to GDM via the
    /// registered `GdmThemer` (polkit-elevated). Returns
    /// `AppError::Settings` if no themer is wired (caller opted in
    /// without providing one) so the UI can surface a clear error
    /// instead of silently doing nothing.
    ///
    /// Only the theme NAME + accent are forwarded across the polkit
    /// boundary. The elevated helper re-validates both.
    pub fn apply_to_gdm(
        &self,
        theme_name: &str,
        accent: &gnomex_domain::HexColor,
    ) -> Result<(), AppError> {
        let gdm = self
            .gdm
            .as_ref()
            .ok_or_else(|| AppError::Settings("gdm themer not wired".into()))?;
        gdm.apply(theme_name, accent)
    }

    /// Remove GNOME X's GDM overrides via the registered `GdmThemer`.
    /// Mirrors [`apply_to_gdm`]'s "no themer = error" contract.
    pub fn reset_gdm(&self) -> Result<(), AppError> {
        let gdm = self
            .gdm
            .as_ref()
            .ok_or_else(|| AppError::Settings("gdm themer not wired".into()))?;
        gdm.reset()
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
        sample_theme_spec, MockAppearance, MockCssGenerator, MockExternalThemer, MockGdmThemer,
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

    #[test]
    fn apply_to_gdm_errors_when_no_themer_wired() {
        let (uc, _, _, _) = make();
        let accent = gnomex_domain::HexColor::new("#3584e4").unwrap();
        let err = uc.apply_to_gdm(CUSTOM_THEME_NAME, &accent).unwrap_err();
        match err {
            AppError::Settings(s) => assert!(s.contains("gdm themer not wired")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn apply_to_gdm_forwards_to_themer() {
        let (uc, _, _, _) = make();
        let gdm = MockGdmThemer::new();
        let uc = uc.with_gdm_themer(gdm.clone());

        let accent = gnomex_domain::HexColor::new("#3584e4").unwrap();
        uc.apply_to_gdm(CUSTOM_THEME_NAME, &accent).unwrap();

        let calls = gdm.applies.lock().unwrap().clone();
        assert_eq!(calls, vec![(CUSTOM_THEME_NAME.into(), "#3584e4".into())]);
    }

    #[test]
    fn reset_gdm_forwards_to_themer() {
        let (uc, _, _, _) = make();
        let gdm = MockGdmThemer::new();
        let uc = uc.with_gdm_themer(gdm.clone());

        uc.reset_gdm().unwrap();

        assert_eq!(*gdm.resets.lock().unwrap(), 1);
    }

    #[test]
    fn apply_without_gdm_toggle_does_not_touch_gdm_themer() {
        // Regression: the base `apply(&spec)` path must never call
        // the GDM themer on its own. GDM is opt-in per-call and
        // goes through `apply_to_gdm`.
        let (uc, _, _, _) = make();
        let gdm = MockGdmThemer::new();
        let uc = uc.with_gdm_themer(gdm.clone());

        uc.apply(&sample_theme_spec()).unwrap();

        assert!(gdm.applies.lock().unwrap().is_empty());
        assert_eq!(*gdm.resets.lock().unwrap(), 0);
    }
}

/// Pure reconcile: produce the new `experimental-features` strv for
/// Mutter, flipping our two managed flags while preserving any other
/// flag the user set. Returns `None` when the result is identical to
/// `current` so callers can skip a no-op write.
///
/// Lives in the app layer (not infra) because it's pure logic — no
/// GSettings dependency — which keeps it trivially unit-testable.
pub fn reconcile_flags(
    current: &[String],
    want_monitor_framebuffer: bool,
    want_x11_fractional: bool,
) -> Option<Vec<String>> {
    const FB: &str = "scale-monitor-framebuffer";
    const X11: &str = "x11-randr-fractional-scaling";

    let mut out: Vec<String> = Vec::with_capacity(current.len() + 2);
    let mut have_fb = false;
    let mut have_x11 = false;

    for flag in current {
        match flag.as_str() {
            FB => {
                if want_monitor_framebuffer && !have_fb {
                    out.push(FB.to_owned());
                    have_fb = true;
                }
            }
            X11 => {
                if want_x11_fractional && !have_x11 {
                    out.push(X11.to_owned());
                    have_x11 = true;
                }
            }
            _ => out.push(flag.clone()),
        }
    }
    if want_monitor_framebuffer && !have_fb {
        out.push(FB.to_owned());
    }
    if want_x11_fractional && !have_x11 {
        out.push(X11.to_owned());
    }
    if out == current { None } else { Some(out) }
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
