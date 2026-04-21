// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Port: apply (or clear) GNOME X theme overrides to the GDM
//! login-screen dconf database.
//!
//! GDM runs as its own user (`gdm`) with its own GSettings DB. Our
//! regular user-level writes never reach it. The adapter behind this
//! port elevates through polkit (`pkexec experiencectl gdm-apply …`)
//! to run the elevated-side helper, which writes a dconf snippet
//! under `/etc/dconf/db/gdm.d/` and runs `dconf update`.
//!
//! The port itself exposes no polkit / subprocess concepts — the
//! caller just provides a validated theme name and accent hex and
//! the adapter figures out how to authorise the write. Adapters
//! MUST refuse to propagate any other arguments from the caller to
//! the elevated helper; the elevated helper re-validates its inputs
//! regardless.
//!
//! Implementations MUST be idempotent. A repeated call with the same
//! inputs should leave the dconf file byte-identical so `dconf update`
//! is effectively a no-op. Files we author MUST contain the literal
//! substring `"GNOME X"` so a subsequent `reset` can identify them.

use crate::AppError;
use gnomex_domain::HexColor;

/// Port: elevated writes to the GDM dconf database.
pub trait GdmThemer: Send + Sync {
    /// Apply the given shell-theme NAME + accent hex to GDM.
    ///
    /// `theme_name` is expected to be a safe shell-identifier (see
    /// `validate_theme_name` in `use_cases::gdm_theme`); adapters
    /// MUST re-validate and reject anything that looks path-like.
    fn apply(&self, theme_name: &str, accent: &HexColor) -> Result<(), AppError>;

    /// Remove GNOME X-authored overrides from the GDM dconf database.
    fn reset(&self) -> Result<(), AppError>;
}
