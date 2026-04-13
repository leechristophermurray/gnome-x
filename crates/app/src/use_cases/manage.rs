// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::ports::{LocalInstaller, ShellProxy};
use crate::AppError;
use gnomex_domain::{Extension, ExtensionUuid};
use std::sync::Arc;

/// Use case: manage installed extensions (list, enable, disable, uninstall).
pub struct ManageUseCase {
    installer: Arc<dyn LocalInstaller>,
    shell: Arc<dyn ShellProxy>,
}

impl ManageUseCase {
    pub fn new(installer: Arc<dyn LocalInstaller>, shell: Arc<dyn ShellProxy>) -> Self {
        Self { installer, shell }
    }

    /// List all extensions known to GNOME Shell on this system.
    pub async fn list_installed_extensions(&self) -> Result<Vec<Extension>, AppError> {
        self.shell.list_extensions().await
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
}
