// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;

/// Port: persist generated CSS to the filesystem and activate it.
pub trait ThemeWriter: Send + Sync {
    fn write_gtk_css(&self, css: &str) -> Result<(), AppError>;
    fn write_shell_css(&self, css: &str, theme_name: &str) -> Result<(), AppError>;
    fn clear_overrides(&self) -> Result<(), AppError>;
}
