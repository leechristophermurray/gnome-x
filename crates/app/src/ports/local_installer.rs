// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;
use gnomex_domain::{ExtensionUuid, ResourceKind, ShadowedResource, ThemeType};

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

    /// Enumerate every resource of the given [`ResourceKind`] that is
    /// installed in more than one search-path location — the caller
    /// can surface these so the user understands *which* of their
    /// themes/icons/cursors is actually winning at GTK lookup time.
    ///
    /// Returns an empty vector when no shadowing is detected. Length-1
    /// entries (not shadowed) are filtered out — every returned
    /// [`ShadowedResource`] has `locations.len() >= 2`.
    ///
    /// See GXF-012. The default implementation returns `Ok(vec![])`
    /// so existing test mocks of this trait keep compiling without
    /// needing to stub the method.
    fn list_shadowed_resources(
        &self,
        _kind: ResourceKind,
    ) -> Result<Vec<ShadowedResource>, AppError> {
        Ok(Vec::new())
    }
}
