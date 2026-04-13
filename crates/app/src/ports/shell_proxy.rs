// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;
use gnomex_domain::{Extension, ExtensionUuid, ShellVersion};

/// Port: interact with GNOME Shell via D-Bus.
#[async_trait::async_trait]
pub trait ShellProxy: Send + Sync {
    /// Get the running GNOME Shell version.
    async fn get_shell_version(&self) -> Result<ShellVersion, AppError>;

    /// List all extensions known to GNOME Shell (installed on this system).
    async fn list_extensions(&self) -> Result<Vec<Extension>, AppError>;

    /// Enable a shell extension.
    async fn enable_extension(&self, uuid: &ExtensionUuid) -> Result<(), AppError>;

    /// Disable a shell extension.
    async fn disable_extension(&self, uuid: &ExtensionUuid) -> Result<(), AppError>;

    /// Open the preferences window for a shell extension.
    async fn open_extension_prefs(&self, uuid: &ExtensionUuid) -> Result<(), AppError>;
}
