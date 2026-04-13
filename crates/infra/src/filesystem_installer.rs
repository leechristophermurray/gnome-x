// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use gnomex_app::ports::LocalInstaller;
use gnomex_app::AppError;
use gnomex_domain::{ExtensionUuid, ThemeType};
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// Concrete adapter: install/uninstall content to the local filesystem.
///
/// Respects XDG directories:
/// - Extensions: `~/.local/share/gnome-shell/extensions/<uuid>/`
/// - Themes: `~/.local/share/themes/<name>/`
/// - Icons/Cursors: `~/.local/share/icons/<name>/`
pub struct FilesystemInstaller {
    data_dir: PathBuf,
    home_dir: PathBuf,
}

impl FilesystemInstaller {
    pub fn new() -> Self {
        let data_dir = directories::BaseDirs::new()
            .map(|d| d.data_local_dir().to_owned())
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                PathBuf::from(home).join(".local/share")
            });

        let home_dir = directories::BaseDirs::new()
            .map(|d| d.home_dir().to_owned())
            .unwrap_or_else(|| PathBuf::from("/tmp"));

        Self { data_dir, home_dir }
    }

    fn extensions_dir(&self) -> PathBuf {
        self.data_dir.join("gnome-shell/extensions")
    }

    fn themes_dir(&self) -> PathBuf {
        self.data_dir.join("themes")
    }

    fn icons_dir(&self) -> PathBuf {
        self.data_dir.join("icons")
    }

    fn list_subdirs(path: &Path) -> Result<Vec<String>, AppError> {
        if !path.exists() {
            return Ok(vec![]);
        }
        let entries = std::fs::read_dir(path)
            .map_err(|e| AppError::Install(format!("failed to read {}: {e}", path.display())))?;

        let mut names = Vec::new();
        for entry in entries.flatten() {
            if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_owned());
                }
            }
        }
        Ok(names)
    }
}

#[async_trait::async_trait]
impl LocalInstaller for FilesystemInstaller {
    async fn install_extension(
        &self,
        uuid: &ExtensionUuid,
        zip_data: &[u8],
    ) -> Result<(), AppError> {
        let dest = self.extensions_dir().join(uuid.as_str());
        tokio::fs::create_dir_all(&dest)
            .await
            .map_err(|e| AppError::Install(format!("mkdir failed: {e}")))?;

        let cursor = Cursor::new(zip_data.to_vec());
        let dest_clone = dest.clone();
        tokio::task::spawn_blocking(move || {
            let mut archive = zip::ZipArchive::new(cursor)
                .map_err(|e| AppError::Install(format!("invalid zip: {e}")))?;
            archive
                .extract(&dest_clone)
                .map_err(|e| AppError::Install(format!("extract failed: {e}")))?;
            Ok::<_, AppError>(())
        })
        .await
        .map_err(|e| AppError::Install(format!("task join error: {e}")))??;

        tracing::info!("installed extension {} to {}", uuid, dest.display());
        Ok(())
    }

    async fn uninstall_extension(&self, uuid: &ExtensionUuid) -> Result<(), AppError> {
        let dir = self.extensions_dir().join(uuid.as_str());
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir)
                .await
                .map_err(|e| AppError::Install(format!("remove failed: {e}")))?;
            tracing::info!("uninstalled extension {uuid}");
        }
        Ok(())
    }

    async fn install_theme(
        &self,
        name: &str,
        archive_data: &[u8],
        _kind: ThemeType,
    ) -> Result<(), AppError> {
        let dest = self.themes_dir().join(name);
        extract_tar_gz(archive_data, &dest).await
    }

    async fn uninstall_theme(&self, name: &str) -> Result<(), AppError> {
        remove_dir_if_exists(&self.themes_dir().join(name)).await
    }

    async fn install_icon_pack(&self, name: &str, archive_data: &[u8]) -> Result<(), AppError> {
        let dest = self.icons_dir().join(name);
        extract_tar_gz(archive_data, &dest).await
    }

    async fn install_cursor(&self, name: &str, archive_data: &[u8]) -> Result<(), AppError> {
        let dest = self.icons_dir().join(name);
        extract_tar_gz(archive_data, &dest).await
    }

    fn list_installed_extensions(&self) -> Result<Vec<String>, AppError> {
        Self::list_subdirs(&self.extensions_dir())
    }

    fn list_installed_themes(&self) -> Result<Vec<String>, AppError> {
        let mut themes = Self::list_subdirs(&self.themes_dir())?;
        // Also check ~/.themes for legacy installations
        let legacy = self.home_dir.join(".themes");
        if legacy.exists() {
            themes.extend(Self::list_subdirs(&legacy)?);
        }
        themes.sort();
        themes.dedup();
        Ok(themes)
    }

    fn list_installed_icons(&self) -> Result<Vec<String>, AppError> {
        let mut icons = Self::list_subdirs(&self.icons_dir())?;
        let legacy = self.home_dir.join(".icons");
        if legacy.exists() {
            icons.extend(Self::list_subdirs(&legacy)?);
        }
        icons.sort();
        icons.dedup();
        Ok(icons)
    }

    fn list_installed_cursors(&self) -> Result<Vec<String>, AppError> {
        // Cursors live alongside icons; filter for those containing a "cursors" subdir
        let icons = self.list_installed_icons()?;
        let base_dirs = [self.icons_dir(), self.home_dir.join(".icons")];
        Ok(icons
            .into_iter()
            .filter(|name| {
                base_dirs
                    .iter()
                    .any(|base| base.join(name).join("cursors").is_dir())
            })
            .collect())
    }
}

async fn extract_tar_gz(data: &[u8], dest: &Path) -> Result<(), AppError> {
    tokio::fs::create_dir_all(dest)
        .await
        .map_err(|e| AppError::Install(format!("mkdir failed: {e}")))?;

    let data = data.to_vec();
    let dest = dest.to_owned();
    tokio::task::spawn_blocking(move || {
        let decoder = flate2::read::GzDecoder::new(Cursor::new(data));
        let mut archive = tar::Archive::new(decoder);
        archive
            .unpack(&dest)
            .map_err(|e| AppError::Install(format!("extract tar.gz failed: {e}")))?;
        Ok::<_, AppError>(())
    })
    .await
    .map_err(|e| AppError::Install(format!("task join error: {e}")))??;

    Ok(())
}

async fn remove_dir_if_exists(path: &Path) -> Result<(), AppError> {
    if path.exists() {
        tokio::fs::remove_dir_all(path)
            .await
            .map_err(|e| AppError::Install(format!("remove failed: {e}")))?;
    }
    Ok(())
}
