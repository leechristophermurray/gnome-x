// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;

/// Port: read/write desktop appearance settings via GSettings.
pub trait AppearanceSettings: Send + Sync {
    fn get_gtk_theme(&self) -> Result<String, AppError>;
    fn set_gtk_theme(&self, name: &str) -> Result<(), AppError>;

    fn get_icon_theme(&self) -> Result<String, AppError>;
    fn set_icon_theme(&self, name: &str) -> Result<(), AppError>;

    fn get_cursor_theme(&self) -> Result<String, AppError>;
    fn set_cursor_theme(&self, name: &str) -> Result<(), AppError>;

    fn get_shell_theme(&self) -> Result<String, AppError>;
    fn set_shell_theme(&self, name: &str) -> Result<(), AppError>;

    fn get_wallpaper(&self) -> Result<String, AppError>;
    fn set_wallpaper(&self, uri: &str) -> Result<(), AppError>;

    /// Read the current GNOME enum accent id (`blue`, `teal`, …).
    /// Default impl returns an empty string so any mock that predates
    /// these methods keeps compiling.
    fn get_accent_color(&self) -> Result<String, AppError> {
        Ok(String::new())
    }

    /// Write the GNOME enum accent id. Default impl is a no-op for
    /// mocks; the production `GSettingsAppearance` overrides this to
    /// bounce past the DConf same-value-suppression so downstream
    /// consumers (Adwaita folder icons, the themer watchers in our
    /// own daemon) actually see a `changed` signal.
    fn set_accent_color(&self, _id: &str) -> Result<(), AppError> {
        Ok(())
    }
}
