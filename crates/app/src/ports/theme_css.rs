// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;
use gnomex_domain::ThemeSpec;

/// Output of CSS generation — one stylesheet per target.
///
/// GTK4 and GTK3 use different token namespaces and widget selectors
/// (GTK4/Libadwaita: `@window_bg_color`, `.navigation-sidebar`, `.card`,
/// `splitview` etc.; GTK3: `@theme_bg_color`, `@theme_base_color`,
/// `.sidebar`, etc.). Emitting byte-identical CSS to both targets made
/// the GTK3 override mostly a no-op and sometimes actively worse than
/// vanilla Adwaita, so the two payloads are produced as separate
/// stylesheets and written to `gtk-4.0/gtk.css` and `gtk-3.0/gtk.css`
/// independently (GXF-001).
pub struct ThemeCss {
    pub gtk_css: String,
    pub gtk3_css: String,
    pub shell_css: String,
}

/// Port: generate platform-specific CSS from a version-agnostic ThemeSpec.
///
/// Each GNOME Shell major version implements this with its own selectors.
pub trait ThemeCssGenerator: Send + Sync {
    fn generate(&self, spec: &ThemeSpec) -> Result<ThemeCss, AppError>;
    fn version_label(&self) -> &str;
}
