// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Application services — holds the tokio runtime handle and use cases.
//!
//! This struct is the bridge between the Composition Root (main.rs) and
//! the UI layer. It is threaded through component Init params so that
//! every view can spawn async work on the tokio runtime.

use gnomex_app::use_cases::{BrowseUseCase, ManageUseCase};
use gnomex_infra::{DbusShellProxy, EgoClient, FilesystemInstaller};
use std::sync::Arc;
use tokio::runtime::Handle;

/// Shared application services passed to all UI components.
#[derive(Clone)]
pub struct AppServices {
    pub handle: Handle,
    pub browse: Arc<BrowseUseCase>,
    pub manage: Arc<ManageUseCase>,
}

impl AppServices {
    pub fn new(handle: Handle) -> Self {
        // --- Composition Root: wire concrete adapters to ports ---

        let ego_client: Arc<EgoClient> = Arc::new(EgoClient::new());
        let installer: Arc<FilesystemInstaller> = Arc::new(FilesystemInstaller::new());

        // D-Bus proxy requires an async connection. Create it lazily on first
        // use via a wrapper that blocks on the handle. For now, block here.
        let shell_proxy: Arc<DbusShellProxy> = Arc::new(
            handle
                .block_on(DbusShellProxy::new())
                .expect("failed to connect to GNOME Shell D-Bus — is GNOME Shell running?"),
        );

        let browse = Arc::new(BrowseUseCase::new(
            ego_client,
            installer.clone(),
            shell_proxy.clone(),
        ));

        let manage = Arc::new(ManageUseCase::new(installer, shell_proxy));

        Self {
            handle,
            browse,
            manage,
        }
    }
}
