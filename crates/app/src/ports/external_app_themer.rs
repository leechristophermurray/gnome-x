// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;
use gnomex_domain::ExternalThemeSpec;

/// Port: propagate the current GNOME appearance (accent + tint + light/dark)
/// to a family of third-party applications (Chromium browsers, VS Code
/// editors, ...).
///
/// Implementations are expected to be idempotent: repeated `apply()` calls
/// with the same spec should not drift (they should overwrite their own
/// previously-written region rather than appending). Implementations
/// MUST NOT clobber user-authored configuration outside of a clearly
/// delimited GNOME X-owned region.
pub trait ExternalAppThemer: Send + Sync {
    /// Short identifier used for logging (e.g. `"chromium"`, `"vscode"`).
    fn name(&self) -> &str;

    /// Propagate the spec to every installed instance of this app family
    /// that we can locate. Absence of the target app is not an error —
    /// return `Ok(())` and log at debug level.
    fn apply(&self, spec: &ExternalThemeSpec) -> Result<(), AppError>;

    /// Remove the GNOME X-owned region from every detected instance,
    /// restoring the user's original configuration.
    fn reset(&self) -> Result<(), AppError>;
}
