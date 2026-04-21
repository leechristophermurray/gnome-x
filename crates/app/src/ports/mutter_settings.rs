// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;

/// Port: read/write the Mutter / desktop GSettings keys that control
/// compositor-level scaling behaviour but aren't part of the visual
/// theme per se.
///
/// Intentionally small: we only need the opt-in fractional-scaling
/// experimental flags and the text-scaling factor. Any richer Mutter
/// integration (per-monitor scale, DPI extraction) belongs in a
/// separate port so this one doesn't balloon.
pub trait MutterSettings: Send + Sync {
    /// Return the current `org.gnome.mutter experimental-features`
    /// strv. Order is preserved so callers can round-trip a GSetting
    /// without reshuffling flags the user set via other tools.
    fn experimental_features(&self) -> Result<Vec<String>, AppError>;

    /// Overwrite `experimental-features` with the given list. Callers
    /// are expected to preserve flags we don't manage (e.g.
    /// `variable-refresh-rate`) by reading the current value first,
    /// editing it, and writing it back — the adapter does not
    /// "merge", it replaces.
    fn set_experimental_features(&self, features: &[String]) -> Result<(), AppError>;

    /// Current `org.gnome.desktop.interface text-scaling-factor`.
    fn text_scaling_factor(&self) -> Result<f64, AppError>;

    /// Write `org.gnome.desktop.interface text-scaling-factor`.
    fn set_text_scaling_factor(&self, factor: f64) -> Result<(), AppError>;
}
