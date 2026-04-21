// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Port: recolor folder / accent icons to match the system accent.
//!
//! The mechanics vary by icon theme: Adwaita on GNOME 47+ tracks
//! `accent-color` natively (no work needed); Papirus exposes a
//! `papirus-folders` script that rewrites symlinks; most third-party
//! themes have no recolour mechanism at all. The adapter decides
//! which path applies; the port just takes an accent id.
//!
//! Advisory: an icon theme without recolour support is reported as
//! `RecolorOutcome::Unsupported` rather than an error, so the calling
//! use case can toast a hint to the user but not abort the apply.

use crate::AppError;

/// Outcome classification. The use case converts this into a user-
/// facing toast, so the variants are shaped around what the user
/// needs to hear, not internal control flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecolorOutcome {
    /// Recolour applied via the theme's external tool (e.g. Papirus
    /// folders). The string is a short human label ("Papirus") for
    /// the toast.
    Applied(String),
    /// No action needed — the active icon theme tracks
    /// `accent-color` natively. Toast still confirms so the user
    /// sees *something* happened on Apply.
    NativelyTracks(String),
    /// Recolour was requested but the active icon theme does not
    /// support recolouring via any mechanism we know about. The
    /// string carries the theme name so the toast can name it.
    Unsupported(String),
}

/// Recolour the currently-active icon theme's folder / accent
/// icons to match `accent_id` (a GNOME accent name like "blue").
pub trait IconThemeRecolorer: Send + Sync {
    fn recolor(&self, accent_id: &str) -> Result<RecolorOutcome, AppError>;
}
