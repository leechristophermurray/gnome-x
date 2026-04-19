// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;
use gnomex_domain::{ExtensionUuid, ThemeType};

/// Port: install and uninstall content on the local filesystem.
#[async_trait::async_trait]
pub trait LocalInstaller: Send + Sync {
    async fn install_extension(
        &self,
        uuid: &ExtensionUuid,
        zip_data: &[u8],
    ) -> Result<(), AppError>;

    async fn uninstall_extension(&self, uuid: &ExtensionUuid) -> Result<(), AppError>;

    async fn install_theme(
        &self,
        name: &str,
        archive_data: &[u8],
        kind: ThemeType,
    ) -> Result<(), AppError>;

    async fn uninstall_theme(&self, name: &str) -> Result<(), AppError>;

    async fn install_icon_pack(&self, name: &str, archive_data: &[u8]) -> Result<(), AppError>;

    async fn install_cursor(&self, name: &str, archive_data: &[u8]) -> Result<(), AppError>;

    fn list_installed_extensions(&self) -> Result<Vec<String>, AppError>;

    fn list_installed_themes(&self) -> Result<Vec<String>, AppError>;

    fn list_installed_icons(&self) -> Result<Vec<String>, AppError>;

    fn list_installed_cursors(&self) -> Result<Vec<String>, AppError>;

    /// Find resources of a given kind whose name exists in more than
    /// one search path. GTK resolves first-hit-wins, so later entries
    /// are *shadowed* — invisible until the winning copy is removed.
    ///
    /// Returns only actually-shadowed names; non-conflicting
    /// resources are filtered out. Resolves GXF-012.
    fn list_shadowed_resources(
        &self,
        kind: ResourceKind,
    ) -> Result<Vec<ShadowedResource>, AppError>;
}

/// Kind of filesystem resource, for the `list_shadowed_resources`
/// query.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceKind {
    Theme,
    Icon,
    Cursor,
}

/// A resource name that exists in multiple search paths, with the
/// paths in resolution order (first entry wins, rest are masked).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowedResource {
    pub name: String,
    pub locations: Vec<ShadowedLocation>,
}

/// One search-path entry a [`ShadowedResource`] was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowedLocation {
    /// Absolute filesystem path, e.g. `/home/user/.themes/Adwaita`.
    pub path: String,
    /// Whether GNOME X could write here without root privileges.
    pub user_writable: bool,
}
