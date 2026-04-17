// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! # gnomex-infra
//!
//! Red layer (L=3) — concrete implementations of the ports defined in
//! `gnomex-app`. Contains HTTP clients, D-Bus proxy, filesystem installer,
//! TOML pack storage, and GSettings adapter.

mod blur_my_shell;
mod chromium_themer;
mod dbus_shell_proxy;
mod ego_client;
mod filesystem_installer;
mod floating_dock;
mod gsettings_app_settings;
mod gsettings_appearance;
mod ocs_client;
mod pack_toml_storage;
pub mod shell_customizer;
pub mod theme_css;
mod theme_writer;
mod vscode_themer;

pub use blur_my_shell::GSettingsBlurMyShell;
pub use chromium_themer::ChromiumThemer;
pub use dbus_shell_proxy::DbusShellProxy;
pub use ego_client::EgoClient;
pub use filesystem_installer::FilesystemInstaller;
pub use floating_dock::GSettingsFloatingDock;
pub use gsettings_app_settings::GSettingsAppSettings;
pub use gsettings_appearance::GSettingsAppearance;
pub use ocs_client::OcsClient;
pub use pack_toml_storage::PackTomlStorage;
pub use theme_writer::FilesystemThemeWriter;
pub use vscode_themer::VscodeThemer;
