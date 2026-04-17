// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use super::common::{
    apply_tweak, baseline_key_for, dispatch_extension, read_tweak, snapshot,
    ExtensionControllers, GSettingsKey, ValueShape, BASELINE_SUPPORTED,
};
use async_trait::async_trait;
use gnomex_app::ports::ShellCustomizer;
use gnomex_app::AppError;
use gnomex_domain::{ShellTweak, ShellTweakId};

pub struct Gnome47ShellCustomizer {
    ext: ExtensionControllers,
}

impl Gnome47ShellCustomizer {
    pub fn new(ext: ExtensionControllers) -> Self {
        Self { ext }
    }

    /// GNOME 47–49 track the baseline. This is the version range most
    /// users are on today; drift on any of the v1 tweaks here would be
    /// a strong signal something upstream has changed worth noting.
    fn key_for(id: ShellTweakId) -> Option<(GSettingsKey, ValueShape)> {
        baseline_key_for(id)
    }
}

#[async_trait]
impl ShellCustomizer for Gnome47ShellCustomizer {
    fn version_label(&self) -> &str {
        "GNOME 47+"
    }

    fn supported_tweaks(&self) -> &[ShellTweakId] {
        BASELINE_SUPPORTED
    }

    async fn read(&self, id: ShellTweakId) -> Result<Option<ShellTweak>, AppError> {
        read_tweak(id, Self::key_for)
    }

    async fn apply(&self, tweak: &ShellTweak) -> Result<(), AppError> {
        apply_tweak(tweak, Self::key_for)?;
        dispatch_extension(tweak, &self.ext);
        Ok(())
    }

    async fn snapshot(&self) -> Result<Vec<ShellTweak>, AppError> {
        snapshot(BASELINE_SUPPORTED, Self::key_for)
    }
}
