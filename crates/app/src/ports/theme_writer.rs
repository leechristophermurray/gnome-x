// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;

/// Port: persist generated CSS to the filesystem and activate it.
///
/// GTK4 and GTK3 use different token namespaces (`@window_bg_color` vs
/// `@theme_bg_color`), different selectors (`.navigation-sidebar` vs
/// `.sidebar`), and different feature sets. The writer takes both
/// payloads separately so each can be routed to the correct XDG path
/// (`~/.config/gtk-4.0/gtk.css` and `~/.config/gtk-3.0/gtk.css`
/// respectively) without the infra layer having to split them.
pub trait ThemeWriter: Send + Sync {
    /// Persist the GTK4 override to `gtk-4.0/gtk.css` and the GTK3
    /// override to `gtk-3.0/gtk.css`. Both must carry a `"GNOME X"`
    /// managed-region marker so the conflict detector recognises them.
    fn write_gtk_css(&self, gtk4_css: &str, gtk3_css: &str) -> Result<(), AppError>;
    fn write_shell_css(&self, css: &str, theme_name: &str) -> Result<(), AppError>;
    fn clear_overrides(&self) -> Result<(), AppError>;
}
