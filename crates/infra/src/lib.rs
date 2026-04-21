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
pub mod desktop_app_launcher_overrides;
mod ego_client;
mod filesystem_installer;
mod floating_dock;
mod gsettings_app_settings;
mod gsettings_appearance;
pub mod gsettings_mutter;
mod ocs_client;
mod pack_toml_storage;
mod pkexec_gdm_themer;
pub mod shell_customizer;
pub mod theme_css;
pub mod theme_paths;
mod theme_writer;
mod theming_conflict_detector;
mod vscode_themer;
pub mod wallpaper_slideshow_xml;
pub mod window_decoration_probe;

pub use blur_my_shell::GSettingsBlurMyShell;
pub use chromium_themer::{build_chromium_gtk3_css, ChromiumThemer};
pub use dbus_shell_proxy::DbusShellProxy;
pub use desktop_app_launcher_overrides::DesktopAppLauncherOverrides;
pub use ego_client::EgoClient;
pub use filesystem_installer::FilesystemInstaller;
pub use floating_dock::GSettingsFloatingDock;
pub use gsettings_app_settings::GSettingsAppSettings;
pub use gsettings_appearance::GSettingsAppearance;
pub use gsettings_mutter::GSettingsMutter;
pub use ocs_client::OcsClient;
pub use pack_toml_storage::PackTomlStorage;
pub use pkexec_gdm_themer::PkexecGdmThemer;
pub use theme_writer::FilesystemThemeWriter;
pub use theming_conflict_detector::GioThemingConflictDetector;
pub use vscode_themer::VscodeThemer;
pub use wallpaper_slideshow_xml::{render_slideshow_xml, XdgWallpaperSlideshowWriter};
pub use window_decoration_probe::WmctrlDecorationProbe;
