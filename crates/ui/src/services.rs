// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Application services — holds the tokio runtime handle and use cases.
//!
//! This struct is the bridge between the Composition Root (main.rs) and
//! the UI layer. It is threaded through component Init params so that
//! every view can spawn async work on the tokio runtime.

use gnomex_app::use_cases::{
    ApplyThemeUseCase, BrowseUseCase, CustomizeUseCase, ManageUseCase, PacksUseCase,
};
use gnomex_infra::{
    DbusShellProxy, EgoClient, FilesystemInstaller, FilesystemThemeWriter, GSettingsAppearance,
    OcsClient, PackTomlStorage,
};
use std::sync::Arc;
use tokio::runtime::Handle;

/// Shared application services passed to all UI components.
#[derive(Clone)]
pub struct AppServices {
    pub handle: Handle,
    pub browse: Arc<BrowseUseCase>,
    pub manage: Arc<ManageUseCase>,
    pub customize: Arc<CustomizeUseCase>,
    pub packs: Arc<PacksUseCase>,
    pub apply_theme: Arc<ApplyThemeUseCase>,
    pub detected_gnome_version: String,
}

impl AppServices {
    pub fn new(handle: Handle) -> Self {
        // --- Composition Root: wire concrete adapters to ports ---

        let ego_client: Arc<EgoClient> = Arc::new(EgoClient::new());
        let ocs_client: Arc<OcsClient> = Arc::new(OcsClient::new());
        let installer: Arc<FilesystemInstaller> = Arc::new(FilesystemInstaller::new());
        let appearance: Arc<GSettingsAppearance> = Arc::new(GSettingsAppearance::new());
        let pack_storage: Arc<PackTomlStorage> = Arc::new(PackTomlStorage::new());

        // D-Bus proxy requires an async connection.
        let shell_proxy: Arc<DbusShellProxy> = Arc::new(
            handle
                .block_on(DbusShellProxy::new())
                .expect("failed to connect to GNOME Shell D-Bus — is GNOME Shell running?"),
        );

        // Detect shell version for the theme CSS adapter factory.
        let shell_version = handle
            .block_on(async {
                use gnomex_app::ports::ShellProxy;
                shell_proxy.get_shell_version().await
            })
            .unwrap_or_else(|_| gnomex_domain::ShellVersion::new(47, 0));

        let css_generator: Arc<dyn gnomex_app::ports::ThemeCssGenerator> =
            Arc::from(gnomex_infra::theme_css::create_css_generator(&shell_version));
        let detected_gnome_version = css_generator.version_label().to_owned();

        let theme_writer: Arc<FilesystemThemeWriter> =
            Arc::new(FilesystemThemeWriter::new());

        let browse = Arc::new(BrowseUseCase::new(
            ego_client.clone(),
            shell_proxy.clone(),
        ));

        let manage = Arc::new(ManageUseCase::new(
            installer.clone(),
            shell_proxy.clone(),
            ego_client.clone(),
        ));

        let customize = Arc::new(CustomizeUseCase::new(
            ocs_client.clone(),
            installer.clone(),
            appearance.clone(),
        ));

        let packs = Arc::new(PacksUseCase::new(
            pack_storage,
            appearance.clone(),
            shell_proxy,
            installer,
            ocs_client,
        ));

        let apply_theme = Arc::new(ApplyThemeUseCase::new(
            css_generator,
            theme_writer,
            appearance,
        ));

        Self {
            handle,
            browse,
            manage,
            customize,
            packs,
            apply_theme,
            detected_gnome_version,
        }
    }
}
