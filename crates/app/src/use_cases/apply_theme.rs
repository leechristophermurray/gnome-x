// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::ports::{AppearanceSettings, ThemeCssGenerator, ThemeWriter};
use crate::AppError;
use gnomex_domain::ThemeSpec;
use std::sync::Arc;

const CUSTOM_THEME_NAME: &str = "GNOME-X-Custom";

/// Use case: apply or reset a ThemeSpec via version-appropriate CSS generation.
pub struct ApplyThemeUseCase {
    generator: Arc<dyn ThemeCssGenerator>,
    writer: Arc<dyn ThemeWriter>,
    appearance: Arc<dyn AppearanceSettings>,
}

impl ApplyThemeUseCase {
    pub fn new(
        generator: Arc<dyn ThemeCssGenerator>,
        writer: Arc<dyn ThemeWriter>,
        appearance: Arc<dyn AppearanceSettings>,
    ) -> Self {
        Self {
            generator,
            writer,
            appearance,
        }
    }

    /// Generate version-specific CSS and write it to disk.
    pub fn apply(&self, spec: &ThemeSpec) -> Result<(), AppError> {
        let css = self.generator.generate(spec)?;
        self.writer.write_gtk_css(&css.gtk_css)?;
        self.writer.write_shell_css(&css.shell_css, CUSTOM_THEME_NAME)?;
        self.appearance.set_shell_theme(CUSTOM_THEME_NAME).ok();
        Ok(())
    }

    /// Remove all custom CSS and reset shell theme.
    pub fn reset(&self) -> Result<(), AppError> {
        self.writer.clear_overrides()?;
        self.appearance.set_shell_theme("").ok();
        Ok(())
    }

    /// The GNOME version this generator targets.
    pub fn detected_version(&self) -> &str {
        self.generator.version_label()
    }
}
