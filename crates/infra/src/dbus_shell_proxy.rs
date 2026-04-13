// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use gnomex_app::ports::ShellProxy;
use gnomex_app::AppError;
use gnomex_domain::{Extension, ExtensionState, ExtensionUuid, ShellVersion};
use std::collections::HashMap;
use zbus::zvariant::OwnedValue;
use zbus::Connection;

/// Concrete adapter: GNOME Shell D-Bus proxy.
///
/// Communicates with `org.gnome.Shell.Extensions` (GNOME 45+) to list,
/// enable, and disable extensions.
pub struct DbusShellProxy {
    connection: Connection,
}

impl DbusShellProxy {
    pub async fn new() -> Result<Self, AppError> {
        let connection = Connection::session()
            .await
            .map_err(|e| AppError::Shell(format!("failed to connect to session bus: {e}")))?;
        Ok(Self { connection })
    }
}

/// Extract a string value from a D-Bus property map.
fn get_string_prop(props: &HashMap<String, OwnedValue>, key: &str) -> Option<String> {
    props
        .get(key)
        .and_then(|v| <&str>::try_from(v).ok())
        .map(|s| s.to_owned())
}

/// Extract a numeric value from a D-Bus property map.
fn get_f64_prop(props: &HashMap<String, OwnedValue>, key: &str) -> Option<f64> {
    props.get(key).and_then(|v| {
        f64::try_from(v)
            .or_else(|_| i32::try_from(v).map(|i| i as f64))
            .or_else(|_| u32::try_from(v).map(|u| u as f64))
            .ok()
    })
}

#[async_trait::async_trait]
impl ShellProxy for DbusShellProxy {
    async fn get_shell_version(&self) -> Result<ShellVersion, AppError> {
        let shell_proxy = ShellExtensionsProxy::new(&self.connection)
            .await
            .map_err(|e| AppError::Shell(e.to_string()))?;

        let version_str = shell_proxy
            .shell_version()
            .await
            .map_err(|e| AppError::Shell(e.to_string()))?;

        ShellVersion::parse(&version_str).map_err(|e| AppError::Shell(e.to_string()))
    }

    async fn list_extensions(&self) -> Result<Vec<Extension>, AppError> {
        let proxy = ShellExtensionsProxy::new(&self.connection)
            .await
            .map_err(|e| AppError::Shell(e.to_string()))?;

        let extensions_map: HashMap<String, HashMap<String, OwnedValue>> = proxy
            .list_extensions()
            .await
            .map_err(|e| AppError::Shell(e.to_string()))?;

        let mut result = Vec::new();
        for (uuid_str, props) in &extensions_map {
            let uuid = match ExtensionUuid::new(uuid_str) {
                Ok(u) => u,
                Err(_) => continue,
            };

            let name = get_string_prop(props, "name").unwrap_or_else(|| uuid_str.clone());
            let description = get_string_prop(props, "description").unwrap_or_default();
            let state_num = get_f64_prop(props, "state").unwrap_or(0.0) as u32;

            let state = match state_num {
                1 => ExtensionState::Enabled,
                2 => ExtensionState::Disabled,
                3 => ExtensionState::Error,
                _ => ExtensionState::Installed,
            };

            let version = get_f64_prop(props, "version").unwrap_or(0.0) as u32;

            let creator = get_string_prop(props, "original-author").unwrap_or_default();
            let homepage_url = get_string_prop(props, "url");

            result.push(Extension {
                uuid,
                name,
                description,
                creator,
                shell_versions: vec![],
                version,
                download_url: None,
                screenshot_url: None,
                homepage_url,
                pk: None,
                state,
            });
        }

        Ok(result)
    }

    async fn install_extension(&self, uuid: &ExtensionUuid) -> Result<(), AppError> {
        let proxy = ShellExtensionsProxy::new(&self.connection)
            .await
            .map_err(|e| AppError::Shell(e.to_string()))?;

        let result = proxy
            .install_remote_extension(uuid.as_str())
            .await
            .map_err(|e| AppError::Shell(format!("install via D-Bus failed: {e}")))?;

        tracing::info!("InstallRemoteExtension({uuid}) returned: {result}");
        Ok(())
    }

    async fn enable_extension(&self, uuid: &ExtensionUuid) -> Result<(), AppError> {
        let proxy = ShellExtensionsProxy::new(&self.connection)
            .await
            .map_err(|e| AppError::Shell(e.to_string()))?;

        proxy
            .enable_extension(uuid.as_str())
            .await
            .map_err(|e| AppError::Shell(e.to_string()))?;

        Ok(())
    }

    async fn disable_extension(&self, uuid: &ExtensionUuid) -> Result<(), AppError> {
        let proxy = ShellExtensionsProxy::new(&self.connection)
            .await
            .map_err(|e| AppError::Shell(e.to_string()))?;

        proxy
            .disable_extension(uuid.as_str())
            .await
            .map_err(|e| AppError::Shell(e.to_string()))?;

        Ok(())
    }

    async fn open_extension_prefs(&self, uuid: &ExtensionUuid) -> Result<(), AppError> {
        let proxy = ShellExtensionsProxy::new(&self.connection)
            .await
            .map_err(|e| AppError::Shell(e.to_string()))?;

        proxy
            .open_extension_prefs(uuid.as_str(), "", HashMap::<String, OwnedValue>::new())
            .await
            .map_err(|e| AppError::Shell(e.to_string()))?;

        Ok(())
    }
}

/// Generated D-Bus proxy for org.gnome.Shell.Extensions (GNOME 45+).
#[zbus::proxy(
    interface = "org.gnome.Shell.Extensions",
    default_service = "org.gnome.Shell.Extensions",
    default_path = "/org/gnome/Shell/Extensions"
)]
trait ShellExtensions {
    fn list_extensions(&self) -> zbus::Result<HashMap<String, HashMap<String, OwnedValue>>>;

    fn install_remote_extension(&self, uuid: &str) -> zbus::Result<String>;

    fn enable_extension(&self, uuid: &str) -> zbus::Result<bool>;

    fn disable_extension(&self, uuid: &str) -> zbus::Result<bool>;

    fn open_extension_prefs(
        &self,
        uuid: &str,
        parent_window: &str,
        options: HashMap<String, OwnedValue>,
    ) -> zbus::Result<()>;

    #[zbus(property)]
    fn shell_version(&self) -> zbus::Result<String>;
}
