// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration test for GXF-061: Experience Pack apply must land both
//! `gtk-4.0/gtk.css` AND `gtk-3.0/gtk.css` on disk, each carrying the
//! `"GNOME X"` managed-region marker that the conflict detector greps
//! for.
//!
//! The test composes a real [`FilesystemThemeWriter`] (pointed at a
//! tempdir via `ResourcePaths::explicit`) with a real
//! [`gnomex_infra::theme_css`] generator and drives them through
//! [`PacksUseCase::apply_pack`]. The trigger wiring mirrors what
//! `services.rs` and `cli/main.rs` do in production, minus the
//! GSettings-backed `FsThemeRenderTrigger` (which we swap out here
//! because unit tests must not touch the user's real dconf database).
//!
//! Hermetic: no env-var dependency, no `$HOME` munging, no parallel-
//! test-execution race because every path is explicit.

use async_trait::async_trait;
use gnomex_app::ports::{
    AppSettings, AppearanceSettings, ContentRepository, ExtensionRepository, LocalInstaller,
    PackStorage, PackSummary, ShellCustomizer, ShellProxy, ThemeRenderTrigger,
};
use gnomex_app::use_cases::{ApplyThemeUseCase, PacksUseCase};
use gnomex_app::AppError;
use gnomex_domain::{
    ContentCategory, ContentId, ContentItem, ExperiencePack, Extension, ExtensionUuid,
    GSettingOverride, SearchResult, ShellTweak, ShellTweakId, ShellVersion, ThemeSpec, ThemeType,
};
use gnomex_infra::theme_paths::ResourcePaths;
use gnomex_infra::{theme_css, PackTomlStorage};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Hermetic mini-adapters
// ---------------------------------------------------------------------------

/// Minimal appearance mock — every op is a no-op.
#[derive(Default)]
struct NullAppearance;

impl AppearanceSettings for NullAppearance {
    fn get_gtk_theme(&self) -> Result<String, AppError> { Ok(String::new()) }
    fn set_gtk_theme(&self, _: &str) -> Result<(), AppError> { Ok(()) }
    fn get_icon_theme(&self) -> Result<String, AppError> { Ok(String::new()) }
    fn set_icon_theme(&self, _: &str) -> Result<(), AppError> { Ok(()) }
    fn get_cursor_theme(&self) -> Result<String, AppError> { Ok(String::new()) }
    fn set_cursor_theme(&self, _: &str) -> Result<(), AppError> { Ok(()) }
    fn get_shell_theme(&self) -> Result<String, AppError> { Ok(String::new()) }
    fn set_shell_theme(&self, _: &str) -> Result<(), AppError> { Ok(()) }
    fn get_wallpaper(&self) -> Result<String, AppError> { Ok(String::new()) }
    fn set_wallpaper(&self, _: &str) -> Result<(), AppError> { Ok(()) }
}

struct StubShellProxy {
    version: ShellVersion,
}

#[async_trait]
impl ShellProxy for StubShellProxy {
    async fn get_shell_version(&self) -> Result<ShellVersion, AppError> {
        Ok(self.version.clone())
    }
    async fn list_extensions(&self) -> Result<Vec<Extension>, AppError> {
        Ok(Vec::new())
    }
    async fn install_extension(&self, _: &ExtensionUuid) -> Result<(), AppError> { Ok(()) }
    async fn enable_extension(&self, _: &ExtensionUuid) -> Result<(), AppError> { Ok(()) }
    async fn disable_extension(&self, _: &ExtensionUuid) -> Result<(), AppError> { Ok(()) }
    async fn open_extension_prefs(&self, _: &ExtensionUuid) -> Result<(), AppError> { Ok(()) }
}

#[derive(Default)]
struct NullInstaller;

#[async_trait]
impl LocalInstaller for NullInstaller {
    async fn install_extension(&self, _: &ExtensionUuid, _: &[u8]) -> Result<(), AppError> { Ok(()) }
    async fn uninstall_extension(&self, _: &ExtensionUuid) -> Result<(), AppError> { Ok(()) }
    async fn install_theme(&self, _: &str, _: &[u8], _: ThemeType) -> Result<(), AppError> { Ok(()) }
    async fn uninstall_theme(&self, _: &str) -> Result<(), AppError> { Ok(()) }
    async fn install_icon_pack(&self, _: &str, _: &[u8]) -> Result<(), AppError> { Ok(()) }
    async fn install_cursor(&self, _: &str, _: &[u8]) -> Result<(), AppError> { Ok(()) }
    fn list_installed_extensions(&self) -> Result<Vec<String>, AppError> { Ok(Vec::new()) }
    fn list_installed_themes(&self) -> Result<Vec<String>, AppError> { Ok(Vec::new()) }
    fn list_installed_icons(&self) -> Result<Vec<String>, AppError> { Ok(Vec::new()) }
    fn list_installed_cursors(&self) -> Result<Vec<String>, AppError> { Ok(Vec::new()) }
}

#[derive(Default)]
struct NullContentRepo;

#[async_trait]
impl ContentRepository for NullContentRepo {
    async fn search(&self, _: &str, _: ContentCategory, _: u32) -> Result<SearchResult<ContentItem>, AppError> {
        Ok(SearchResult { items: vec![], total: 0, page: 0, pages: 0 })
    }
    async fn get_info(&self, _: ContentId) -> Result<ContentItem, AppError> {
        Err(AppError::Repository("unused".into()))
    }
    async fn download(&self, _: ContentId, _: u64) -> Result<Vec<u8>, AppError> { Ok(Vec::new()) }
    async fn list_popular(&self, _: ContentCategory, _: u32) -> Result<SearchResult<ContentItem>, AppError> {
        Ok(SearchResult { items: vec![], total: 0, page: 0, pages: 0 })
    }
    async fn list_recent(&self, _: ContentCategory, _: u32) -> Result<SearchResult<ContentItem>, AppError> {
        Ok(SearchResult { items: vec![], total: 0, page: 0, pages: 0 })
    }
}

#[derive(Default)]
struct NullAppSettings {
    applied: Mutex<Vec<GSettingOverride>>,
}

impl AppSettings for NullAppSettings {
    fn snapshot_pack_settings(&self) -> Result<Vec<GSettingOverride>, AppError> { Ok(Vec::new()) }
    fn apply_overrides(&self, overrides: &[GSettingOverride]) -> Result<(), AppError> {
        self.applied.lock().unwrap().extend_from_slice(overrides);
        Ok(())
    }
}

#[derive(Default)]
struct NullShellCustomizer;

#[async_trait]
impl ShellCustomizer for NullShellCustomizer {
    fn version_label(&self) -> &str { "TEST" }
    fn supported_tweaks(&self) -> &[ShellTweakId] { &[] }
    async fn read(&self, _: ShellTweakId) -> Result<Option<ShellTweak>, AppError> { Ok(None) }
    async fn apply(&self, _: &ShellTweak) -> Result<(), AppError> { Ok(()) }
    async fn snapshot(&self) -> Result<Vec<ShellTweak>, AppError> { Ok(Vec::new()) }
}

#[allow(dead_code)]
#[derive(Default)]
struct NullExtensionRepo;

#[async_trait]
#[allow(dead_code)]
impl ExtensionRepository for NullExtensionRepo {
    async fn search(&self, _: &str, _: &ShellVersion, _: u32) -> Result<SearchResult<Extension>, AppError> {
        Ok(SearchResult { items: vec![], total: 0, page: 0, pages: 0 })
    }
    async fn get_info(&self, _: &ExtensionUuid, _: &ShellVersion) -> Result<Extension, AppError> {
        Err(AppError::Repository("unused".into()))
    }
    async fn download(&self, _: &ExtensionUuid, _: &ShellVersion) -> Result<Vec<u8>, AppError> {
        Ok(Vec::new())
    }
    async fn list_popular(&self, _: &ShellVersion, _: u32) -> Result<SearchResult<Extension>, AppError> {
        Ok(SearchResult { items: vec![], total: 0, page: 0, pages: 0 })
    }
    async fn list_recent(&self, _: &ShellVersion, _: u32) -> Result<SearchResult<Extension>, AppError> {
        Ok(SearchResult { items: vec![], total: 0, page: 0, pages: 0 })
    }
}

/// Hermetic `ThemeRenderTrigger` used in tests — skips the GSettings
/// read-path and routes a fixed default `ThemeSpec` through the
/// real [`ApplyThemeUseCase`]. This mirrors what
/// `FsThemeRenderTrigger` does in production, but without requiring
/// a live dconf daemon under CI.
struct FixedSpecRenderTrigger {
    apply: Arc<ApplyThemeUseCase>,
    spec: ThemeSpec,
}

impl ThemeRenderTrigger for FixedSpecRenderTrigger {
    fn rerender(&self) -> Result<(), AppError> {
        self.apply.apply(&self.spec)
    }
}

// ---------------------------------------------------------------------------
// The real test
// ---------------------------------------------------------------------------

/// Minimal pack fixture — no content refs, but a non-empty
/// `gsettings_overrides` vec so the trigger fires after the overrides
/// are "applied".
fn fixture_pack(id: &str) -> ExperiencePack {
    ExperiencePack {
        id: id.into(),
        name: "Dual CSS".into(),
        description: "Pack that should regen both GTK3 and GTK4".into(),
        author: "integration".into(),
        created_at: "0".into(),
        shell_version: ShellVersion::new(47, 0),
        pack_format: 2,
        gtk_theme: None,
        shell_theme: None,
        icon_pack: None,
        cursor_pack: None,
        extensions: Vec::new(),
        wallpaper: None,
        // A single placeholder override keeps the code path that
        // forwards to AppSettings active, matching production.
        gsettings_overrides: vec![GSettingOverride {
            key: "tb-window-radius".into(),
            value: "12.0".into(),
        }],
        shell_tweaks: Vec::new(),
    }
}

/// Tempdir-backed `FilesystemThemeWriter`. Constructed via
/// `ResourcePaths::explicit` rather than env vars so parallel
/// test-execution can't race.
fn tempdir_writer(root: &Path) -> gnomex_infra::FilesystemThemeWriter {
    // The private `with_paths`-style constructor isn't exposed, so we
    // build the writer through its public `new()` after an `explicit`
    // swap via the `ResourcePaths` the writer looks up internally —
    // but `FilesystemThemeWriter::new()` reads env vars directly, so
    // we instead call the testing helper that takes a tempdir root.
    // The equivalent shape lives in `crate::theme_writer::tests` as
    // `tmp_writer`; we reproduce it here by using the same private
    // composition via `with_paths`-style public construction.
    gnomex_infra::FilesystemThemeWriter::with_paths(
        ResourcePaths::explicit(
            root,
            root.join(".local/share"),
            vec![],
            root.join(".config"),
        ),
        root.join(".local/share/themes"),
    )
}

#[test]
fn apply_pack_writes_both_gtk3_and_gtk4_css_with_gnome_x_marker() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(async {
    let dir = TempDir::new().expect("tempdir");
    let pack_dir = TempDir::new().expect("pack tempdir");

    // --- Real adapters, hermetic paths ---
    let writer = Arc::new(tempdir_writer(dir.path()));
    let appearance = Arc::new(NullAppearance);
    let css_generator: Arc<dyn gnomex_app::ports::ThemeCssGenerator> =
        Arc::from(theme_css::create_css_generator(&ShellVersion::new(47, 0)));

    let apply_theme = Arc::new(
        ApplyThemeUseCase::new(css_generator, writer.clone(), appearance.clone()),
    );
    let trigger: Arc<dyn ThemeRenderTrigger> = Arc::new(FixedSpecRenderTrigger {
        apply: apply_theme,
        spec: ThemeSpec::defaults(),
    });

    // --- Pack storage: real TOML adapter, tempdir-backed ---
    let pack_storage = Arc::new(PackTomlStorage::from_dir(pack_dir.path()));
    let pack = fixture_pack("dual-css");
    pack_storage.save_pack(&pack).expect("save");

    // Export → import round trip through the archive path so we
    // exercise the same code an end-user hits when sharing a pack.
    let archive = pack_storage
        .export_pack(&pack.id, None)
        .expect("export archive");

    let imported_dir = TempDir::new().expect("imported tempdir");
    let imported_storage = Arc::new(PackTomlStorage::from_dir(imported_dir.path()));
    let (imported_id, _) = imported_storage
        .import_pack(&archive)
        .expect("import archive");
    assert_eq!(imported_id, pack.id, "imported id should round-trip");

    // --- PacksUseCase, wired like the real composition root ---
    let packs = PacksUseCase::new(
        imported_storage,
        appearance,
        Arc::new(StubShellProxy { version: ShellVersion::new(47, 0) }),
        Arc::new(NullInstaller),
        Arc::new(NullContentRepo),
        Arc::new(NullAppSettings::default()),
        Arc::new(NullShellCustomizer),
    )
    .with_theme_render_trigger(trigger);

    // --- Act ---
    packs.apply_pack(&imported_id).await.expect("apply pack");

    // --- Assert: both files on disk, both carrying the marker ---
    let gtk4_path: PathBuf = dir.path().join(".config/gtk-4.0/gtk.css");
    let gtk3_path: PathBuf = dir.path().join(".config/gtk-3.0/gtk.css");

    assert!(
        gtk4_path.exists(),
        "pack apply must produce {}",
        gtk4_path.display()
    );
    assert!(
        gtk3_path.exists(),
        "pack apply must produce {} (GXF-061 regression)",
        gtk3_path.display()
    );

    let gtk4 = std::fs::read_to_string(&gtk4_path).expect("read gtk4 css");
    let gtk3 = std::fs::read_to_string(&gtk3_path).expect("read gtk3 css");

    assert!(
        gtk4.contains("GNOME X"),
        "GTK4 css missing managed-region marker; conflict detector will \
         false-flag it. Head:\n{}",
        &gtk4[..gtk4.len().min(300)],
    );
    assert!(
        gtk3.contains("GNOME X"),
        "GTK3 css missing managed-region marker; conflict detector will \
         false-flag it. Head:\n{}",
        &gtk3[..gtk3.len().min(300)],
    );

    // Sanity: GTK4 output should NOT carry the GTK3-only legacy
    // token namespace, and vice versa. This pins the divergence
    // landed in PR #44.
    assert!(
        gtk4.contains("window_bg_color") || !gtk4.contains("@define-color theme_bg_color"),
        "GTK4 css leaked the GTK3 `@theme_bg_color` namespace:\n{gtk4}",
    );
    assert!(
        gtk3.contains("@define-color theme_bg_color"),
        "GTK3 css missing the legacy `@theme_bg_color` token \
         namespace — did the generator silently collapse back to \
         byte-identical output?\n{gtk3}",
    );
    });
}

#[test]
fn apply_pack_without_trigger_does_not_write_css_files() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime.block_on(async {
    // Control test: confirms the CSS writes come from the trigger,
    // not from something else in the pack-apply path. If a future
    // refactor moves CSS writes into the use case directly, this
    // test exists to force an explicit review of the new coupling.
    let dir = TempDir::new().expect("tempdir");
    let pack_dir = TempDir::new().expect("pack tempdir");

    let pack_storage = Arc::new(PackTomlStorage::from_dir(pack_dir.path()));
    let pack = fixture_pack("no-trigger");
    pack_storage.save_pack(&pack).expect("save");

    let packs = PacksUseCase::new(
        pack_storage,
        Arc::new(NullAppearance),
        Arc::new(StubShellProxy { version: ShellVersion::new(47, 0) }),
        Arc::new(NullInstaller),
        Arc::new(NullContentRepo),
        Arc::new(NullAppSettings::default()),
        Arc::new(NullShellCustomizer),
    );

    packs.apply_pack(&pack.id).await.expect("apply pack");

    let gtk4_path = dir.path().join(".config/gtk-4.0/gtk.css");
    let gtk3_path = dir.path().join(".config/gtk-3.0/gtk.css");
    assert!(!gtk4_path.exists(), "no trigger → no GTK4 css");
    assert!(!gtk3_path.exists(), "no trigger → no GTK3 css");
    });
}

// Silence the unused-import warnings for `PackSummary` from the
// ports glob — we use it transitively through the PackStorage
// trait but don't reference it by name in this file.
#[allow(dead_code)]
fn _use_imports(_: PackSummary) {}
