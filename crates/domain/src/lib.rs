// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! # gnomex-domain
//!
//! Green layer (L=0) — pure domain types, value objects, entities, and
//! business rules. This crate has **zero** external dependencies.

pub mod color;
mod error;
mod extension;
mod experience_pack;
mod shell_version;
mod theme;
mod theme_spec;

pub use error::*;
pub use extension::*;
pub use experience_pack::*;
pub use shell_version::*;
pub use theme::*;
pub use theme_spec::*;
