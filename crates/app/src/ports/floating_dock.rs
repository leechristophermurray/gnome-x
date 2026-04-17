// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;

/// Port: drive the "floating dock" feature.
///
/// GNOME Shell's built-in dash only renders inside the Activities
/// overview. To get an always-visible floating dock we rely on the
/// third-party **Dash to Dock** extension — its GSettings schema
/// (`org.gnome.shell.extensions.dash-to-dock`) is configured to
/// produce the floating look.
///
/// Implementations report availability so the UI can degrade
/// gracefully when the extension isn't installed.
pub trait FloatingDockController: Send + Sync {
    /// True if Dash to Dock is installed (its GSettings schema is
    /// present on this system).
    fn is_available(&self) -> bool;

    /// Toggle floating-dock mode. Writes the Dash to Dock GSettings
    /// preset when `enabled = true`, and resets them to upstream
    /// defaults when `enabled = false`.
    fn apply(&self, enabled: bool) -> Result<(), AppError>;
}
