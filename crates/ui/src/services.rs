// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Application services — holds the tokio runtime handle and use cases.
//!
//! This struct is the bridge between the Composition Root (main.rs) and
//! the UI layer. It is threaded through component Init params so that
//! every view can spawn async work on the tokio runtime.

use gnomex_app::ports::{
    BlurMyShellController, FloatingDockController, ShellCustomizer, ThemingConflictDetector,
};
use gnomex_app::use_cases::{
    ApplyThemeUseCase, BrowseUseCase, CustomizeShellUseCase, CustomizeUseCase, ManageUseCase,
    PacksUseCase,
};
use gnomex_infra::{
    ChromiumThemer, DbusShellProxy, EgoClient, FilesystemInstaller, FilesystemThemeWriter,
    GSettingsAppSettings, GSettingsAppearance, GSettingsBlurMyShell, GSettingsFloatingDock,
    GioThemingConflictDetector, OcsClient, PackTomlStorage, VscodeThemer,
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
    pub customize_shell: Arc<CustomizeShellUseCase>,
    pub floating_dock: Arc<dyn FloatingDockController>,
    pub blur_my_shell: Arc<dyn BlurMyShellController>,
    pub conflict_detector: Arc<dyn ThemingConflictDetector>,
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

        let app_settings: Arc<GSettingsAppSettings> = Arc::new(GSettingsAppSettings::new());
        let floating_dock: Arc<dyn FloatingDockController> =
            Arc::new(GSettingsFloatingDock::new());
        let blur_my_shell: Arc<dyn BlurMyShellController> =
            Arc::new(GSettingsBlurMyShell::new());

        let shell_customizer: Arc<dyn ShellCustomizer> =
            Arc::from(gnomex_infra::shell_customizer::create_shell_customizer(
                &shell_version,
                gnomex_infra::shell_customizer::ExtensionControllers {
                    floating_dock: floating_dock.clone(),
                    blur_my_shell: blur_my_shell.clone(),
                },
            ));
        let customize_shell = Arc::new(CustomizeShellUseCase::new(
            shell_customizer.clone(),
            &shell_version,
        ));

        let packs = Arc::new(PacksUseCase::new(
            pack_storage,
            appearance.clone(),
            shell_proxy,
            installer,
            ocs_client,
            app_settings,
            shell_customizer,
        ));

        let apply_theme = Arc::new(
            ApplyThemeUseCase::new(css_generator, theme_writer, appearance)
                .with_external_themer(Arc::new(VscodeThemer::new()))
                .with_external_themer(Arc::new(ChromiumThemer::new())),
        );

        let conflict_detector: Arc<dyn ThemingConflictDetector> =
            Arc::new(GioThemingConflictDetector::new());

        Self {
            handle,
            browse,
            manage,
            customize,
            packs,
            apply_theme,
            customize_shell,
            floating_dock,
            blur_my_shell,
            conflict_detector,
            detected_gnome_version,
        }
    }
}
