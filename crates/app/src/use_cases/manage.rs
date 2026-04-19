// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::ports::{ExtensionRepository, LocalInstaller, ShellProxy};
use crate::AppError;
use gnomex_domain::{Extension, ExtensionUuid};
use std::sync::Arc;

/// Use case: manage installed extensions (list, enable, disable, uninstall).
pub struct ManageUseCase {
    installer: Arc<dyn LocalInstaller>,
    shell: Arc<dyn ShellProxy>,
    extension_repo: Arc<dyn ExtensionRepository>,
}

impl ManageUseCase {
    pub fn new(
        installer: Arc<dyn LocalInstaller>,
        shell: Arc<dyn ShellProxy>,
        extension_repo: Arc<dyn ExtensionRepository>,
    ) -> Self {
        Self {
            installer,
            shell,
            extension_repo,
        }
    }

    /// List all extensions known to GNOME Shell, enriched with EGO metadata
    /// (creator, screenshot URL) where available.
    pub async fn list_installed_extensions(&self) -> Result<Vec<Extension>, AppError> {
        let mut extensions = self.shell.list_extensions().await?;
        let shell_version = self.shell.get_shell_version().await?;

        // Enrich each extension with EGO data (best-effort, don't fail on individual lookups)
        for ext in &mut extensions {
            if ext.creator.is_empty() || ext.screenshot_url.is_none() {
                if let Ok(info) = self
                    .extension_repo
                    .get_info(&ext.uuid, &shell_version)
                    .await
                {
                    if ext.creator.is_empty() {
                        ext.creator = info.creator;
                    }
                    if ext.screenshot_url.is_none() {
                        ext.screenshot_url = info.screenshot_url;
                    }
                    if ext.homepage_url.is_none() {
                        ext.homepage_url = info.homepage_url;
                    }
                }
            }
        }

        Ok(extensions)
    }

    /// Toggle an extension on or off.
    pub async fn toggle_extension(
        &self,
        uuid: &ExtensionUuid,
        enabled: bool,
    ) -> Result<(), AppError> {
        if enabled {
            self.shell.enable_extension(uuid).await
        } else {
            self.shell.disable_extension(uuid).await
        }
    }

    /// Remove an extension from the system.
    pub async fn uninstall_extension(&self, uuid: &ExtensionUuid) -> Result<(), AppError> {
        self.shell.disable_extension(uuid).await.ok();
        self.installer.uninstall_extension(uuid).await
    }

    /// Open the preferences window for an extension (if it has one).
    pub async fn open_extension_prefs(&self, uuid: &ExtensionUuid) -> Result<(), AppError> {
        self.shell.open_extension_prefs(uuid).await
    }

    /// List installed themes on the filesystem.
    pub fn list_installed_themes(&self) -> Result<Vec<String>, AppError> {
        self.installer.list_installed_themes()
    }

    /// List installed icon packs on the filesystem.
    pub fn list_installed_icons(&self) -> Result<Vec<String>, AppError> {
        self.installer.list_installed_icons()
    }

    /// List installed cursor themes on the filesystem.
    pub fn list_installed_cursors(&self) -> Result<Vec<String>, AppError> {
        self.installer.list_installed_cursors()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{MockExtensionRepo, MockInstaller, MockShellProxy};
    use gnomex_domain::ShellVersion;

    fn uuid() -> ExtensionUuid {
        ExtensionUuid::new("dash-to-dock@micxgx.gmail.com").unwrap()
    }

    #[tokio::test]
    async fn toggle_extension_true_enables_via_shell() {
        let installer = MockInstaller::new();
        let shell = MockShellProxy::new(ShellVersion::new(47, 0));
        let repo = MockExtensionRepo::new();
        let uc = ManageUseCase::new(installer, shell.clone(), repo);

        uc.toggle_extension(&uuid(), true).await.unwrap();

        assert_eq!(
            shell.enabled.lock().unwrap().as_slice(),
            &["dash-to-dock@micxgx.gmail.com".to_owned()]
        );
    }

    #[tokio::test]
    async fn toggle_extension_false_does_not_enable() {
        let installer = MockInstaller::new();
        let shell = MockShellProxy::new(ShellVersion::new(47, 0));
        let repo = MockExtensionRepo::new();
        let uc = ManageUseCase::new(installer, shell.clone(), repo);

        uc.toggle_extension(&uuid(), false).await.unwrap();

        assert!(shell.enabled.lock().unwrap().is_empty());
    }

    #[test]
    fn list_installed_lists_forward_to_installer() {
        let installer = MockInstaller::new();
        installer.themes.lock().unwrap().push("Adwaita".into());
        installer.icons.lock().unwrap().push("Papirus".into());
        installer.cursors.lock().unwrap().push("Bibata".into());

        let shell = MockShellProxy::new(ShellVersion::new(47, 0));
        let repo = MockExtensionRepo::new();
        let uc = ManageUseCase::new(installer, shell, repo);

        assert_eq!(uc.list_installed_themes().unwrap(), vec!["Adwaita"]);
        assert_eq!(uc.list_installed_icons().unwrap(), vec!["Papirus"]);
        assert_eq!(uc.list_installed_cursors().unwrap(), vec!["Bibata"]);
    }
}
