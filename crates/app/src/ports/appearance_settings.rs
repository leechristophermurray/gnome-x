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
}
