// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Port: emit a [`WallpaperSlideshow`] as a GNOME-compatible XML file
//! under the user's `XDG_DATA_HOME/gnome-background-properties/`
//! directory.
//!
//! The return value is the absolute filesystem path to the written
//! XML — callers turn it into a `file://` URI and hand it to
//! [`AppearanceSettings::set_wallpaper`] so GNOME Shell picks it up.

use crate::AppError;
use gnomex_domain::WallpaperSlideshow;
use std::path::PathBuf;

/// Port: persist a slideshow spec to disk.
pub trait WallpaperSlideshowWriter: Send + Sync {
    /// Materialise `slideshow` as an XML file. Returns the absolute
    /// path of the written file. Must be atomic (write-to-temp +
    /// rename) so concurrent reads by `gnome-background-properties`
    /// never observe a truncated file.
    fn write(&self, slideshow: &WallpaperSlideshow) -> Result<PathBuf, AppError>;

    /// Remove a previously written slideshow XML. No-op if absent.
    /// Used by the UI's "delete slideshow" affordance.
    fn delete(&self, name: &str) -> Result<(), AppError>;
}
