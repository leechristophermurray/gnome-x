// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! # gnomex-infra
//!
//! Red layer (L=3) — concrete implementations of the ports defined in
//! `gnomex-app`. Contains HTTP clients, D-Bus proxy, filesystem installer,
//! TOML pack storage, and GSettings adapter.

mod dbus_shell_proxy;
mod ego_client;
mod filesystem_installer;
mod pack_toml_storage;

pub use dbus_shell_proxy::DbusShellProxy;
pub use ego_client::EgoClient;
pub use filesystem_installer::FilesystemInstaller;
pub use pack_toml_storage::PackTomlStorage;
