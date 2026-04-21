// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Scaling adapter integration tests (GXF-050, GXF-051, GXF-053).
//!
//! Pins three invariants:
//!
//! 1. **Default `ScalingSpec` is byte-silent.** A fresh `ThemeSpec`
//!    must not rewrite any scaling file, flag, or GSetting. This is
//!    the equivalent of the "opt-in, not opt-out" rule we apply to
//!    every new spec field.
//! 2. **Text scaling round-trips via `apply_scaling`.** Setting
//!    `text_scaling` triggers `MutterSettings::set_text_scaling_factor`
//!    exactly once with the right value.
//! 3. **Mutter experimental-features are merged non-destructively.**
//!    When `scale_monitor_framebuffer` is requested, it's added to
//!    the current strv without clobbering unrelated flags the user
//!    may have set via Tweaks (e.g. `variable-refresh-rate`).
//!
//! The per-app launcher adapter is exercised end-to-end against a
//! `tempfile::tempdir()`-rooted fake HOME so the test doesn't touch
//! the user's real `~/.local/share/applications/`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gnomex_app::ports::{
    AppLauncherOverrides, AppearanceSettings, MutterSettings, ThemeCss, ThemeCssGenerator,
    ThemeWriter,
};
use gnomex_app::use_cases::{apply_theme::reconcile_flags, ApplyThemeUseCase};
use gnomex_app::AppError;
use gnomex_domain::{
    PerAppScaleOverride, ScaleFactor, ScalingSpec, TextScaling, ThemeSpec,
};
use gnomex_infra::desktop_app_launcher_overrides::{
    rewrite_exec_lines, DesktopAppLauncherOverrides,
};

// ---- Minimal in-memory test doubles (mirror the app crate's test mocks) ----

#[derive(Default)]
struct StubAppearance;
impl AppearanceSettings for StubAppearance {
    fn get_gtk_theme(&self) -> Result<String, AppError> { Ok(String::new()) }
    fn set_gtk_theme(&self, _n: &str) -> Result<(), AppError> { Ok(()) }
    fn get_icon_theme(&self) -> Result<String, AppError> { Ok(String::new()) }
    fn set_icon_theme(&self, _n: &str) -> Result<(), AppError> { Ok(()) }
    fn get_cursor_theme(&self) -> Result<String, AppError> { Ok(String::new()) }
    fn set_cursor_theme(&self, _n: &str) -> Result<(), AppError> { Ok(()) }
    fn get_shell_theme(&self) -> Result<String, AppError> { Ok(String::new()) }
    fn set_shell_theme(&self, _n: &str) -> Result<(), AppError> { Ok(()) }
    fn get_wallpaper(&self) -> Result<String, AppError> { Ok(String::new()) }
    fn set_wallpaper(&self, _n: &str) -> Result<(), AppError> { Ok(()) }
}

struct StubCssGen;
impl ThemeCssGenerator for StubCssGen {
    fn generate(&self, _s: &ThemeSpec) -> Result<ThemeCss, AppError> {
        Ok(ThemeCss {
            gtk_css: String::new(),
            gtk3_css: String::new(),
            shell_css: String::new(),
        })
    }
    fn version_label(&self) -> &str { "TEST" }
}

#[derive(Default)]
struct StubWriter;
impl ThemeWriter for StubWriter {
    fn write_gtk_css(&self, _gtk4: &str, _gtk3: &str) -> Result<(), AppError> { Ok(()) }
    fn write_shell_css(&self, _c: &str, _n: &str) -> Result<(), AppError> { Ok(()) }
    fn clear_overrides(&self) -> Result<(), AppError> { Ok(()) }
}

#[derive(Default)]
struct RecordingMutter {
    features: Mutex<Vec<String>>,
    text_scale: Mutex<Option<f64>>,
    set_features_calls: Mutex<u32>,
    set_text_calls: Mutex<u32>,
}
impl RecordingMutter {
    fn with_features(flags: Vec<String>) -> Arc<Self> {
        Arc::new(Self {
            features: Mutex::new(flags),
            ..Default::default()
        })
    }
}
impl MutterSettings for RecordingMutter {
    fn experimental_features(&self) -> Result<Vec<String>, AppError> {
        Ok(self.features.lock().unwrap().clone())
    }
    fn set_experimental_features(&self, features: &[String]) -> Result<(), AppError> {
        *self.features.lock().unwrap() = features.to_vec();
        *self.set_features_calls.lock().unwrap() += 1;
        Ok(())
    }
    fn text_scaling_factor(&self) -> Result<f64, AppError> {
        Ok(self.text_scale.lock().unwrap().unwrap_or(1.0))
    }
    fn set_text_scaling_factor(&self, factor: f64) -> Result<(), AppError> {
        *self.text_scale.lock().unwrap() = Some(factor);
        *self.set_text_calls.lock().unwrap() += 1;
        Ok(())
    }
}

#[derive(Default)]
struct RecordingLauncher {
    registered: Mutex<Vec<PerAppScaleOverride>>,
}
impl RecordingLauncher {
    fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}
impl AppLauncherOverrides for RecordingLauncher {
    fn register_override(&self, spec: &PerAppScaleOverride) -> Result<(), AppError> {
        self.registered.lock().unwrap().push(spec.clone());
        Ok(())
    }
    fn remove_override(&self, _app_id: &str) -> Result<(), AppError> { Ok(()) }
    fn list_overrides(&self) -> Result<Vec<String>, AppError> {
        Ok(self
            .registered
            .lock()
            .unwrap()
            .iter()
            .map(|o| o.app_id.clone())
            .collect())
    }
}

// ---- Tests ----

#[test]
fn default_scaling_spec_writes_nothing() {
    // Invariant 1: `ThemeSpec::defaults()` must produce zero scaling
    // side-effects.
    let mutter = RecordingMutter::with_features(vec!["variable-refresh-rate".into()]);
    let launcher = RecordingLauncher::new();
    let uc = ApplyThemeUseCase::new(
        Arc::new(StubCssGen),
        Arc::new(StubWriter),
        Arc::new(StubAppearance),
    )
    .with_mutter_settings(mutter.clone())
    .with_app_launcher_overrides(launcher.clone());

    uc.apply_scaling(&ScalingSpec::default()).unwrap();

    assert_eq!(
        *mutter.set_text_calls.lock().unwrap(),
        0,
        "default text_scaling must not trigger a GSettings write"
    );
    assert_eq!(
        *mutter.set_features_calls.lock().unwrap(),
        0,
        "default flags must not rewrite experimental-features"
    );
    assert!(
        launcher.registered.lock().unwrap().is_empty(),
        "default scaling must not register any per-app overrides"
    );
}

#[test]
fn setting_text_scaling_writes_the_gsetting_exactly_once() {
    let mutter = RecordingMutter::with_features(vec![]);
    let launcher = RecordingLauncher::new();
    let uc = ApplyThemeUseCase::new(
        Arc::new(StubCssGen),
        Arc::new(StubWriter),
        Arc::new(StubAppearance),
    )
    .with_mutter_settings(mutter.clone())
    .with_app_launcher_overrides(launcher);

    let spec = ScalingSpec {
        text_scaling: TextScaling::new(1.25).unwrap(),
        ..Default::default()
    };
    uc.apply_scaling(&spec).unwrap();

    assert_eq!(*mutter.set_text_calls.lock().unwrap(), 1);
    assert_eq!(*mutter.text_scale.lock().unwrap(), Some(1.25));
}

#[test]
fn adding_experimental_flag_preserves_existing_user_flags() {
    // Invariant 3 (the ScopeBoundaries bullet point): a user who has
    // `variable-refresh-rate` set via Tweaks must not see it
    // disappear when we flip `scale-monitor-framebuffer`.
    let mutter = RecordingMutter::with_features(vec![
        "variable-refresh-rate".into(),
    ]);
    let launcher = RecordingLauncher::new();
    let uc = ApplyThemeUseCase::new(
        Arc::new(StubCssGen),
        Arc::new(StubWriter),
        Arc::new(StubAppearance),
    )
    .with_mutter_settings(mutter.clone())
    .with_app_launcher_overrides(launcher);

    let spec = ScalingSpec {
        scale_monitor_framebuffer: true,
        ..Default::default()
    };
    uc.apply_scaling(&spec).unwrap();

    let new = mutter.features.lock().unwrap().clone();
    assert!(
        new.contains(&"variable-refresh-rate".to_string()),
        "unrelated user flag was lost on merge: {new:?}"
    );
    assert!(
        new.contains(&"scale-monitor-framebuffer".to_string()),
        "our flag was not added: {new:?}"
    );
}

#[test]
fn reconcile_flags_is_idempotent() {
    // Calling apply_scaling twice with the same desired state must
    // only produce one write (reconcile_flags returns None on the
    // second call because there's no change).
    let already: Vec<String> = vec!["scale-monitor-framebuffer".into()];
    let result = reconcile_flags(&already, true, false);
    assert!(
        result.is_none(),
        "flag already present → no redundant write (got {result:?})"
    );

    let need = reconcile_flags(&already, true, true).unwrap();
    assert!(need.contains(&"scale-monitor-framebuffer".to_string()));
    assert!(need.contains(&"x11-randr-fractional-scaling".to_string()));
}

#[test]
fn per_app_override_writes_marked_desktop_file_to_fake_home() {
    // End-to-end: point the adapter at a tempdir and verify the
    // generated .desktop file carries the marker comment + rewritten
    // Exec= line.
    let tmp = tempfile::tempdir().unwrap();
    let system_apps: PathBuf = tmp.path().join("usr-share-applications");
    let user_apps: PathBuf = tmp.path().join("local-share-applications");
    std::fs::create_dir_all(&system_apps).unwrap();

    let source = system_apps.join("org.gnome.Nautilus.desktop");
    std::fs::write(
        &source,
        "[Desktop Entry]\nName=Files\nExec=nautilus %U\nType=Application\n",
    )
    .unwrap();

    let adapter = DesktopAppLauncherOverrides::with_dirs(
        user_apps.clone(),
        vec![system_apps.clone()],
    );

    adapter
        .register_override(&PerAppScaleOverride {
            app_id: "org.gnome.Nautilus".into(),
            scale: ScaleFactor::new(1.5).unwrap(),
        })
        .unwrap();

    let out = std::fs::read_to_string(user_apps.join("org.gnome.Nautilus.desktop")).unwrap();
    assert!(
        out.contains("# Generated by GNOME X"),
        "override file missing marker comment:\n{out}"
    );
    assert!(
        out.contains("Exec=env GDK_SCALE=2 GDK_DPI_SCALE=0.750 nautilus %U --force-device-scale-factor=1.50"),
        "Exec line not rewritten as expected:\n{out}"
    );

    let listed = adapter.list_overrides().unwrap();
    assert_eq!(listed, vec!["org.gnome.Nautilus".to_string()]);
}

#[test]
fn rewrite_exec_lines_leaves_unrelated_keys_alone() {
    // Regression guard: the Exec-detector must not be spoofed by
    // `Comment=` or localised name rows containing "Exec" substrings.
    let input = "\
[Desktop Entry]
Name=Code
Comment=Execute the editor
Exec=/usr/bin/code %F
Icon=code
";
    let out = rewrite_exec_lines(input, 1.25);
    assert!(out.contains("Comment=Execute the editor\n"));
    assert!(out.contains("Icon=code\n"));
    assert!(
        out.contains("Exec=env GDK_SCALE=1 GDK_DPI_SCALE=1.250 /usr/bin/code %F --force-device-scale-factor=1.25"),
        "rewrite output malformed:\n{out}"
    );
}

#[test]
fn remove_override_refuses_to_delete_non_managed_file() {
    // Safety: we must not clobber a user-authored .desktop override.
    let tmp = tempfile::tempdir().unwrap();
    let user_apps = tmp.path().join("apps");
    std::fs::create_dir_all(&user_apps).unwrap();
    let not_ours = user_apps.join("firefox.desktop");
    std::fs::write(&not_ours, "[Desktop Entry]\nExec=firefox\n").unwrap();

    let adapter = DesktopAppLauncherOverrides::with_dirs(user_apps.clone(), vec![]);
    let err = adapter.remove_override("firefox").unwrap_err();
    assert!(
        format!("{err}").contains("marker missing"),
        "expected marker-missing error, got: {err}"
    );
    assert!(not_ours.exists(), "file was wrongly removed");
}
