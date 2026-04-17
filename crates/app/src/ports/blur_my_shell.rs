// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;

/// Port: drive the "overview wallpaper blur" feature.
///
/// GNOME Shell has no native overview-blur toggle; the real effect is
/// provided by the third-party **Blur My Shell** extension. When that
/// extension is installed, enabling blur writes to its GSettings
/// sub-schemas (`org.gnome.shell.extensions.blur-my-shell.overview`,
/// etc.). When it isn't, the UI falls back to a CSS dim overlay only.
pub trait BlurMyShellController: Send + Sync {
    /// True if Blur My Shell is installed (its overview sub-schema is
    /// present on this system).
    fn is_available(&self) -> bool;

    /// Toggle overview wallpaper blur. Writes the Blur My Shell
    /// overview sub-schema when `enabled = true`, resets it to
    /// extension defaults when `enabled = false`.
    fn apply(&self, enabled: bool) -> Result<(), AppError>;
}
