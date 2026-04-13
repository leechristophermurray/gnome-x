// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::ports::{AppearanceSettings, ContentRepository, LocalInstaller};
use crate::AppError;
use gnomex_domain::{ContentCategory, ContentId, ContentItem, ContentState, SearchResult, ThemeType};
use std::sync::Arc;

/// Use case: browse, install, and apply themes/icons/cursors from gnome-look.org.
pub struct CustomizeUseCase {
    content_repo: Arc<dyn ContentRepository>,
    installer: Arc<dyn LocalInstaller>,
    appearance: Arc<dyn AppearanceSettings>,
}

impl CustomizeUseCase {
    pub fn new(
        content_repo: Arc<dyn ContentRepository>,
        installer: Arc<dyn LocalInstaller>,
        appearance: Arc<dyn AppearanceSettings>,
    ) -> Self {
        Self {
            content_repo,
            installer,
            appearance,
        }
    }

    /// Search for content items, cross-referencing against locally installed items.
    pub async fn search_content(
        &self,
        query: &str,
        category: ContentCategory,
        page: u32,
    ) -> Result<SearchResult<ContentItem>, AppError> {
        let mut result = self.content_repo.search(query, category, page).await?;

        let installed_names = match category {
            ContentCategory::GtkTheme | ContentCategory::ShellTheme => {
                self.installer.list_installed_themes().unwrap_or_default()
            }
            ContentCategory::IconTheme => {
                self.installer.list_installed_icons().unwrap_or_default()
            }
            ContentCategory::CursorTheme => {
                self.installer.list_installed_cursors().unwrap_or_default()
            }
            ContentCategory::Wallpaper => vec![],
        };

        let active_name = self.active_name_for(category);

        for item in &mut result.items {
            if installed_names.iter().any(|n| n == &item.name) {
                item.state = if active_name.as_deref() == Some(&item.name) {
                    ContentState::Active
                } else {
                    ContentState::Installed
                };
            }
        }

        Ok(result)
    }

    /// Install a content item by downloading and extracting it.
    pub async fn install_content(
        &self,
        id: ContentId,
        file_id: u64,
        name: &str,
        category: ContentCategory,
    ) -> Result<(), AppError> {
        let data = self.content_repo.download(id, file_id).await?;

        match category {
            ContentCategory::GtkTheme => {
                self.installer
                    .install_theme(name, &data, ThemeType::Gtk4)
                    .await?;
            }
            ContentCategory::ShellTheme => {
                self.installer
                    .install_theme(name, &data, ThemeType::Shell)
                    .await?;
            }
            ContentCategory::IconTheme => {
                self.installer.install_icon_pack(name, &data).await?;
            }
            ContentCategory::CursorTheme => {
                self.installer.install_cursor(name, &data).await?;
            }
            ContentCategory::Wallpaper => {
                // Wallpapers don't get "installed" — they get applied directly.
                // The caller should use apply_content instead.
                return Err(AppError::Install(
                    "use apply_content for wallpapers".into(),
                ));
            }
        }

        Ok(())
    }

    /// Apply a locally installed item as the active theme/icon/cursor.
    pub fn apply_content(
        &self,
        name: &str,
        category: ContentCategory,
    ) -> Result<(), AppError> {
        match category {
            ContentCategory::GtkTheme => self.appearance.set_gtk_theme(name)?,
            ContentCategory::ShellTheme => self.appearance.set_shell_theme(name)?,
            ContentCategory::IconTheme => self.appearance.set_icon_theme(name)?,
            ContentCategory::CursorTheme => self.appearance.set_cursor_theme(name)?,
            ContentCategory::Wallpaper => self.appearance.set_wallpaper(name)?,
        }
        Ok(())
    }

    /// Get the currently active name for a category.
    fn active_name_for(&self, category: ContentCategory) -> Option<String> {
        let result = match category {
            ContentCategory::GtkTheme => self.appearance.get_gtk_theme(),
            ContentCategory::ShellTheme => self.appearance.get_shell_theme(),
            ContentCategory::IconTheme => self.appearance.get_icon_theme(),
            ContentCategory::CursorTheme => self.appearance.get_cursor_theme(),
            ContentCategory::Wallpaper => self.appearance.get_wallpaper(),
        };
        result.ok().filter(|s| !s.is_empty())
    }
}
