// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Experience Pack version compatibility reports.
//!
//! An Experience Pack carries the `ShellVersion` it was snapshotted on.
//! Applying it to a different target version may silently drop or
//! degrade individual theme-builder values and shell tweaks. This
//! module classifies the diff so the UI can present it to the user
//! before applying (see GXF-060).
//!
//! The comparison piggybacks on [`crate::ShellTweakProfile`] so the
//! same per-version capability table that drives the Shell Tweaks UI
//! is the source of truth for pack compatibility.

use crate::{ShellTweak, ShellTweakId, ShellTweakProfile, ShellVersion};
use crate::theme_capability::ControlSupport;

/// How far apart the pack's and target's shell versions are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilitySeverity {
    /// Exact version match. Apply is safe.
    Match,
    /// Same major version, different minor. Usually fine; flagged for
    /// transparency.
    MinorDrift,
    /// Different major versions. Expect some tweaks to be unsupported
    /// or to behave differently.
    MajorDrift,
}

impl CompatibilitySeverity {
    pub fn is_match(self) -> bool {
        matches!(self, Self::Match)
    }
    pub fn is_major_drift(self) -> bool {
        matches!(self, Self::MajorDrift)
    }
}

/// A single concern raised by the compatibility check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompatibilityWarning {
    /// The pack contains a shell tweak that the target version does
    /// not support. Applying it will be a silent no-op on the target.
    UnsupportedTweak {
        id: ShellTweakId,
        reason: &'static str,
    },
    /// The pack contains a shell tweak that the target version can
    /// apply but with caveats (see reason).
    DegradedTweak {
        id: ShellTweakId,
        reason: &'static str,
    },
}

impl CompatibilityWarning {
    pub fn tweak_id(&self) -> ShellTweakId {
        match self {
            Self::UnsupportedTweak { id, .. } | Self::DegradedTweak { id, .. } => *id,
        }
    }

    /// Short user-facing description suitable for a toast or list item.
    pub fn summary(&self) -> String {
        match self {
            Self::UnsupportedTweak { id, reason } => {
                format!("{} is unsupported on the target: {}", id.label(), reason)
            }
            Self::DegradedTweak { id, reason } => {
                format!("{} will be degraded on the target: {}", id.label(), reason)
            }
        }
    }
}

/// Full report of differences between a pack and a target version.
#[derive(Debug, Clone)]
pub struct PackCompatibilityReport {
    pub pack_version: ShellVersion,
    pub target_version: ShellVersion,
    pub severity: CompatibilitySeverity,
    pub warnings: Vec<CompatibilityWarning>,
}

impl PackCompatibilityReport {
    /// Build a report by comparing a pack's captured shell tweaks
    /// against the target version's profile.
    ///
    /// `pack_version` is the version the pack was snapshotted on;
    /// `target_version` is the version that will execute the apply.
    /// `pack_tweaks` is the pack's `shell_tweaks` vector.
    pub fn build(
        pack_version: &ShellVersion,
        target_version: &ShellVersion,
        pack_tweaks: &[ShellTweak],
    ) -> Self {
        let severity = if pack_version.major == target_version.major {
            if pack_version.minor == target_version.minor {
                CompatibilitySeverity::Match
            } else {
                CompatibilitySeverity::MinorDrift
            }
        } else {
            CompatibilitySeverity::MajorDrift
        };

        let profile = ShellTweakProfile::for_version(target_version);
        let mut warnings = Vec::new();

        for tweak in pack_tweaks {
            let Some(hint) = profile.hint_for(tweak.id) else {
                // Tweak id exists in the pack but the target's profile
                // doesn't know it. Either forward-compat (newer app
                // feature) or backward-compat (removed). Skip — it'll
                // be dropped silently on apply anyway.
                continue;
            };
            match &hint.support {
                ControlSupport::Full => {}
                ControlSupport::Degraded { reason } => {
                    warnings.push(CompatibilityWarning::DegradedTweak {
                        id: tweak.id,
                        reason,
                    });
                }
                ControlSupport::Unsupported { reason } => {
                    warnings.push(CompatibilityWarning::UnsupportedTweak {
                        id: tweak.id,
                        reason,
                    });
                }
            }
        }

        Self {
            pack_version: pack_version.clone(),
            target_version: target_version.clone(),
            severity,
            warnings,
        }
    }

    /// True when the target is identical to the pack — applying is
    /// a no-surprise operation.
    pub fn is_clean(&self) -> bool {
        self.severity.is_match() && self.warnings.is_empty()
    }

    /// True when at least one captured tweak will visibly degrade or
    /// silently drop on the target. Applies are safe but users should
    /// be informed.
    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TweakValue;

    fn tweak(id: ShellTweakId, value: TweakValue) -> ShellTweak {
        ShellTweak { id, value }
    }

    #[test]
    fn same_version_no_tweaks_is_clean() {
        let v = ShellVersion::new(47, 0);
        let r = PackCompatibilityReport::build(&v, &v, &[]);
        assert!(r.is_clean());
        assert_eq!(r.severity, CompatibilitySeverity::Match);
        assert!(r.warnings.is_empty());
    }

    #[test]
    fn same_major_different_minor_is_minor_drift() {
        let a = ShellVersion::new(47, 0);
        let b = ShellVersion::new(47, 3);
        let r = PackCompatibilityReport::build(&a, &b, &[]);
        assert_eq!(r.severity, CompatibilitySeverity::MinorDrift);
    }

    #[test]
    fn different_major_is_major_drift() {
        let a = ShellVersion::new(46, 0);
        let b = ShellVersion::new(50, 0);
        let r = PackCompatibilityReport::build(&a, &b, &[]);
        assert!(r.severity.is_major_drift());
    }

    #[test]
    fn unsupported_tweak_in_pack_surfaces_warning() {
        // `AppsGridColumns` is Unsupported on GNOME 45 but Full on 47.
        // A pack built on 47 with AppsGridColumns set, applied on 45,
        // should raise one Unsupported warning.
        let pack_version = ShellVersion::new(47, 0);
        let target_version = ShellVersion::new(45, 0);
        let tweaks = vec![tweak(ShellTweakId::AppsGridColumns, TweakValue::Int(6))];
        let r = PackCompatibilityReport::build(&pack_version, &target_version, &tweaks);

        assert_eq!(r.warnings.len(), 1);
        assert!(matches!(
            r.warnings[0],
            CompatibilityWarning::UnsupportedTweak {
                id: ShellTweakId::AppsGridColumns,
                ..
            }
        ));
    }

    #[test]
    fn degraded_tweak_surfaces_warning_with_reason() {
        // `AppsGridColumns` is Degraded on GNOME 46+ (requires a shell
        // restart). A pack that includes it should raise a Degraded
        // warning when the target is 46.
        let target = ShellVersion::new(46, 0);
        let tweaks = vec![tweak(ShellTweakId::AppsGridColumns, TweakValue::Int(6))];
        let r = PackCompatibilityReport::build(&target, &target, &tweaks);

        assert!(r.warnings.iter().any(|w| matches!(
            w,
            CompatibilityWarning::DegradedTweak {
                id: ShellTweakId::AppsGridColumns,
                ..
            }
        )));
    }

    #[test]
    fn fully_supported_tweaks_produce_no_warnings() {
        // EnableAnimations is Full on every version we support.
        let pack = ShellVersion::new(45, 0);
        let target = ShellVersion::new(50, 0);
        let tweaks = vec![tweak(ShellTweakId::EnableAnimations, TweakValue::Bool(true))];
        let r = PackCompatibilityReport::build(&pack, &target, &tweaks);
        assert!(r.warnings.is_empty(), "got: {:?}", r.warnings);
    }

    #[test]
    fn summary_strings_are_self_contained() {
        // Regression guard: the summary() text should never include
        // a Debug-only field name like `id:` so we can drop them into
        // a user-facing toast.
        let w = CompatibilityWarning::UnsupportedTweak {
            id: ShellTweakId::TopBarPosition,
            reason: "needs bundled extension",
        };
        let s = w.summary();
        assert!(s.contains("Top Bar Position"));
        assert!(!s.contains("id:"));
        assert!(!s.contains("UnsupportedTweak"));
    }

    #[test]
    fn has_warnings_tracks_only_non_empty_warning_list() {
        let v = ShellVersion::new(47, 0);
        let clean = PackCompatibilityReport::build(&v, &v, &[]);
        assert!(!clean.has_warnings());

        let tweaks = vec![tweak(ShellTweakId::AppsGridColumns, TweakValue::Int(6))];
        let dirty = PackCompatibilityReport::build(&v, &ShellVersion::new(45, 0), &tweaks);
        assert!(dirty.has_warnings());
    }
}
