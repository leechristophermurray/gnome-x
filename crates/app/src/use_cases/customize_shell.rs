// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::ports::ShellCustomizer;
use crate::AppError;
use gnomex_domain::{ShellTweak, ShellTweakId, ShellTweakProfile, ShellVersion, TweakHint};
use std::sync::Arc;

/// Use case: orchestrate read / apply / snapshot of GNOME Shell tweaks
/// through the version-specific [`ShellCustomizer`] adapter.
///
/// Holds a [`ShellTweakProfile`] in addition to the adapter so the UI
/// can ask "what hints should I render for which tweaks?" without
/// reaching into Green directly.
pub struct CustomizeShellUseCase {
    customizer: Arc<dyn ShellCustomizer>,
    profile: ShellTweakProfile,
}

impl CustomizeShellUseCase {
    pub fn new(customizer: Arc<dyn ShellCustomizer>, version: &ShellVersion) -> Self {
        Self {
            profile: ShellTweakProfile::for_version(version),
            customizer,
        }
    }

    pub fn version_label(&self) -> &str {
        self.customizer.version_label()
    }

    /// List `(id, hint)` for every tweak in the profile, including
    /// `Unsupported` ones — Yellow renders those as disabled rows with
    /// the reason surfaced in the subtitle (per plan Decision 2). Order
    /// matches the profile's declaration order.
    pub fn list_all_hints(&self) -> Vec<(ShellTweakId, TweakHint)> {
        self.profile
            .hints
            .iter()
            .map(|h| (h.id, h.clone()))
            .collect()
    }

    /// Read the current value of a single tweak.
    pub async fn read(&self, id: ShellTweakId) -> Result<Option<ShellTweak>, AppError> {
        self.customizer.read(id).await
    }

    /// Apply a tweak. Unsupported tweaks are absorbed by the adapter
    /// with a log line — never an error.
    pub async fn apply(&self, tweak: ShellTweak) -> Result<(), AppError> {
        self.customizer.apply(&tweak).await
    }

    /// Snapshot every supported tweak's current value (for export).
    pub async fn snapshot(&self) -> Result<Vec<ShellTweak>, AppError> {
        self.customizer.snapshot().await
    }
}
