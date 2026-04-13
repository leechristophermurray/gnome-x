// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;
use gnomex_domain::{Extension, ExtensionUuid, SearchResult, ShellVersion};

/// Port: search and download extensions from extensions.gnome.org.
#[async_trait::async_trait]
pub trait ExtensionRepository: Send + Sync {
    /// Search for extensions matching `query`, filtered by shell compatibility.
    async fn search(
        &self,
        query: &str,
        shell_version: &ShellVersion,
        page: u32,
    ) -> Result<SearchResult<Extension>, AppError>;

    /// Fetch full details for a single extension by UUID.
    async fn get_info(
        &self,
        uuid: &ExtensionUuid,
        shell_version: &ShellVersion,
    ) -> Result<Extension, AppError>;

    /// Download the extension zip archive.
    async fn download(
        &self,
        uuid: &ExtensionUuid,
        shell_version: &ShellVersion,
    ) -> Result<Vec<u8>, AppError>;
}
