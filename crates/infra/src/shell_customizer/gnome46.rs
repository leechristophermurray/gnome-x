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

pub struct Gnome46ShellCustomizer {
    ext: ExtensionControllers,
}

impl Gnome46ShellCustomizer {
    pub fn new(ext: ExtensionControllers) -> Self {
        Self { ext }
    }

    /// GNOME 46 tracks the baseline. The Libadwaita 1.5 bump brought
    /// new accent-color plumbing, but the v1 shell-tweak keys are
    /// unchanged from 45.
    fn key_for(id: ShellTweakId) -> Option<(GSettingsKey, ValueShape)> {
        baseline_key_for(id)
    }
}

#[async_trait]
impl ShellCustomizer for Gnome46ShellCustomizer {
    fn version_label(&self) -> &str {
        "GNOME 46"
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
