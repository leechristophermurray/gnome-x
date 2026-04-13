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
}
