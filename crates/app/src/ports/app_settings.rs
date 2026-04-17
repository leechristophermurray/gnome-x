// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;
use gnomex_domain::GSettingOverride;

/// Port: read/write the GNOME X application's own GSettings schema
/// (`io.github.gnomex.GnomeX`) — the theme-builder knobs (`tb-*`),
/// accent scheduling, and related customization toggles.
///
/// This is deliberately separate from [`AppearanceSettings`], which
/// targets GNOME-system schemas (`org.gnome.desktop.interface`, …).
/// Mixing the two in one port would couple the pack snapshot to
/// GNOME internals we don't want to leak into.
pub trait AppSettings: Send + Sync {
    /// Capture every pack-relevant key as `(key, gvariant_text)` pairs.
    /// Window-size / session-state keys are deliberately excluded — they
    /// aren't part of a user's *customization*. Values are encoded as
    /// GVariant text so round-tripping preserves types exactly.
    fn snapshot_pack_settings(&self) -> Result<Vec<GSettingOverride>, AppError>;

    /// Apply a batch of `(key, gvariant_text)` overrides to the GNOME X
    /// schema. Unknown keys and unparseable values are logged and
    /// skipped — the batch never aborts halfway.
    fn apply_overrides(&self, overrides: &[GSettingOverride]) -> Result<(), AppError>;
}
