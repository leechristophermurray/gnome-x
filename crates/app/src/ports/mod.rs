// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Port definitions (interfaces) for infrastructure adapters.
//!
//! These traits define what the application layer *needs* from the outside
//! world. Concrete implementations live in `gnomex-infra` (Red layer).

mod appearance_settings;
mod content_repository;
mod external_app_themer;
mod extension_repository;
mod local_installer;
mod pack_storage;
mod shell_proxy;
mod theme_css;
mod theme_writer;

pub use appearance_settings::*;
pub use content_repository::*;
pub use external_app_themer::*;
pub use extension_repository::*;
pub use local_installer::*;
pub use pack_storage::*;
pub use shell_proxy::*;
pub use theme_css::*;
pub use theme_writer::*;
