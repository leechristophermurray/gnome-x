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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{enable_animations, MockShellCustomizer};
    use gnomex_domain::theme_capability::ControlSupport;
    use gnomex_domain::{ShellTweakId, ShellVersion, TweakValue};

    #[test]
    fn list_all_hints_returns_full_profile_including_unsupported() {
        let customizer = MockShellCustomizer::new(vec![ShellTweakId::EnableAnimations]);
        let uc = CustomizeShellUseCase::new(customizer, &ShellVersion::new(47, 0));

        let hints = uc.list_all_hints();

        // Full profile means we see every id the version profile knows
        // about — including Unsupported ones — so the UI can render
        // disabled-with-reason rows.
        assert!(hints.len() > 5, "profile should carry many ids, got {}", hints.len());
        assert!(hints.iter().any(|(id, _)| *id == ShellTweakId::EnableAnimations));
        // Placeholder tweaks (waiting on bundled extension) must still
        // surface so users can see the reason.
        let has_unsupported = hints
            .iter()
            .any(|(_, h)| matches!(h.support, ControlSupport::Unsupported { .. }));
        assert!(has_unsupported, "expected at least one Unsupported hint");
    }

    #[tokio::test]
    async fn apply_routes_to_the_customizer() {
        let customizer = MockShellCustomizer::new(vec![ShellTweakId::EnableAnimations]);
        let uc = CustomizeShellUseCase::new(customizer.clone(), &ShellVersion::new(47, 0));

        uc.apply(enable_animations(true)).await.unwrap();
        uc.apply(enable_animations(false)).await.unwrap();

        let applies = customizer.applies.lock().unwrap().clone();
        assert_eq!(applies.len(), 2);
        assert!(matches!(applies[0].value, TweakValue::Bool(true)));
        assert!(matches!(applies[1].value, TweakValue::Bool(false)));
    }

    #[tokio::test]
    async fn snapshot_returns_what_the_customizer_returns() {
        let customizer = MockShellCustomizer::with_snapshot(
            vec![ShellTweakId::EnableAnimations],
            vec![enable_animations(true)],
        );
        let uc = CustomizeShellUseCase::new(customizer, &ShellVersion::new(47, 0));

        let snap = uc.snapshot().await.unwrap();
        assert_eq!(snap, vec![enable_animations(true)]);
    }

    #[test]
    fn version_label_forwards_to_adapter() {
        let customizer = MockShellCustomizer::new(vec![]);
        let uc = CustomizeShellUseCase::new(customizer, &ShellVersion::new(47, 0));
        assert_eq!(uc.version_label(), "MOCK GNOME 47+");
    }
}
