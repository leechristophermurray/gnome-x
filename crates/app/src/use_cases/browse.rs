// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::ports::{ExtensionRepository, LocalInstaller, ShellProxy};
use crate::AppError;
use gnomex_domain::{Extension, ExtensionUuid, SearchResult};
use std::sync::Arc;

/// Use case: browse and install extensions from remote sources.
pub struct BrowseUseCase {
    extension_repo: Arc<dyn ExtensionRepository>,
    installer: Arc<dyn LocalInstaller>,
    shell: Arc<dyn ShellProxy>,
}

impl BrowseUseCase {
    pub fn new(
        extension_repo: Arc<dyn ExtensionRepository>,
        installer: Arc<dyn LocalInstaller>,
        shell: Arc<dyn ShellProxy>,
    ) -> Self {
        Self {
            extension_repo,
            installer,
            shell,
        }
    }

    /// Search for extensions by query string.
    ///
    /// Results are cross-referenced against locally installed extensions so
    /// that each item carries its real on-disk state (Enabled / Disabled / …)
    /// rather than the blanket `Available` the repository returns.
    pub async fn search_extensions(
        &self,
        query: &str,
        page: u32,
    ) -> Result<SearchResult<Extension>, AppError> {
        let shell_version = self.shell.get_shell_version().await?;
        let mut result = self
            .extension_repo
            .search(query, &shell_version, page)
            .await?;

        // Build a lookup of locally-known extension states.
        if let Ok(installed) = self.shell.list_extensions().await {
            let state_map: std::collections::HashMap<_, _> = installed
                .into_iter()
                .map(|e| (e.uuid.clone(), e.state))
                .collect();

            for ext in &mut result.items {
                if let Some(&state) = state_map.get(&ext.uuid) {
                    ext.state = state;
                }
            }
        }

        Ok(result)
    }

    /// Download and install an extension, then enable it.
    pub async fn install_extension(&self, uuid: &ExtensionUuid) -> Result<(), AppError> {
        let shell_version = self.shell.get_shell_version().await?;
        let zip_data = self.extension_repo.download(uuid, &shell_version).await?;
        self.installer.install_extension(uuid, &zip_data).await?;
        self.shell.enable_extension(uuid).await?;
        Ok(())
    }

    /// Get full details for a single extension.
    pub async fn get_extension_details(
        &self,
        uuid: &ExtensionUuid,
    ) -> Result<Extension, AppError> {
        let shell_version = self.shell.get_shell_version().await?;
        self.extension_repo.get_info(uuid, &shell_version).await
    }
}
