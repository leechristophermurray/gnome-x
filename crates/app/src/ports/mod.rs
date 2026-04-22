// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Port definitions (interfaces) for infrastructure adapters.
//!
//! These traits define what the application layer *needs* from the outside
//! world. Concrete implementations live in `gnomex-infra` (Red layer).

mod app_launcher_overrides;
mod app_settings;
mod appearance_settings;
mod blur_my_shell;
mod content_repository;
mod external_app_themer;
mod extension_repository;
mod floating_dock;
mod gdm_themer;
mod icon_theme_recolorer;
mod local_installer;
mod mutter_settings;
mod pack_storage;
mod shell_customizer;
mod shell_proxy;
mod theme_css;
mod theme_render_trigger;
mod theme_writer;
mod theming_conflict_detector;
mod wallpaper_slideshow_writer;
mod window_decoration_probe;

pub use app_launcher_overrides::*;
pub use app_settings::*;
pub use appearance_settings::*;
pub use blur_my_shell::*;
pub use content_repository::*;
pub use external_app_themer::*;
pub use extension_repository::*;
pub use floating_dock::*;
pub use gdm_themer::*;
pub use icon_theme_recolorer::*;
pub use local_installer::*;
pub use mutter_settings::*;
pub use pack_storage::*;
pub use shell_customizer::*;
pub use shell_proxy::*;
pub use theme_css::*;
pub use theme_render_trigger::*;
pub use theme_writer::*;
pub use theming_conflict_detector::*;
pub use wallpaper_slideshow_writer::*;
pub use window_decoration_probe::*;
