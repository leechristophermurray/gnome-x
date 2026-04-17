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

pub struct Gnome45ShellCustomizer {
    ext: ExtensionControllers,
}

impl Gnome45ShellCustomizer {
    pub fn new(ext: ExtensionControllers) -> Self {
        Self { ext }
    }

    /// GNOME 45 tracks the baseline exactly — no key renames or schema
    /// moves between 45 and the later versions for our v1 taxonomy.
    /// Future schema drift goes as a match arm *above* the fall-through
    /// to `baseline_key_for`.
    fn key_for(id: ShellTweakId) -> Option<(GSettingsKey, ValueShape)> {
        baseline_key_for(id)
    }
}

#[async_trait]
impl ShellCustomizer for Gnome45ShellCustomizer {
    fn version_label(&self) -> &str {
        "GNOME 45"
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
