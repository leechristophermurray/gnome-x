// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! # gnomex-app
//!
//! Blue layer (L=1) — application use cases and port (trait) definitions.
//! Depends only on `gnomex-domain`. Infrastructure is injected through ports.

pub mod error;
pub mod ports;
pub mod use_cases;

pub use error::*;
