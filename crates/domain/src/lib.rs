// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! # gnomex-domain
//!
//! Green layer (L=0) — pure domain types, value objects, entities, and
//! business rules. This crate has **zero** external dependencies.

mod error;
mod extension;
mod experience_pack;
mod shell_version;
mod theme;

pub use error::*;
pub use extension::*;
pub use experience_pack::*;
pub use shell_version::*;
pub use theme::*;
