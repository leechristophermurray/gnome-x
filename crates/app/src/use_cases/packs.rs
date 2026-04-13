// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::ports::{
    AppearanceSettings, ContentRepository, LocalInstaller, PackStorage, PackSummary, ShellProxy,
};
use crate::AppError;
use gnomex_domain::{ExperiencePack, ExtensionRef, ExtensionState, ThemeType};
use std::sync::Arc;

/// Use case: snapshot, list, and apply Experience Packs.
pub struct PacksUseCase {
    pack_storage: Arc<dyn PackStorage>,
    appearance: Arc<dyn AppearanceSettings>,
    shell: Arc<dyn ShellProxy>,
    installer: Arc<dyn LocalInstaller>,
    content_repo: Arc<dyn ContentRepository>,
}

impl PacksUseCase {
    pub fn new(
        pack_storage: Arc<dyn PackStorage>,
        appearance: Arc<dyn AppearanceSettings>,
        shell: Arc<dyn ShellProxy>,
        installer: Arc<dyn LocalInstaller>,
        content_repo: Arc<dyn ContentRepository>,
    ) -> Self {
        Self {
            pack_storage,
            appearance,
            shell,
            installer,
            content_repo,
        }
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
            pack_format: 1,
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
            gsettings_overrides: vec![],
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
