// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Port: detect active theming conflicts (extensions, legacy
//! gsettings, hand-edited CSS) that fight GNOME X's managed output.

use gnomex_domain::ConflictReport;

/// Scan the live GNOME session and report any detected conflicts.
///
/// Implementations are expected to be synchronous and fast (< 50 ms)
/// so the Theme Builder can call this on every view open without
/// noticeable latency.
pub trait ThemingConflictDetector: Send + Sync {
    fn detect(&self) -> Vec<ConflictReport>;
}
