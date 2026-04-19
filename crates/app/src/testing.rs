// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Hand-rolled test doubles for every port.
//!
//! Compiled only under `cfg(test)` so no mock code ends up in release
//! builds. Tests across this crate's `use_cases` module use these to
//! drive the Blue layer without touching real GSettings, D-Bus, HTTP,
//! or the filesystem.
//!
//! **Pattern:** each mock is a struct of `Arc<Mutex<...>>` fields
//! holding a recorded call log and configurable responses. Methods
//! append to the log and return whatever the test set up. Keeping
//! mutability inside `Mutex` lets us satisfy the `&self, Send + Sync`
//! trait bounds while still letting tests mutate call logs.

#![cfg(test)]
#![allow(dead_code)]

use crate::ports::{
    AppSettings, AppearanceSettings, BlurMyShellController, ContentRepository,
    ExternalAppThemer, ExtensionRepository, FloatingDockController, LocalInstaller,
    PackStorage, PackSummary, ShellCustomizer, ShellProxy, ThemeCss, ThemeCssGenerator,
    ThemeWriter,
};
use crate::AppError;
use async_trait::async_trait;
use gnomex_domain::{
    ContentCategory, ContentId, ContentItem, ExperiencePack, Extension, ExtensionUuid,
    ExternalThemeSpec, GSettingOverride, PanelPosition, SearchResult, ShellTweak, ShellTweakId,
    ShellVersion, ThemeSpec, ThemeType, TweakValue,
};
use std::sync::{Arc, Mutex};

// ---------------------------------------------------------------------------
// AppearanceSettings
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MockAppearance {
    pub gtk_theme: Mutex<String>,
    pub shell_theme: Mutex<String>,
    pub icon_theme: Mutex<String>,
    pub cursor_theme: Mutex<String>,
    pub wallpaper: Mutex<String>,
    pub calls: Mutex<Vec<String>>,
}

impl MockAppearance {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl AppearanceSettings for MockAppearance {
    fn get_gtk_theme(&self) -> Result<String, AppError> {
        self.calls.lock().unwrap().push("get_gtk_theme".into());
        Ok(self.gtk_theme.lock().unwrap().clone())
    }
    fn set_gtk_theme(&self, name: &str) -> Result<(), AppError> {
        self.calls.lock().unwrap().push(format!("set_gtk_theme({name})"));
        *self.gtk_theme.lock().unwrap() = name.to_owned();
        Ok(())
    }
    fn get_icon_theme(&self) -> Result<String, AppError> {
        Ok(self.icon_theme.lock().unwrap().clone())
    }
    fn set_icon_theme(&self, name: &str) -> Result<(), AppError> {
        self.calls.lock().unwrap().push(format!("set_icon_theme({name})"));
        *self.icon_theme.lock().unwrap() = name.to_owned();
        Ok(())
    }
    fn get_cursor_theme(&self) -> Result<String, AppError> {
        Ok(self.cursor_theme.lock().unwrap().clone())
    }
    fn set_cursor_theme(&self, name: &str) -> Result<(), AppError> {
        self.calls.lock().unwrap().push(format!("set_cursor_theme({name})"));
        *self.cursor_theme.lock().unwrap() = name.to_owned();
        Ok(())
    }
    fn get_shell_theme(&self) -> Result<String, AppError> {
        Ok(self.shell_theme.lock().unwrap().clone())
    }
    fn set_shell_theme(&self, name: &str) -> Result<(), AppError> {
        self.calls.lock().unwrap().push(format!("set_shell_theme({name})"));
        *self.shell_theme.lock().unwrap() = name.to_owned();
        Ok(())
    }
    fn get_wallpaper(&self) -> Result<String, AppError> {
        Ok(self.wallpaper.lock().unwrap().clone())
    }
    fn set_wallpaper(&self, uri: &str) -> Result<(), AppError> {
        self.calls.lock().unwrap().push(format!("set_wallpaper({uri})"));
        *self.wallpaper.lock().unwrap() = uri.to_owned();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ThemeCssGenerator
// ---------------------------------------------------------------------------

pub struct MockCssGenerator {
    pub label: String,
    pub last_spec: Mutex<Option<ThemeSpec>>,
}

impl MockCssGenerator {
    pub fn new() -> Arc<Self> {
        Arc::new(Self { label: "MOCK GNOME".into(), last_spec: Mutex::new(None) })
    }
}

impl ThemeCssGenerator for MockCssGenerator {
    fn generate(&self, spec: &ThemeSpec) -> Result<ThemeCss, AppError> {
        *self.last_spec.lock().unwrap() = Some(spec.clone());
        Ok(ThemeCss {
            gtk_css: "/* mock gtk */".to_owned(),
            shell_css: "/* mock shell */".to_owned(),
        })
    }
    fn version_label(&self) -> &str {
        &self.label
    }
}

// ---------------------------------------------------------------------------
// ThemeWriter
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MockThemeWriter {
    pub calls: Mutex<Vec<String>>,
    pub last_gtk_css: Mutex<Option<String>>,
    pub last_shell_css: Mutex<Option<(String, String)>>,
}

impl MockThemeWriter {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

impl ThemeWriter for MockThemeWriter {
    fn write_gtk_css(&self, css: &str) -> Result<(), AppError> {
        self.calls.lock().unwrap().push("write_gtk_css".into());
        *self.last_gtk_css.lock().unwrap() = Some(css.to_owned());
        Ok(())
    }
    fn write_shell_css(&self, css: &str, theme_name: &str) -> Result<(), AppError> {
        self.calls.lock().unwrap().push(format!("write_shell_css({theme_name})"));
        *self.last_shell_css.lock().unwrap() = Some((css.to_owned(), theme_name.to_owned()));
        Ok(())
    }
    fn clear_overrides(&self) -> Result<(), AppError> {
        self.calls.lock().unwrap().push("clear_overrides".into());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ExternalAppThemer
// ---------------------------------------------------------------------------

pub struct MockExternalThemer {
    pub name: String,
    pub applies: Mutex<Vec<ExternalThemeSpec>>,
    pub resets: Mutex<u32>,
}

impl MockExternalThemer {
    pub fn new(name: &str) -> Arc<Self> {
        Arc::new(Self {
            name: name.to_owned(),
            applies: Mutex::new(Vec::new()),
            resets: Mutex::new(0),
        })
    }
}

impl ExternalAppThemer for MockExternalThemer {
    fn name(&self) -> &str {
        &self.name
    }
    fn apply(&self, spec: &ExternalThemeSpec) -> Result<(), AppError> {
        self.applies.lock().unwrap().push(spec.clone());
        Ok(())
    }
    fn reset(&self) -> Result<(), AppError> {
        *self.resets.lock().unwrap() += 1;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// AppSettings (our own GNOME X schema)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MockAppSettings {
    pub snapshot_value: Mutex<Vec<GSettingOverride>>,
    pub applied: Mutex<Vec<GSettingOverride>>,
}

impl MockAppSettings {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    pub fn with_snapshot(snapshot: Vec<GSettingOverride>) -> Arc<Self> {
        Arc::new(Self {
            snapshot_value: Mutex::new(snapshot),
            applied: Mutex::new(Vec::new()),
        })
    }
}

impl AppSettings for MockAppSettings {
    fn snapshot_pack_settings(&self) -> Result<Vec<GSettingOverride>, AppError> {
        Ok(self.snapshot_value.lock().unwrap().clone())
    }
    fn apply_overrides(&self, overrides: &[GSettingOverride]) -> Result<(), AppError> {
        self.applied.lock().unwrap().extend_from_slice(overrides);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ShellCustomizer
// ---------------------------------------------------------------------------

pub struct MockShellCustomizer {
    pub label: String,
    pub supported: Vec<ShellTweakId>,
    pub applies: Mutex<Vec<ShellTweak>>,
    pub snapshot_value: Mutex<Vec<ShellTweak>>,
    pub reads: Mutex<Vec<ShellTweakId>>,
}

impl MockShellCustomizer {
    pub fn new(supported: Vec<ShellTweakId>) -> Arc<Self> {
        Arc::new(Self {
            label: "MOCK GNOME 47+".into(),
            supported,
            applies: Mutex::new(Vec::new()),
            snapshot_value: Mutex::new(Vec::new()),
            reads: Mutex::new(Vec::new()),
        })
    }
    pub fn with_snapshot(supported: Vec<ShellTweakId>, snap: Vec<ShellTweak>) -> Arc<Self> {
        Arc::new(Self {
            label: "MOCK GNOME 47+".into(),
            supported,
            applies: Mutex::new(Vec::new()),
            snapshot_value: Mutex::new(snap),
            reads: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl ShellCustomizer for MockShellCustomizer {
    fn version_label(&self) -> &str {
        &self.label
    }
    fn supported_tweaks(&self) -> &[ShellTweakId] {
        &self.supported
    }
    async fn read(&self, id: ShellTweakId) -> Result<Option<ShellTweak>, AppError> {
        self.reads.lock().unwrap().push(id);
        Ok(None)
    }
    async fn apply(&self, tweak: &ShellTweak) -> Result<(), AppError> {
        self.applies.lock().unwrap().push(tweak.clone());
        Ok(())
    }
    async fn snapshot(&self) -> Result<Vec<ShellTweak>, AppError> {
        Ok(self.snapshot_value.lock().unwrap().clone())
    }
}

// ---------------------------------------------------------------------------
// PackStorage
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MockPackStorage {
    pub saved: Mutex<Vec<ExperiencePack>>,
    pub loadable: Mutex<Vec<ExperiencePack>>,
}

impl MockPackStorage {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
    pub fn with_loadable(packs: Vec<ExperiencePack>) -> Arc<Self> {
        Arc::new(Self {
            saved: Mutex::new(Vec::new()),
            loadable: Mutex::new(packs),
        })
    }
}

impl PackStorage for MockPackStorage {
    fn save_pack(&self, pack: &ExperiencePack) -> Result<String, AppError> {
        self.saved.lock().unwrap().push(pack.clone());
        Ok(pack.id.clone())
    }
    fn load_pack(&self, id: &str) -> Result<ExperiencePack, AppError> {
        self.loadable
            .lock()
            .unwrap()
            .iter()
            .find(|p| p.id == id)
            .cloned()
            .ok_or_else(|| AppError::Storage(format!("pack {id} not found")))
    }
    fn list_packs(&self) -> Result<Vec<PackSummary>, AppError> {
        Ok(self
            .loadable
            .lock()
            .unwrap()
            .iter()
            .map(|p| PackSummary {
                id: p.id.clone(),
                name: p.name.clone(),
                author: p.author.clone(),
                description: p.description.clone(),
            })
            .collect())
    }
    fn delete_pack(&self, _id: &str) -> Result<(), AppError> {
        Ok(())
    }
    fn export_pack(&self, _id: &str, _screenshot: Option<&[u8]>) -> Result<Vec<u8>, AppError> {
        Ok(Vec::new())
    }
    fn import_pack(&self, _archive: &[u8]) -> Result<(String, Option<Vec<u8>>), AppError> {
        Ok(("imported".into(), None))
    }
}

// ---------------------------------------------------------------------------
// ShellProxy
// ---------------------------------------------------------------------------

pub struct MockShellProxy {
    pub version: ShellVersion,
    pub extensions: Mutex<Vec<Extension>>,
    pub enabled: Mutex<Vec<String>>,
    pub installed: Mutex<Vec<String>>,
}

impl MockShellProxy {
    pub fn new(version: ShellVersion) -> Arc<Self> {
        Arc::new(Self {
            version,
            extensions: Mutex::new(Vec::new()),
            enabled: Mutex::new(Vec::new()),
            installed: Mutex::new(Vec::new()),
        })
    }
    pub fn with_extensions(version: ShellVersion, exts: Vec<Extension>) -> Arc<Self> {
        Arc::new(Self {
            version,
            extensions: Mutex::new(exts),
            enabled: Mutex::new(Vec::new()),
            installed: Mutex::new(Vec::new()),
        })
    }
}

#[async_trait]
impl ShellProxy for MockShellProxy {
    async fn get_shell_version(&self) -> Result<ShellVersion, AppError> {
        Ok(self.version.clone())
    }
    async fn list_extensions(&self) -> Result<Vec<Extension>, AppError> {
        Ok(self.extensions.lock().unwrap().clone())
    }
    async fn install_extension(&self, uuid: &ExtensionUuid) -> Result<(), AppError> {
        self.installed.lock().unwrap().push(uuid.as_str().to_owned());
        Ok(())
    }
    async fn enable_extension(&self, uuid: &ExtensionUuid) -> Result<(), AppError> {
        self.enabled.lock().unwrap().push(uuid.as_str().to_owned());
        Ok(())
    }
    async fn disable_extension(&self, _uuid: &ExtensionUuid) -> Result<(), AppError> {
        Ok(())
    }
    async fn open_extension_prefs(&self, _uuid: &ExtensionUuid) -> Result<(), AppError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// LocalInstaller
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MockInstaller {
    pub themes: Mutex<Vec<String>>,
    pub icons: Mutex<Vec<String>>,
    pub cursors: Mutex<Vec<String>>,
    pub installed_extensions: Mutex<Vec<String>>,
}

impl MockInstaller {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl LocalInstaller for MockInstaller {
    async fn install_extension(
        &self,
        uuid: &ExtensionUuid,
        _zip_data: &[u8],
    ) -> Result<(), AppError> {
        self.installed_extensions
            .lock()
            .unwrap()
            .push(uuid.as_str().to_owned());
        Ok(())
    }
    async fn uninstall_extension(&self, _uuid: &ExtensionUuid) -> Result<(), AppError> {
        Ok(())
    }
    async fn install_theme(
        &self,
        name: &str,
        _archive_data: &[u8],
        _kind: ThemeType,
    ) -> Result<(), AppError> {
        self.themes.lock().unwrap().push(name.to_owned());
        Ok(())
    }
    async fn uninstall_theme(&self, _name: &str) -> Result<(), AppError> {
        Ok(())
    }
    async fn install_icon_pack(&self, name: &str, _archive_data: &[u8]) -> Result<(), AppError> {
        self.icons.lock().unwrap().push(name.to_owned());
        Ok(())
    }
    async fn install_cursor(&self, name: &str, _archive_data: &[u8]) -> Result<(), AppError> {
        self.cursors.lock().unwrap().push(name.to_owned());
        Ok(())
    }
    fn list_installed_extensions(&self) -> Result<Vec<String>, AppError> {
        Ok(self.installed_extensions.lock().unwrap().clone())
    }
    fn list_installed_themes(&self) -> Result<Vec<String>, AppError> {
        Ok(self.themes.lock().unwrap().clone())
    }
    fn list_installed_icons(&self) -> Result<Vec<String>, AppError> {
        Ok(self.icons.lock().unwrap().clone())
    }
    fn list_installed_cursors(&self) -> Result<Vec<String>, AppError> {
        Ok(self.cursors.lock().unwrap().clone())
    }
}

// ---------------------------------------------------------------------------
// ContentRepository (gnome-look.org OCS)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MockContentRepo {
    pub download_bytes: Mutex<Vec<u8>>,
}

impl MockContentRepo {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl ContentRepository for MockContentRepo {
    async fn search(
        &self,
        _query: &str,
        _category: ContentCategory,
        _page: u32,
    ) -> Result<SearchResult<ContentItem>, AppError> {
        Ok(SearchResult { items: vec![], total: 0, page: 0, pages: 0 })
    }
    async fn get_info(&self, _id: ContentId) -> Result<ContentItem, AppError> {
        Err(AppError::Repository("not implemented in mock".into()))
    }
    async fn download(&self, _id: ContentId, _file_id: u64) -> Result<Vec<u8>, AppError> {
        Ok(self.download_bytes.lock().unwrap().clone())
    }
    async fn list_popular(
        &self,
        _category: ContentCategory,
        _page: u32,
    ) -> Result<SearchResult<ContentItem>, AppError> {
        Ok(SearchResult { items: vec![], total: 0, page: 0, pages: 0 })
    }
    async fn list_recent(
        &self,
        _category: ContentCategory,
        _page: u32,
    ) -> Result<SearchResult<ContentItem>, AppError> {
        Ok(SearchResult { items: vec![], total: 0, page: 0, pages: 0 })
    }
}

// ---------------------------------------------------------------------------
// ExtensionRepository (EGO)
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MockExtensionRepo;

impl MockExtensionRepo {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl ExtensionRepository for MockExtensionRepo {
    async fn search(
        &self,
        _query: &str,
        _shell_version: &ShellVersion,
        _page: u32,
    ) -> Result<SearchResult<Extension>, AppError> {
        Ok(SearchResult { items: vec![], total: 0, page: 0, pages: 0 })
    }
    async fn get_info(
        &self,
        _uuid: &ExtensionUuid,
        _shell_version: &ShellVersion,
    ) -> Result<Extension, AppError> {
        Err(AppError::Repository("not implemented in mock".into()))
    }
    async fn download(
        &self,
        _uuid: &ExtensionUuid,
        _shell_version: &ShellVersion,
    ) -> Result<Vec<u8>, AppError> {
        Ok(Vec::new())
    }
    async fn list_popular(
        &self,
        _shell_version: &ShellVersion,
        _page: u32,
    ) -> Result<SearchResult<Extension>, AppError> {
        Ok(SearchResult { items: vec![], total: 0, page: 0, pages: 0 })
    }
    async fn list_recent(
        &self,
        _shell_version: &ShellVersion,
        _page: u32,
    ) -> Result<SearchResult<Extension>, AppError> {
        Ok(SearchResult { items: vec![], total: 0, page: 0, pages: 0 })
    }
}

// ---------------------------------------------------------------------------
// BlurMyShell / FloatingDock controllers
// ---------------------------------------------------------------------------

pub struct MockBlur {
    pub available: bool,
    pub applies: Mutex<Vec<bool>>,
}
impl MockBlur {
    pub fn new(available: bool) -> Arc<Self> {
        Arc::new(Self { available, applies: Mutex::new(Vec::new()) })
    }
}
impl BlurMyShellController for MockBlur {
    fn is_available(&self) -> bool { self.available }
    fn apply(&self, enabled: bool) -> Result<(), AppError> {
        self.applies.lock().unwrap().push(enabled);
        Ok(())
    }
}

pub struct MockDock {
    pub available: bool,
    pub applies: Mutex<Vec<bool>>,
}
impl MockDock {
    pub fn new(available: bool) -> Arc<Self> {
        Arc::new(Self { available, applies: Mutex::new(Vec::new()) })
    }
}
impl FloatingDockController for MockDock {
    fn is_available(&self) -> bool { self.available }
    fn apply(&self, enabled: bool) -> Result<(), AppError> {
        self.applies.lock().unwrap().push(enabled);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Shared fixtures
// ---------------------------------------------------------------------------

/// A minimal, valid `ThemeSpec` for use across tests.
pub fn sample_theme_spec() -> ThemeSpec {
    ThemeSpec::defaults()
}

/// A minimal, valid `ExperiencePack` for use across tests.
pub fn sample_pack(id: &str) -> ExperiencePack {
    ExperiencePack {
        id: id.into(),
        name: "Test Pack".into(),
        description: "For tests".into(),
        author: "tester".into(),
        created_at: "0".into(),
        shell_version: ShellVersion::new(47, 0),
        pack_format: 2,
        gtk_theme: None,
        shell_theme: None,
        icon_pack: None,
        cursor_pack: None,
        extensions: Vec::new(),
        wallpaper: None,
        gsettings_overrides: Vec::new(),
        shell_tweaks: Vec::new(),
    }
}

/// Conveniently build a `ShellTweak::Bool(EnableAnimations, v)`.
pub fn enable_animations(v: bool) -> ShellTweak {
    ShellTweak {
        id: ShellTweakId::EnableAnimations,
        value: TweakValue::Bool(v),
    }
}

// Silence unused-import warnings for fixtures not yet referenced
// by any test.
pub fn _use_all() {
    let _ = PanelPosition::Top;
}
