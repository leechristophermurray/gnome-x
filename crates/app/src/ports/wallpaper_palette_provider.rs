// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Port: read the cached top-N wallpaper palette.
//!
//! The daemon already populates
//! `io.github.gnomex.GnomeX cached-wallpaper-palette` on every
//! wallpaper change. The Material-palette use case needs to read
//! that without taking a dependency on GSettings directly, so this
//! port abstracts the read.

use gnomex_domain::HexColor;

/// Provides the top three wallpaper colours, or `None` if the cache
/// isn't populated yet (e.g. first run, daemon not running).
pub trait WallpaperPaletteProvider: Send + Sync {
    /// Returns the top-3 palette in dominance order. `None` when
    /// fewer than three colours are currently cached — MD3 mode
    /// needs exactly three to build its role triple.
    fn top3(&self) -> Option<[HexColor; 3]>;
}
