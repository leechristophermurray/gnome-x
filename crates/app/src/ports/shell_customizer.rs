// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;
use async_trait::async_trait;
use gnomex_domain::{ShellTweak, ShellTweakId};

/// Port: read, apply, and snapshot GNOME Shell behavioral tweaks for
/// a specific shell version.
///
/// This is the peer of [`crate::ports::ThemeCssGenerator`]: each
/// implementation absorbs the schemas, keys, and D-Bus signatures of
/// *one* GNOME Shell major version. The factory in `gnomex-infra`
/// (`create_shell_customizer`) selects the right one at runtime; no
/// other layer ever branches on shell version.
#[async_trait]
pub trait ShellCustomizer: Send + Sync {
    /// Display label for the running version, e.g. "GNOME 47".
    fn version_label(&self) -> &str;

    /// Tweak ids this version can read or write. The UI uses this list
    /// to decide what to render — Yellow never branches on version.
    fn supported_tweaks(&self) -> &[ShellTweakId];

    /// Read the current value. `Ok(None)` means the tweak is
    /// unsupported on this version.
    async fn read(&self, id: ShellTweakId) -> Result<Option<ShellTweak>, AppError>;

    /// Apply a tweak. Silently no-ops with a log line if unsupported.
    async fn apply(&self, tweak: &ShellTweak) -> Result<(), AppError>;

    /// Snapshot every supported tweak's current value. Used by the
    /// Experience Pack export path.
    async fn snapshot(&self) -> Result<Vec<ShellTweak>, AppError>;
}
