// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Port: probe the running session for CSD vs SSD windows.
//!
//! Theme controls like headerbar height, window corner radius, and
//! drop-shadow only reach apps that draw their own decorations
//! (client-side). Server-side-decorated apps (legacy GTK 3 dialogs,
//! Electron without Wayland decorations, Wine/Proton) get their
//! titlebar from Mutter and are *not* re-themed by GNOME X.
//!
//! This port snapshots the current window list so the UI can warn the
//! user which apps won't pick up the theme. Implementations are
//! expected to be synchronous, fast (< 100 ms), and tolerant of
//! windowing-backend quirks — returning an empty report is fine when
//! no signal is available.
//!
//! See GXF-021.

use gnomex_domain::DecorationReport;

/// Snapshot probe for window decoration mode. Single method; no
/// subscription / streaming API — the UI re-probes when the user
/// opens the Theme Builder or Diagnostics view.
pub trait WindowDecorationProbe: Send + Sync {
    /// Inspect currently-open top-level windows and classify each as
    /// CSD / SSD / Unknown. Implementations must never panic;
    /// windowing-backend failures should surface as an empty report
    /// rather than an error.
    fn detect_decoration_mix(&self) -> DecorationReport;
}
