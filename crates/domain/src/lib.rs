// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! # gnomex-domain
//!
//! Green layer (L=0) — pure domain types, value objects, entities, and
//! business rules. This crate has **zero** external dependencies.

pub mod color;
mod decoration_report;
mod error;
mod extension;
mod experience_pack;
mod external_theme_spec;
mod material_palette;
mod pack_compatibility;
mod shadowed_resource;
mod shell_tweak;
mod shell_version;
mod theme;
pub mod theme_capability;
mod theme_spec;
mod theming_conflict;
mod wallpaper_slideshow;

pub use decoration_report::*;
pub use error::*;
pub use extension::*;
pub use experience_pack::*;
pub use external_theme_spec::*;
pub use material_palette::*;
pub use pack_compatibility::*;
pub use shadowed_resource::*;
pub use shell_tweak::*;
pub use shell_version::*;
pub use theme::*;
pub use theme_spec::*;
pub use theming_conflict::*;
pub use wallpaper_slideshow::*;
