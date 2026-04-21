// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;
use gnomex_domain::PerAppScaleOverride;

/// Port: write per-app `.desktop` override files that force
/// individual applications to launch at a specific HiDPI scale.
///
/// The underlying technique is the XDG user-override rule: any
/// `.desktop` file under `~/.local/share/applications/` shadows the
/// system copy in `/usr/share/applications/`. We copy the upstream
/// entry, rewrite the `Exec=` line to prefix `env GDK_SCALE=<n>` (and
/// `--force-device-scale-factor=<n>` for Chromium-family apps), and
/// leave everything else intact.
///
/// This is deliberately a port (not just a helper) because:
/// - Testing it requires a writable fake HOME, which means wrapping
///   filesystem I/O behind a trait so the domain tests don't touch
///   the user's real applications folder.
/// - A future Flatpak-aware adapter may want to write
///   `flatpak override --env=GDK_SCALE=...` instead of editing a
///   `.desktop` file. Keeping the port abstract leaves room for that.
pub trait AppLauncherOverrides: Send + Sync {
    /// Register an override: resolve the app id, copy the source
    /// `.desktop`, and write a modified copy to the user
    /// applications directory.
    ///
    /// Returns `Ok(())` on success. If the upstream `.desktop` can't
    /// be located, returns an `AppError::Settings` — the caller
    /// should surface that to the UI as a toast rather than crash.
    fn register_override(&self, spec: &PerAppScaleOverride) -> Result<(), AppError>;

    /// Remove a previously-registered override. No-op if the user
    /// override doesn't exist.
    fn remove_override(&self, app_id: &str) -> Result<(), AppError>;

    /// List app ids that currently have a GNOME X-managed override.
    /// Identification is done via a marker comment we embed when we
    /// write the file, so hand-edited user overrides are not
    /// misreported as ours.
    fn list_overrides(&self) -> Result<Vec<String>, AppError>;
}
