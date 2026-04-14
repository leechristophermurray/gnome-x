// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::AppError;
use gnomex_domain::ThemeSpec;

/// Output of CSS generation — one stylesheet per target.
pub struct ThemeCss {
    pub gtk_css: String,
    pub shell_css: String,
}

/// Port: generate platform-specific CSS from a version-agnostic ThemeSpec.
///
/// Each GNOME Shell major version implements this with its own selectors.
pub trait ThemeCssGenerator: Send + Sync {
    fn generate(&self, spec: &ThemeSpec) -> Result<ThemeCss, AppError>;
    fn version_label(&self) -> &str;
}
