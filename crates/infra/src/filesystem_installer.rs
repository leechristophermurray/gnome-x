// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::theme_paths::{list_all, shadow_map, ResourcePaths, SearchPath};
use gnomex_app::ports::LocalInstaller;
use gnomex_app::AppError;
use gnomex_domain::{
    ExtensionUuid, ResourceKind, ShadowedLocation, ShadowedResource, ThemeType,
};
use std::io::Cursor;
use std::path::{Path, PathBuf};

/// Concrete adapter: install/uninstall content to the local filesystem.
///
/// Install targets (always user-writable):
/// - Extensions: `$XDG_DATA_HOME/gnome-shell/extensions/<uuid>/`
/// - Themes: `$XDG_DATA_HOME/themes/<name>/`
/// - Icons/Cursors: `$XDG_DATA_HOME/icons/<name>/`
///
/// List operations walk every path that GNOME searches, not just the
/// install target — themes under `~/.themes` (legacy), system themes
/// under `/usr/share/themes`, and all `$XDG_DATA_DIRS` entries. See
/// [`theme_paths`](crate::theme_paths) for the full resolution model.
pub struct FilesystemInstaller {
    data_dir: PathBuf,
    paths: ResourcePaths,
}

impl FilesystemInstaller {
    pub fn new() -> Self {
        let data_dir = directories::BaseDirs::new()
            .map(|d| d.data_local_dir().to_owned())
            .unwrap_or_else(|| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
                PathBuf::from(home).join(".local/share")
            });

        Self {
            data_dir,
            paths: ResourcePaths::from_env(),
        }
    }

    /// Hermetic constructor for tests — wire an explicit `data_dir`
    /// (where new installs land) and a fully-formed [`ResourcePaths`]
    /// (what gets walked for listings / shadow detection). Lets
    /// tests run in parallel without racing `$HOME` / `$XDG_*`.
    pub fn with_paths(data_dir: PathBuf, paths: ResourcePaths) -> Self {
        Self { data_dir, paths }
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
        extract_archive(archive_data, &dest).await
    }

    async fn uninstall_theme(&self, name: &str) -> Result<(), AppError> {
        remove_dir_if_exists(&self.themes_dir().join(name)).await
    }

    async fn install_icon_pack(&self, name: &str, archive_data: &[u8]) -> Result<(), AppError> {
        let dest = self.icons_dir().join(name);
        extract_archive(archive_data, &dest).await
    }

    async fn install_cursor(&self, name: &str, archive_data: &[u8]) -> Result<(), AppError> {
        let dest = self.icons_dir().join(name);
        extract_archive(archive_data, &dest).await
    }

    fn list_installed_extensions(&self) -> Result<Vec<String>, AppError> {
        Self::list_subdirs(&self.extensions_dir())
    }

    fn list_installed_themes(&self) -> Result<Vec<String>, AppError> {
        // Walks XDG_DATA_HOME + ~/.themes + every XDG_DATA_DIRS entry.
        Ok(list_all(&self.paths.themes()))
    }

    fn list_installed_icons(&self) -> Result<Vec<String>, AppError> {
        Ok(list_all(&self.paths.icons()))
    }

    fn list_installed_cursors(&self) -> Result<Vec<String>, AppError> {
        // Cursors live inside icon-theme dirs but only count when the
        // theme has a `cursors/` subdir — otherwise it's just icons.
        let mut cursors: Vec<String> = self
            .paths
            .cursors()
            .iter()
            .flat_map(|sp| {
                crate::theme_paths::list_subdirs(&sp.path)
                    .into_iter()
                    .filter(move |name| sp.path.join(name).join("cursors").is_dir())
            })
            .collect();
        cursors.sort();
        cursors.dedup();
        Ok(cursors)
    }

    fn list_shadowed_resources(
        &self,
        kind: ResourceKind,
    ) -> Result<Vec<ShadowedResource>, AppError> {
        let search = match kind {
            ResourceKind::Theme => self.paths.themes(),
            ResourceKind::Icon => self.paths.icons(),
            ResourceKind::Cursor => self.paths.cursors(),
        };
        let map = shadow_map(&search);
        let mut out = Vec::new();
        for (name, locs) in map {
            if locs.len() < 2 {
                continue;
            }
            // For cursors, discard names whose directory doesn't have
            // a `cursors/` subdir in *any* location — those are
            // icon-only entries we don't want to confuse the cursor
            // report with.
            if kind == ResourceKind::Cursor
                && !locs
                    .iter()
                    .any(|sp: &SearchPath| sp.path.join(&name).join("cursors").is_dir())
            {
                continue;
            }
            // shadow_map returns the *parent* directory (e.g.
            // `/usr/share/themes`); join the resource name onto each
            // so the UI can surface the absolute path to the theme
            // itself.
            let locations = locs
                .into_iter()
                .map(|sp| ShadowedLocation {
                    path: sp.path.join(&name),
                    user_writable: sp.origin.is_user_writable(),
                })
                .collect();
            out.push(ShadowedResource {
                kind,
                name,
                locations,
            });
        }
        Ok(out)
    }
}

/// Detect archive format by magic bytes and extract accordingly.
async fn extract_archive(data: &[u8], dest: &Path) -> Result<(), AppError> {
    tokio::fs::create_dir_all(dest)
        .await
        .map_err(|e| AppError::Install(format!("mkdir failed: {e}")))?;

    let data = data.to_vec();
    let dest = dest.to_owned();
    tokio::task::spawn_blocking(move || {
        let format = detect_format(&data);
        tracing::debug!("archive format: {format:?}, size: {} bytes", data.len());

        match format {
            ArchiveFormat::Zip => {
                let cursor = Cursor::new(data);
                let mut archive = zip::ZipArchive::new(cursor)
                    .map_err(|e| AppError::Install(format!("invalid zip: {e}")))?;
                archive
                    .extract(&dest)
                    .map_err(|e| AppError::Install(format!("extract zip failed: {e}")))?;
            }
            ArchiveFormat::TarGz => {
                let decoder = flate2::read::GzDecoder::new(Cursor::new(data));
                let mut archive = tar::Archive::new(decoder);
                archive
                    .unpack(&dest)
                    .map_err(|e| AppError::Install(format!("extract tar.gz failed: {e}")))?;
            }
            ArchiveFormat::TarXz => {
                let decoder = xz2::read::XzDecoder::new(Cursor::new(data));
                let mut archive = tar::Archive::new(decoder);
                archive
                    .unpack(&dest)
                    .map_err(|e| AppError::Install(format!("extract tar.xz failed: {e}")))?;
            }
            ArchiveFormat::TarZstd => {
                let decoder = zstd::stream::read::Decoder::new(Cursor::new(data))
                    .map_err(|e| AppError::Install(format!("invalid zstd: {e}")))?;
                let mut archive = tar::Archive::new(decoder);
                archive
                    .unpack(&dest)
                    .map_err(|e| AppError::Install(format!("extract tar.zst failed: {e}")))?;
            }
            ArchiveFormat::Unknown => {
                return Err(AppError::Install(
                    "unrecognized archive format (expected zip, tar.gz, tar.xz, or tar.zst)".into(),
                ));
            }
        }
        Ok::<_, AppError>(())
    })
    .await
    .map_err(|e| AppError::Install(format!("task join error: {e}")))??;

    Ok(())
}

#[derive(Debug)]
enum ArchiveFormat {
    Zip,
    TarGz,
    TarXz,
    TarZstd,
    Unknown,
}

fn detect_format(data: &[u8]) -> ArchiveFormat {
    if data.len() < 6 {
        return ArchiveFormat::Unknown;
    }
    // ZIP: starts with PK\x03\x04
    if data[0..4] == [0x50, 0x4B, 0x03, 0x04] {
        return ArchiveFormat::Zip;
    }
    // GZ: starts with \x1F\x8B
    if data[0..2] == [0x1F, 0x8B] {
        return ArchiveFormat::TarGz;
    }
    // XZ: starts with \xFD7zXZ\x00
    if data[0..6] == [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00] {
        return ArchiveFormat::TarXz;
    }
    // Zstandard: starts with \x28\xB5\x2F\xFD
    if data[0..4] == [0x28, 0xB5, 0x2F, 0xFD] {
        return ArchiveFormat::TarZstd;
    }
    ArchiveFormat::Unknown
}

async fn remove_dir_if_exists(path: &Path) -> Result<(), AppError> {
    if path.exists() {
        tokio::fs::remove_dir_all(path)
            .await
            .map_err(|e| AppError::Install(format!("remove failed: {e}")))?;
    }
    Ok(())
}
