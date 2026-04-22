// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use gnomex_app::ports::AppearanceSettings;
use gnomex_app::AppError;
use gio::prelude::*;

const IFACE_SCHEMA: &str = "org.gnome.desktop.interface";
const BG_SCHEMA: &str = "org.gnome.desktop.background";
const SHELL_SCHEMA: &str = "org.gnome.shell.extensions.user-theme";

/// Concrete adapter: read/write desktop appearance via GSettings.
///
/// GSettings objects are GLib types that aren't Send+Sync, so we create
/// short-lived handles on each call rather than caching them in the struct.
pub struct GSettingsAppearance {
    has_shell_theme: bool,
}

impl GSettingsAppearance {
    pub fn new() -> Self {
        let has_shell_theme = gio::SettingsSchemaSource::default()
            .and_then(|src| src.lookup(SHELL_SCHEMA, true))
            .is_some();

        Self { has_shell_theme }
    }

    fn get_string(schema: &str, key: &str) -> Result<String, AppError> {
        let settings = gio::Settings::new(schema);
        Ok(settings.string(key).to_string())
    }

    fn set_string(schema: &str, key: &str, value: &str) -> Result<(), AppError> {
        let settings = gio::Settings::new(schema);
        settings
            .set_string(key, value)
            .map_err(|e| AppError::Settings(format!("failed to set {key}: {e}")))?;
        Ok(())
    }
}

impl AppearanceSettings for GSettingsAppearance {
    fn get_gtk_theme(&self) -> Result<String, AppError> {
        Self::get_string(IFACE_SCHEMA, "gtk-theme")
    }

    fn set_gtk_theme(&self, name: &str) -> Result<(), AppError> {
        Self::set_string(IFACE_SCHEMA, "gtk-theme", name)
    }

    fn get_icon_theme(&self) -> Result<String, AppError> {
        Self::get_string(IFACE_SCHEMA, "icon-theme")
    }

    fn set_icon_theme(&self, name: &str) -> Result<(), AppError> {
        Self::set_string(IFACE_SCHEMA, "icon-theme", name)
    }

    fn get_cursor_theme(&self) -> Result<String, AppError> {
        Self::get_string(IFACE_SCHEMA, "cursor-theme")
    }

    fn set_cursor_theme(&self, name: &str) -> Result<(), AppError> {
        Self::set_string(IFACE_SCHEMA, "cursor-theme", name)
    }

    fn get_shell_theme(&self) -> Result<String, AppError> {
        if self.has_shell_theme {
            Self::get_string(SHELL_SCHEMA, "name")
        } else {
            Ok(String::new())
        }
    }

    fn set_shell_theme(&self, name: &str) -> Result<(), AppError> {
        if self.has_shell_theme {
            Self::set_string(SHELL_SCHEMA, "name", name)
        } else {
            Err(AppError::Settings(
                "User Themes extension not installed — cannot set shell theme".into(),
            ))
        }
    }

    fn get_wallpaper(&self) -> Result<String, AppError> {
        Self::get_string(BG_SCHEMA, "picture-uri")
    }

    fn set_wallpaper(&self, uri: &str) -> Result<(), AppError> {
        // GSettings `set_string` suppresses the write when the new
        // value equals the old — the `changed` signal never fires.
        // For slideshow Apply this bites: we overwrite the XML file
        // in place but `picture-uri` still holds the identical URI,
        // so GNOME Shell never re-reads it and keeps playing the
        // stale cycle. Clear first, then set, so the signal fires
        // twice and Mutter picks up the new XML content.
        let current = Self::get_string(BG_SCHEMA, "picture-uri").unwrap_or_default();
        if current == uri {
            Self::set_string(BG_SCHEMA, "picture-uri", "")?;
            Self::set_string(BG_SCHEMA, "picture-uri-dark", "")?;
        }
        Self::set_string(BG_SCHEMA, "picture-uri", uri)?;
        Self::set_string(BG_SCHEMA, "picture-uri-dark", uri)?;
        Ok(())
    }

    fn get_accent_color(&self) -> Result<String, AppError> {
        Self::get_string(IFACE_SCHEMA, "accent-color")
    }

    fn get_color_scheme(&self) -> Result<String, AppError> {
        Self::get_string(IFACE_SCHEMA, "color-scheme")
    }

    fn set_accent_color(&self, id: &str) -> Result<(), AppError> {
        // Same same-value-suppression story as `set_wallpaper`. For
        // icons in particular the bounce is load-bearing: Adwaita
        // 47+ recolours folder icons on the `changed::accent-color`
        // signal, and if we're re-applying because the wallpaper
        // cycled to a new image that happens to map to the same
        // stock GNOME accent (very common — there are only 9 to
        // pick from), Adwaita would otherwise never get the nudge
        // to re-evaluate. Bouncing through a different valid enum
        // value (`blue` unless we're already on it, else `slate`)
        // forces the signal. The flicker is invisible because the
        // flush-to-target write follows immediately.
        let current = Self::get_string(IFACE_SCHEMA, "accent-color").unwrap_or_default();
        if current == id {
            let bounce_via = if id == "blue" { "slate" } else { "blue" };
            Self::set_string(IFACE_SCHEMA, "accent-color", bounce_via)?;
        }
        Self::set_string(IFACE_SCHEMA, "accent-color", id)?;
        Ok(())
    }
}
