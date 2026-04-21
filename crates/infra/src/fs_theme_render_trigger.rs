// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! [`ThemeRenderTrigger`] adapter backed by GSettings + an
//! [`ApplyThemeUseCase`].
//!
//! On `rerender()` the trigger re-reads every `tb-*` key via
//! [`crate::gsettings_theme_spec::build_theme_spec_from_gsettings`],
//! then routes the resulting `ThemeSpec` through the full
//! [`ApplyThemeUseCase::apply`] pipeline. This means a pack apply
//! guarantees both `gtk-4.0/gtk.css` and `gtk-3.0/gtk.css` are
//! regenerated from the restored knobs (GXF-061), and external-app
//! themers (Chromium, VS Code) fan out too.
//!
//! The trigger is intentionally thin — the real composition is one
//! level up, where the caller owns the `ApplyThemeUseCase` and the
//! adapter table. This keeps the GSettings dependency isolated to the
//! `build_theme_spec_from_gsettings` helper rather than letting it
//! leak into the app-layer port.

use crate::gsettings_theme_spec::build_theme_spec_from_gsettings;
use gnomex_app::use_cases::ApplyThemeUseCase;
use gnomex_app::{ports::ThemeRenderTrigger, AppError};
use std::sync::Arc;

/// Concrete adapter implementing [`ThemeRenderTrigger`] by reading the
/// live `tb-*` GSettings keys and routing them through the provided
/// [`ApplyThemeUseCase`].
pub struct FsThemeRenderTrigger {
    apply: Arc<ApplyThemeUseCase>,
}

impl FsThemeRenderTrigger {
    pub fn new(apply: Arc<ApplyThemeUseCase>) -> Self {
        Self { apply }
    }
}

impl ThemeRenderTrigger for FsThemeRenderTrigger {
    fn rerender(&self) -> Result<(), AppError> {
        let spec = build_theme_spec_from_gsettings()?;
        self.apply.apply(&spec)
    }
}
