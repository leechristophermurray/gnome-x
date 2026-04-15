// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use gnomex_app::ports::ThemeWriter;
use gnomex_app::AppError;
use std::path::PathBuf;

const GTK_CSS_PATH: &str = ".config/gtk-4.0/gtk.css";
const GTK_CSS_BACKUP: &str = ".config/gtk-4.0/gtk.css.gnomex-backup";
const SHELL_THEME_BASE: &str = ".local/share/themes";

/// Concrete adapter: write theme CSS to the local filesystem.
pub struct FilesystemThemeWriter {
    home: PathBuf,
}

impl FilesystemThemeWriter {
    pub fn new() -> Self {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        Self { home }
    }
}

impl ThemeWriter for FilesystemThemeWriter {
    fn write_gtk_css(&self, css: &str) -> Result<(), AppError> {
        let path = self.home.join(GTK_CSS_PATH);
        let backup = self.home.join(GTK_CSS_BACKUP);

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::Settings(format!("mkdir: {e}")))?;
        }

        // Back up existing gtk.css if it wasn't written by us and no backup exists yet
        if path.exists() && !backup.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if !content.contains("GNOME X") {
                    std::fs::copy(&path, &backup).ok();
                    tracing::info!("backed up existing gtk.css");
                }
            }
        }

        std::fs::write(&path, css)
            .map_err(|e| AppError::Settings(format!("write gtk.css: {e}")))?;
        tracing::info!("wrote GTK CSS to {}", path.display());
        Ok(())
    }

    fn write_shell_css(&self, css: &str, theme_name: &str) -> Result<(), AppError> {
        let dir = self
            .home
            .join(SHELL_THEME_BASE)
            .join(theme_name)
            .join("gnome-shell");
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::Settings(format!("mkdir: {e}")))?;
        std::fs::write(dir.join("gnome-shell.css"), css)
            .map_err(|e| AppError::Settings(format!("write shell css: {e}")))?;
        tracing::info!("wrote Shell CSS to {}", dir.display());
        Ok(())
    }

    fn clear_overrides(&self) -> Result<(), AppError> {
        let gtk_path = self.home.join(GTK_CSS_PATH);
        let backup = self.home.join(GTK_CSS_BACKUP);

        // Restore the user's original gtk.css if we backed it up
        if backup.exists() {
            std::fs::rename(&backup, &gtk_path).ok();
            tracing::info!("restored original gtk.css from backup");
        } else {
            let _ = std::fs::remove_file(&gtk_path);
        }

        let custom_dir = self.home.join(SHELL_THEME_BASE).join("GNOME-X-Custom");
        let _ = std::fs::remove_dir_all(custom_dir);
        tracing::info!("cleared theme overrides");
        Ok(())
    }
}
