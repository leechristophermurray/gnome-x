// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Version-adaptive CSS generation for GNOME Shell and GTK4.
//!
//! Each supported GNOME version has its own adapter implementing
//! `ThemeCssGenerator`. The factory selects the right one at runtime
//! based on the detected shell version.
//!
//! The GTK3 output does **not** depend on the GNOME version — GTK 3.24
//! is the last stable release line — so a single [`gtk3::Gtk3CssGenerator`]
//! covers every case and each per-version adapter fills `ThemeCss::gtk3_css`
//! from the shared [`gtk3::generate_gtk3_css`] helper (GXF-001).

mod common;
mod gnome45;
mod gnome46;
mod gnome47;
mod gnome50;
pub mod gtk3;

use gnomex_app::ports::ThemeCssGenerator;
use gnomex_domain::ShellVersion;

/// Select the correct CSS adapter for the running GNOME Shell version.
///
/// Falls back to the latest known adapter for unrecognised versions,
/// keeping the app functional on day-one of a new GNOME release.
pub fn create_css_generator(version: &ShellVersion) -> Box<dyn ThemeCssGenerator> {
    match version.major {
        45 => Box::new(gnome45::Gnome45CssGenerator),
        46 => Box::new(gnome46::Gnome46CssGenerator),
        47..=49 => Box::new(gnome47::Gnome47CssGenerator),
        _ => Box::new(gnome50::Gnome50CssGenerator), // 50+ and future versions
    }
}
