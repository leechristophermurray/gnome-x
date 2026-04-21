// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::ports::{
    AppSettings, AppearanceSettings, ContentRepository, LocalInstaller, PackStorage, PackSummary,
    ShellCustomizer, ShellProxy, ThemeRenderTrigger,
};
use crate::AppError;
use gnomex_domain::{
    ExperiencePack, ExtensionRef, ExtensionState, PackCompatibilityReport, ShellVersion, ThemeType,
};
use std::sync::Arc;

/// Use case: snapshot, list, and apply Experience Packs.
pub struct PacksUseCase {
    pack_storage: Arc<dyn PackStorage>,
    appearance: Arc<dyn AppearanceSettings>,
    shell: Arc<dyn ShellProxy>,
    installer: Arc<dyn LocalInstaller>,
    content_repo: Arc<dyn ContentRepository>,
    app_settings: Arc<dyn AppSettings>,
    shell_customizer: Arc<dyn ShellCustomizer>,
    /// Optional post-apply theme regenerator. When present, `apply_pack`
    /// runs it after restoring the pack's `gsettings_overrides` so the
    /// newly-restored `tb-*` values turn into GTK 4 + GTK 3 CSS on
    /// disk immediately (GXF-061). When absent (headless tests), the
    /// pack's `tb-*` values are still restored but callers are
    /// responsible for triggering a theme re-render.
    theme_render_trigger: Option<Arc<dyn ThemeRenderTrigger>>,
}

impl PacksUseCase {
    pub fn new(
        pack_storage: Arc<dyn PackStorage>,
        appearance: Arc<dyn AppearanceSettings>,
        shell: Arc<dyn ShellProxy>,
        installer: Arc<dyn LocalInstaller>,
        content_repo: Arc<dyn ContentRepository>,
        app_settings: Arc<dyn AppSettings>,
        shell_customizer: Arc<dyn ShellCustomizer>,
    ) -> Self {
        Self {
            pack_storage,
            appearance,
            shell,
            installer,
            content_repo,
            app_settings,
            shell_customizer,
            theme_render_trigger: None,
        }
    }

    /// Wire a theme render trigger so `apply_pack` regenerates GTK 4 +
    /// GTK 3 CSS after restoring the pack's GSettings overrides. See
    /// [`ThemeRenderTrigger`] and GXF-061.
    pub fn with_theme_render_trigger(
        mut self,
        trigger: Arc<dyn ThemeRenderTrigger>,
    ) -> Self {
        self.theme_render_trigger = Some(trigger);
        self
    }

    /// List all saved packs.
    pub fn list_packs(&self) -> Result<Vec<PackSummary>, AppError> {
        self.pack_storage.list_packs()
    }

    /// Load a pack by ID.
    pub fn load_pack(&self, id: &str) -> Result<ExperiencePack, AppError> {
        self.pack_storage.load_pack(id)
    }

    /// Delete a saved pack.
    pub fn delete_pack(&self, id: &str) -> Result<(), AppError> {
        self.pack_storage.delete_pack(id)
    }

    /// Classify the version-drift between a pack and a target shell
    /// version. Callers should present the report to the user (or at
    /// least log it) before calling [`apply_pack`]. Actual apply is
    /// still safe when warnings are present — the adapter will
    /// log-and-skip each unsupported tweak — but users deserve to
    /// know ahead of time which parts of the pack won't survive.
    ///
    /// Resolves GXF-060.
    pub fn check_compatibility(
        &self,
        id: &str,
        target_version: &ShellVersion,
    ) -> Result<PackCompatibilityReport, AppError> {
        let pack = self.pack_storage.load_pack(id)?;
        Ok(PackCompatibilityReport::build(
            &pack.shell_version,
            target_version,
            &pack.shell_tweaks,
        ))
    }

    /// Export a pack as a portable archive with an optional screenshot.
    pub fn export_pack(&self, id: &str, screenshot: Option<&[u8]>) -> Result<Vec<u8>, AppError> {
        self.pack_storage.export_pack(id, screenshot)
    }

    /// Import a pack from a portable archive.
    /// Returns the pack ID and any bundled screenshot.
    pub fn import_pack(&self, archive: &[u8]) -> Result<(String, Option<Vec<u8>>), AppError> {
        self.pack_storage.import_pack(archive)
    }

    /// Snapshot the current desktop configuration into a new Experience Pack.
    pub async fn snapshot_current(
        &self,
        name: String,
        description: String,
        author: String,
    ) -> Result<String, AppError> {
        let shell_version = self.shell.get_shell_version().await?;

        // Read current appearance settings
        let gtk_theme = self.appearance.get_gtk_theme().ok();
        let shell_theme = self.appearance.get_shell_theme().ok().filter(|s| !s.is_empty());
        let icon_theme = self.appearance.get_icon_theme().ok();
        let cursor_theme = self.appearance.get_cursor_theme().ok();
        let wallpaper = self.appearance.get_wallpaper().ok().filter(|s| !s.is_empty());

        // Read enabled extensions
        let extensions = self.shell.list_extensions().await.unwrap_or_default();
        let ext_refs: Vec<ExtensionRef> = extensions
            .iter()
            .filter(|e| e.state == ExtensionState::Enabled)
            .map(|e| ExtensionRef {
                uuid: e.uuid.as_str().to_owned(),
                name: e.name.clone(),
                required: true,
            })
            .collect();

        // Capture the theme-builder knobs + accent-scheduling + cross-cutting
        // toggles as GSettings overrides. Unknown keys are skipped inside the
        // adapter, so older schemas still produce a valid pack.
        let gsettings_overrides = self
            .app_settings
            .snapshot_pack_settings()
            .unwrap_or_else(|e| {
                tracing::warn!("snapshot pack settings failed: {e}");
                Vec::new()
            });

        // Capture every shell tweak the running version can read.
        // Failures are non-fatal — we'd rather ship a v2 pack missing
        // some tweaks than reject the whole snapshot.
        let shell_tweaks = self
            .shell_customizer
            .snapshot()
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("snapshot shell tweaks failed: {e}");
                Vec::new()
            });

        let id = slug(&name);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| format!("{}", d.as_secs()))
            .unwrap_or_default();

        let pack = ExperiencePack {
            id: id.clone(),
            name,
            description,
            author,
            created_at: now,
            shell_version,
            // v2 introduces shell_tweaks. v1 packs still load cleanly;
            // see ExperiencePack::validate and PackTomlStorage.
            pack_format: 2,
            gtk_theme: gtk_theme.map(|n| gnomex_domain::ThemeRef {
                name: n,
                source: "local".into(),
                content_id: 0,
                file_id: 0,
            }),
            shell_theme: shell_theme.map(|n| gnomex_domain::ThemeRef {
                name: n,
                source: "local".into(),
                content_id: 0,
                file_id: 0,
            }),
            icon_pack: icon_theme.map(|n| gnomex_domain::IconPackRef {
                name: n,
                source: "local".into(),
                content_id: 0,
                file_id: 0,
            }),
            cursor_pack: cursor_theme.map(|n| gnomex_domain::CursorPackRef {
                name: n,
                source: "local".into(),
                content_id: 0,
                file_id: 0,
            }),
            extensions: ext_refs,
            wallpaper,
            gsettings_overrides,
            shell_tweaks,
        };

        self.pack_storage.save_pack(&pack)?;
        Ok(id)
    }

    /// Apply an Experience Pack to the current desktop.
    ///
    /// Sets themes/icons/cursors/wallpaper and enables required extensions.
    /// Content items with a non-zero `content_id` that aren't installed locally
    /// will be downloaded and installed first.
    pub async fn apply_pack(&self, id: &str) -> Result<(), AppError> {
        let pack = self.pack_storage.load_pack(id)?;

        // Log a compatibility breadcrumb even if the caller didn't
        // call `check_compatibility` first. The apply itself stays
        // best-effort — the shell customizer logs-and-skips each
        // unsupported tweak — but this gives postmortem-style
        // visibility when things render unexpectedly.
        let target_version = self.shell.get_shell_version().await.ok();
        if let Some(target) = &target_version {
            let report = PackCompatibilityReport::build(
                &pack.shell_version,
                target,
                &pack.shell_tweaks,
            );
            if report.has_warnings() {
                tracing::warn!(
                    "pack '{}' applied to {} but was built on {} — {} compatibility warning(s)",
                    pack.id,
                    target,
                    pack.shell_version,
                    report.warnings.len()
                );
                for w in &report.warnings {
                    tracing::warn!("  {}", w.summary());
                }
            }
        }

        // Apply GTK theme
        if let Some(ref t) = pack.gtk_theme {
            if t.content_id != 0 && !self.is_theme_installed(&t.name) {
                let data = self.content_repo.download(
                    gnomex_domain::ContentId(t.content_id),
                    t.file_id,
                ).await?;
                self.installer.install_theme(&t.name, &data, ThemeType::Gtk4).await?;
            }
            self.appearance.set_gtk_theme(&t.name)?;
        }

        // Apply shell theme
        if let Some(ref t) = pack.shell_theme {
            if t.content_id != 0 && !self.is_theme_installed(&t.name) {
                let data = self.content_repo.download(
                    gnomex_domain::ContentId(t.content_id),
                    t.file_id,
                ).await?;
                self.installer.install_theme(&t.name, &data, ThemeType::Shell).await?;
            }
            self.appearance.set_shell_theme(&t.name).ok(); // may fail if User Themes not installed
        }

        // Apply icon theme
        if let Some(ref i) = pack.icon_pack {
            if i.content_id != 0 && !self.is_icon_installed(&i.name) {
                let data = self.content_repo.download(
                    gnomex_domain::ContentId(i.content_id),
                    i.file_id,
                ).await?;
                self.installer.install_icon_pack(&i.name, &data).await?;
            }
            self.appearance.set_icon_theme(&i.name)?;
        }

        // Apply cursor theme
        if let Some(ref c) = pack.cursor_pack {
            if c.content_id != 0 && !self.is_cursor_installed(&c.name) {
                let data = self.content_repo.download(
                    gnomex_domain::ContentId(c.content_id),
                    c.file_id,
                ).await?;
                self.installer.install_cursor(&c.name, &data).await?;
            }
            self.appearance.set_cursor_theme(&c.name)?;
        }

        // Apply wallpaper
        if let Some(ref uri) = pack.wallpaper {
            self.appearance.set_wallpaper(uri)?;
        }

        // Enable required extensions
        for ext in &pack.extensions {
            if ext.required {
                if let Ok(uuid) = gnomex_domain::ExtensionUuid::new(&ext.uuid) {
                    self.shell.enable_extension(&uuid).await.ok();
                }
            }
        }

        // Restore the captured theme-builder knobs + accent scheduling.
        // Callers should trigger a theme re-render after this to rebuild the
        // CSS from the newly-applied `tb-*` values (the GUI's pack-apply
        // handler / `experiencectl pack apply` both do this).
        if !pack.gsettings_overrides.is_empty() {
            if let Err(e) = self.app_settings.apply_overrides(&pack.gsettings_overrides) {
                tracing::warn!("pack apply: overrides failed: {e}");
            }
        }

        // Replay shell tweaks through the version-specific customizer.
        // Cross-version application degrades gracefully: tweaks that
        // aren't supported on the target shell are logged and skipped
        // by the adapter, never surfaced as errors.
        for tweak in &pack.shell_tweaks {
            if let Err(e) = self.shell_customizer.apply(tweak).await {
                tracing::warn!("pack apply: shell tweak {:?} failed: {e}", tweak.id);
            }
        }

        // Re-render the theme CSS (GTK 4 + GTK 3 both, via the
        // `ThemeWriter` port) from the just-restored `tb-*` values.
        // Failures here are logged but non-fatal — the pack's
        // GSettings state is already on disk, so the user can trigger
        // a regen manually via the Theme Builder UI if this path
        // stumbles. GXF-061.
        if let Some(trigger) = &self.theme_render_trigger {
            if let Err(e) = trigger.rerender() {
                tracing::warn!("pack apply: theme re-render failed: {e}");
            }
        } else {
            tracing::debug!(
                "pack apply: no ThemeRenderTrigger wired — caller must regenerate CSS"
            );
        }

        Ok(())
    }

    fn is_theme_installed(&self, name: &str) -> bool {
        self.installer
            .list_installed_themes()
            .map(|list| list.iter().any(|n| n == name))
            .unwrap_or(false)
    }

    fn is_icon_installed(&self, name: &str) -> bool {
        self.installer
            .list_installed_icons()
            .map(|list| list.iter().any(|n| n == name))
            .unwrap_or(false)
    }

    fn is_cursor_installed(&self, name: &str) -> bool {
        self.installer
            .list_installed_cursors()
            .map(|list| list.iter().any(|n| n == name))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{
        enable_animations, sample_pack, MockAppSettings, MockAppearance, MockContentRepo,
        MockInstaller, MockPackStorage, MockShellCustomizer, MockShellProxy,
        MockThemeRenderTrigger,
    };
    use gnomex_domain::{GSettingOverride, ShellTweak, ShellTweakId, ShellVersion, TweakValue};

    fn build(
        storage: std::sync::Arc<MockPackStorage>,
        customizer: std::sync::Arc<MockShellCustomizer>,
        app_settings: std::sync::Arc<MockAppSettings>,
    ) -> PacksUseCase {
        PacksUseCase::new(
            storage,
            MockAppearance::new(),
            MockShellProxy::new(ShellVersion::new(47, 0)),
            MockInstaller::new(),
            MockContentRepo::new(),
            app_settings,
            customizer,
        )
    }

    #[tokio::test]
    async fn snapshot_captures_shell_tweaks_and_gsettings_overrides() {
        let storage = MockPackStorage::new();
        let customizer = MockShellCustomizer::with_snapshot(
            vec![ShellTweakId::EnableAnimations],
            vec![enable_animations(true)],
        );
        let app_settings = MockAppSettings::with_snapshot(vec![GSettingOverride {
            key: "tb-window-radius".into(),
            value: "25.0".into(),
        }]);
        let uc = build(storage.clone(), customizer, app_settings);

        let id = uc
            .snapshot_current("Test".into(), "desc".into(), "me".into())
            .await
            .unwrap();

        let saved = storage.saved.lock().unwrap().clone();
        assert_eq!(saved.len(), 1);
        let pack = &saved[0];
        assert_eq!(pack.id, id);
        // v2 format — new packs ship shell_tweaks.
        assert_eq!(pack.pack_format, 2);
        // Shell tweaks came from the customizer.
        assert_eq!(pack.shell_tweaks, vec![enable_animations(true)]);
        // GSettings overrides came from app_settings.
        assert_eq!(pack.gsettings_overrides.len(), 1);
        assert_eq!(pack.gsettings_overrides[0].key, "tb-window-radius");
    }

    #[tokio::test]
    async fn apply_pack_fans_tweaks_to_customizer_and_overrides_to_app_settings() {
        let mut pack = sample_pack("fan-out");
        pack.gsettings_overrides = vec![
            GSettingOverride {
                key: "tb-window-radius".into(),
                value: "30.0".into(),
            },
            GSettingOverride {
                key: "tb-panel-opacity".into(),
                value: "85.0".into(),
            },
        ];
        pack.shell_tweaks = vec![
            enable_animations(false),
            ShellTweak {
                id: ShellTweakId::ShowWeekday,
                value: TweakValue::Bool(true),
            },
        ];

        let storage = MockPackStorage::with_loadable(vec![pack.clone()]);
        let customizer = MockShellCustomizer::new(vec![
            ShellTweakId::EnableAnimations,
            ShellTweakId::ShowWeekday,
        ]);
        let app_settings = MockAppSettings::new();
        let uc = build(storage, customizer.clone(), app_settings.clone());

        uc.apply_pack(&pack.id).await.unwrap();

        // GSettings overrides went through.
        let applied = app_settings.applied.lock().unwrap().clone();
        assert_eq!(applied.len(), 2);
        assert_eq!(applied[0].key, "tb-window-radius");
        assert_eq!(applied[1].key, "tb-panel-opacity");

        // Shell tweaks got replayed in order.
        let tweaks = customizer.applies.lock().unwrap().clone();
        assert_eq!(tweaks.len(), 2);
        assert_eq!(tweaks[0].id, ShellTweakId::EnableAnimations);
        assert_eq!(tweaks[1].id, ShellTweakId::ShowWeekday);
    }

    #[tokio::test]
    async fn apply_pack_with_empty_tweaks_does_not_call_customizer() {
        // v1 packs (and empty v2 packs) shouldn't trigger spurious
        // tweak writes — that would undo state the user may have
        // customized after saving the pack.
        let pack = sample_pack("empty");
        let storage = MockPackStorage::with_loadable(vec![pack.clone()]);
        let customizer = MockShellCustomizer::new(vec![ShellTweakId::EnableAnimations]);
        let app_settings = MockAppSettings::new();
        let uc = build(storage, customizer.clone(), app_settings.clone());

        uc.apply_pack(&pack.id).await.unwrap();

        assert!(customizer.applies.lock().unwrap().is_empty());
        assert!(app_settings.applied.lock().unwrap().is_empty());
    }

    #[test]
    fn list_packs_forwards_to_storage() {
        let packs = vec![
            sample_pack("one"),
            sample_pack("two"),
        ];
        let storage = MockPackStorage::with_loadable(packs);
        let customizer = MockShellCustomizer::new(vec![]);
        let app_settings = MockAppSettings::new();
        let uc = build(storage, customizer, app_settings);

        let summaries = uc.list_packs().unwrap();
        assert_eq!(summaries.len(), 2);
        assert_eq!(summaries[0].id, "one");
        assert_eq!(summaries[1].id, "two");
    }

    #[test]
    fn load_pack_returns_from_storage() {
        let pack = sample_pack("target");
        let storage = MockPackStorage::with_loadable(vec![pack.clone()]);
        let customizer = MockShellCustomizer::new(vec![]);
        let app_settings = MockAppSettings::new();
        let uc = build(storage, customizer, app_settings);

        let loaded = uc.load_pack("target").unwrap();
        assert_eq!(loaded.id, pack.id);
    }

    #[test]
    fn check_compatibility_reports_unsupported_when_downgrading() {
        // Pack saved on GNOME 47 with AppsGridColumns — apply target
        // GNOME 45 where that tweak is Unsupported. Report should
        // surface one Unsupported warning.
        let mut pack = sample_pack("cross-version");
        pack.shell_version = ShellVersion::new(47, 0);
        pack.shell_tweaks = vec![ShellTweak {
            id: ShellTweakId::AppsGridColumns,
            value: TweakValue::Int(6),
        }];

        let storage = MockPackStorage::with_loadable(vec![pack]);
        let customizer = MockShellCustomizer::new(vec![]);
        let app_settings = MockAppSettings::new();
        let uc = build(storage, customizer, app_settings);

        let report = uc
            .check_compatibility("cross-version", &ShellVersion::new(45, 0))
            .unwrap();
        assert!(report.severity.is_major_drift());
        assert_eq!(report.warnings.len(), 1);
    }

    #[test]
    fn check_compatibility_on_matching_version_is_clean() {
        let pack = sample_pack("same-version");
        let storage = MockPackStorage::with_loadable(vec![pack]);
        let customizer = MockShellCustomizer::new(vec![]);
        let app_settings = MockAppSettings::new();
        let uc = build(storage, customizer, app_settings);

        let report = uc
            .check_compatibility("same-version", &ShellVersion::new(47, 0))
            .unwrap();
        assert!(report.is_clean());
    }

    #[tokio::test]
    async fn apply_pack_triggers_theme_rerender_when_wired() {
        // Wiring a `ThemeRenderTrigger` is what guarantees dual GTK 3 +
        // GTK 4 CSS ends up on disk after a pack apply (GXF-061). If this
        // ever regresses, packs start silently dropping the GTK3 half.
        let pack = sample_pack("re-render");
        let storage = MockPackStorage::with_loadable(vec![pack.clone()]);
        let customizer = MockShellCustomizer::new(vec![]);
        let app_settings = MockAppSettings::new();
        let trigger = MockThemeRenderTrigger::new();

        let uc = build(storage, customizer, app_settings)
            .with_theme_render_trigger(trigger.clone());

        uc.apply_pack(&pack.id).await.unwrap();

        assert_eq!(
            *trigger.calls.lock().unwrap(),
            1,
            "apply_pack must call rerender() exactly once"
        );
    }

    #[tokio::test]
    async fn apply_pack_without_trigger_still_succeeds() {
        // Headless unit-test configurations don't need a trigger — the
        // pack apply must still succeed, it just doesn't regenerate CSS.
        let pack = sample_pack("no-trigger");
        let storage = MockPackStorage::with_loadable(vec![pack.clone()]);
        let customizer = MockShellCustomizer::new(vec![]);
        let app_settings = MockAppSettings::new();

        let uc = build(storage, customizer, app_settings);
        uc.apply_pack(&pack.id).await.unwrap();
    }

    #[tokio::test]
    async fn apply_pack_swallows_trigger_failure() {
        // A failing trigger must not abort the apply — the pack's
        // GSettings state is already on disk, so we log and move on.
        let pack = sample_pack("trigger-fails");
        let storage = MockPackStorage::with_loadable(vec![pack.clone()]);
        let customizer = MockShellCustomizer::new(vec![]);
        let app_settings = MockAppSettings::new();
        let trigger = MockThemeRenderTrigger::failing(AppError::Settings(
            "writer exploded".into(),
        ));

        let uc = build(storage, customizer, app_settings)
            .with_theme_render_trigger(trigger.clone());

        // Apply still returns Ok() even when the trigger fails.
        uc.apply_pack(&pack.id).await.unwrap();
        assert_eq!(*trigger.calls.lock().unwrap(), 1);
    }

    #[test]
    fn load_missing_pack_errors() {
        let storage = MockPackStorage::new();
        let customizer = MockShellCustomizer::new(vec![]);
        let app_settings = MockAppSettings::new();
        let uc = build(storage, customizer, app_settings);

        let err = uc.load_pack("nope").unwrap_err();
        // Storage returns a Storage error wrapping "not found".
        assert!(matches!(err, AppError::Storage(_)));
    }
}

/// Generate a URL-safe slug from a name.
fn slug(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}
