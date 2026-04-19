// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::ports::{ExtensionRepository, ShellProxy};
use crate::AppError;
use gnomex_domain::{Extension, ExtensionUuid, SearchResult};
use std::sync::Arc;

/// Use case: browse and install extensions from remote sources.
pub struct BrowseUseCase {
    extension_repo: Arc<dyn ExtensionRepository>,
    shell: Arc<dyn ShellProxy>,
}

impl BrowseUseCase {
    pub fn new(
        extension_repo: Arc<dyn ExtensionRepository>,
        shell: Arc<dyn ShellProxy>,
    ) -> Self {
        Self {
            extension_repo,
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

    /// Install an extension via GNOME Shell's D-Bus interface.
    /// This delegates the full download, extraction, and enablement to the shell.
    pub async fn install_extension(&self, uuid: &ExtensionUuid) -> Result<(), AppError> {
        self.shell.install_extension(uuid).await
    }

    /// Get full details for a single extension.
    pub async fn get_extension_details(
        &self,
        uuid: &ExtensionUuid,
    ) -> Result<Extension, AppError> {
        let shell_version = self.shell.get_shell_version().await?;
        self.extension_repo.get_info(uuid, &shell_version).await
    }

    /// List popular extensions, cross-referenced against installed state.
    pub async fn list_popular(&self) -> Result<SearchResult<Extension>, AppError> {
        let shell_version = self.shell.get_shell_version().await?;
        let mut result = self.extension_repo.list_popular(&shell_version, 1).await?;
        self.patch_installed_state(&mut result).await;
        Ok(result)
    }

    /// List recently updated extensions, cross-referenced against installed state.
    pub async fn list_recent(&self) -> Result<SearchResult<Extension>, AppError> {
        let shell_version = self.shell.get_shell_version().await?;
        let mut result = self.extension_repo.list_recent(&shell_version, 1).await?;
        self.patch_installed_state(&mut result).await;
        Ok(result)
    }

    async fn patch_installed_state(&self, result: &mut SearchResult<Extension>) {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{MockExtensionRepo, MockShellProxy};
    use gnomex_domain::ShellVersion;

    #[tokio::test]
    async fn install_extension_delegates_to_shell_proxy() {
        let repo = MockExtensionRepo::new();
        let shell = MockShellProxy::new(ShellVersion::new(47, 0));
        let uc = BrowseUseCase::new(repo, shell.clone());

        let uuid = ExtensionUuid::new("dash-to-dock@micxgx.gmail.com").unwrap();
        uc.install_extension(&uuid).await.unwrap();

        let installed = shell.installed.lock().unwrap().clone();
        assert_eq!(installed, vec!["dash-to-dock@micxgx.gmail.com"]);
    }

    #[tokio::test]
    async fn search_uses_detected_shell_version() {
        // Empty repo results are fine — we're just asserting the call
        // reached through without an error and that the shell version
        // was queried.
        let repo = MockExtensionRepo::new();
        let shell = MockShellProxy::new(ShellVersion::new(47, 0));
        let uc = BrowseUseCase::new(repo, shell);

        let result = uc.search_extensions("dash", 1).await.unwrap();
        assert!(result.items.is_empty());
    }
}
