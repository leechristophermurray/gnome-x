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
        Self::set_string(BG_SCHEMA, "picture-uri", uri)?;
        Self::set_string(BG_SCHEMA, "picture-uri-dark", uri)?;
        Ok(())
    }
}
