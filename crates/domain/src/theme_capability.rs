// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Theme capability model — domain knowledge about what each GNOME version
//! supports for theming controls. Pure policy, no IO.

use crate::{ShellVersion, ThemeSpec};

/// Identifies a specific theme builder control.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeControlId {
    WindowRadius,
    ElementRadius,
    PanelRadius,
    PanelOpacity,
    PanelTint,
    AccentTint,
    DashOpacity,
    OverviewBlur,
    AccentColor,
}

/// How well a GNOME version supports a given control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlSupport {
    /// Fully supported, works as expected.
    Full,
    /// Works but with caveats the user should know about.
    Degraded { reason: &'static str },
    /// Not available on this version at all.
    Unsupported { reason: &'static str },
}

impl ControlSupport {
    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }

    pub fn is_unsupported(&self) -> bool {
        matches!(self, Self::Unsupported { .. })
    }

    pub fn reason(&self) -> Option<&'static str> {
        match self {
            Self::Full => None,
            Self::Degraded { reason } | Self::Unsupported { reason } => Some(reason),
        }
    }
}

/// A control paired with its support level and optional safe value range.
#[derive(Debug, Clone)]
pub struct ControlHint {
    pub control: ThemeControlId,
    pub support: ControlSupport,
    /// If set, values outside this range degrade on this version.
    pub safe_range: Option<(f64, f64)>,
}

/// Capability profile for a specific GNOME version.
#[derive(Debug, Clone)]
pub struct VersionProfile {
    pub version_major: u32,
    pub hints: Vec<ControlHint>,
}

impl VersionProfile {
    /// Build the capability profile for a given shell version.
    /// This is the single source of truth for version compatibility.
    pub fn for_version(version: &ShellVersion) -> Self {
        let major = version.major;
        let mut hints = Vec::new();

        // --- Accent color (introduced GNOME 46) ---
        hints.push(ControlHint {
            control: ThemeControlId::AccentColor,
            support: if major >= 46 {
                ControlSupport::Full
            } else {
                ControlSupport::Unsupported {
                    reason: "Accent color requires GNOME 46+",
                }
            },
            safe_range: None,
        });

        // --- Accent tinting ---
        hints.push(ControlHint {
            control: ThemeControlId::AccentTint,
            support: if major >= 45 {
                ControlSupport::Full
            } else {
                ControlSupport::Unsupported {
                    reason: "GTK4 CSS theming requires GNOME 45+",
                }
            },
            safe_range: Some((0.0, 20.0)),
        });

        // --- Window radius ---
        hints.push(ControlHint {
            control: ThemeControlId::WindowRadius,
            support: ControlSupport::Full,
            safe_range: if major < 46 {
                Some((0.0, 24.0)) // older Mutter clips large radii
            } else {
                Some((0.0, 48.0))
            },
        });

        // --- Element radius ---
        hints.push(ControlHint {
            control: ThemeControlId::ElementRadius,
            support: ControlSupport::Full,
            safe_range: Some((0.0, 24.0)),
        });

        // --- Panel radius ---
        hints.push(ControlHint {
            control: ThemeControlId::PanelRadius,
            support: if major >= 45 {
                ControlSupport::Full
            } else {
                ControlSupport::Unsupported {
                    reason: "Shell panel theming requires GNOME 45+",
                }
            },
            safe_range: if major < 47 {
                Some((0.0, 12.0)) // older shells clip above 12px
            } else {
                Some((0.0, 24.0))
            },
        });

        // --- Panel opacity ---
        hints.push(ControlHint {
            control: ThemeControlId::PanelOpacity,
            support: if major >= 45 {
                ControlSupport::Full
            } else {
                ControlSupport::Unsupported {
                    reason: "Shell panel theming requires GNOME 45+",
                }
            },
            safe_range: None,
        });

        // --- Panel tint ---
        hints.push(ControlHint {
            control: ThemeControlId::PanelTint,
            support: if major >= 45 {
                ControlSupport::Full
            } else {
                ControlSupport::Unsupported {
                    reason: "Shell panel theming requires GNOME 45+",
                }
            },
            safe_range: None,
        });

        // --- Dash opacity ---
        hints.push(ControlHint {
            control: ThemeControlId::DashOpacity,
            support: if major >= 45 {
                ControlSupport::Full
            } else {
                ControlSupport::Unsupported {
                    reason: "Shell dash theming requires GNOME 45+",
                }
            },
            safe_range: None,
        });

        // --- Overview blur ---
        hints.push(ControlHint {
            control: ThemeControlId::OverviewBlur,
            support: if major >= 50 {
                ControlSupport::Full
            } else if major >= 45 {
                ControlSupport::Degraded {
                    reason: "No reduced-motion guard before GNOME 50",
                }
            } else {
                ControlSupport::Unsupported {
                    reason: "Shell theming requires GNOME 45+",
                }
            },
            safe_range: None,
        });

        Self {
            version_major: major,
            hints,
        }
    }

    /// Get the hint for a specific control.
    pub fn hint_for(&self, control: ThemeControlId) -> Option<&ControlHint> {
        self.hints.iter().find(|h| h.control == control)
    }

    /// Check whether a specific value exceeds the safe range for a control
    /// on this version. Returns a warning message if so.
    pub fn check_value(&self, control: ThemeControlId, value: f64) -> Option<String> {
        let hint = self.hint_for(control)?;
        if let Some((min, max)) = hint.safe_range {
            if value < min || value > max {
                return Some(format!(
                    "GNOME {}: values above {max} may not render correctly",
                    self.version_major,
                ));
            }
        }
        if hint.support.is_unsupported() {
            return Some(format!(
                "GNOME {}: {}",
                self.version_major,
                hint.support.reason().unwrap_or("unsupported"),
            ));
        }
        None
    }
}

/// Generate a cross-version compatibility report for a ThemeSpec.
/// Returns warnings for each control that has issues on any supported version.
pub fn compatibility_report(spec: &ThemeSpec) -> Vec<(ThemeControlId, Vec<String>)> {
    let supported_versions = [45, 46, 47, 48, 49, 50];
    let controls_and_values = [
        (ThemeControlId::WindowRadius, spec.window_radius.as_px()),
        (ThemeControlId::ElementRadius, spec.element_radius.as_px()),
        (ThemeControlId::PanelRadius, spec.panel.radius.as_px()),
        (ThemeControlId::PanelOpacity, spec.panel.opacity.as_percent()),
        (ThemeControlId::AccentTint, spec.tint.intensity.as_percent()),
        (ThemeControlId::DashOpacity, spec.dash.opacity.as_percent()),
    ];

    let mut report = Vec::new();

    for &(control, value) in &controls_and_values {
        let mut warnings = Vec::new();
        for &major in &supported_versions {
            let ver = ShellVersion::new(major, 0);
            let profile = VersionProfile::for_version(&ver);
            if let Some(warning) = profile.check_value(control, value) {
                warnings.push(warning);
            }
        }
        if !warnings.is_empty() {
            report.push((control, warnings));
        }
    }

    // Check boolean/special controls
    let blur_warnings: Vec<String> = supported_versions
        .iter()
        .filter_map(|&major| {
            let ver = ShellVersion::new(major, 0);
            let profile = VersionProfile::for_version(&ver);
            let hint = profile.hint_for(ThemeControlId::OverviewBlur)?;
            match &hint.support {
                ControlSupport::Degraded { reason } => {
                    Some(format!("GNOME {major}: {reason}"))
                }
                ControlSupport::Unsupported { reason } => {
                    Some(format!("GNOME {major}: {reason}"))
                }
                _ => None,
            }
        })
        .collect();
    if !blur_warnings.is_empty() {
        report.push((ThemeControlId::OverviewBlur, blur_warnings));
    }

    let accent_warnings: Vec<String> = supported_versions
        .iter()
        .filter_map(|&major| {
            let ver = ShellVersion::new(major, 0);
            let profile = VersionProfile::for_version(&ver);
            let hint = profile.hint_for(ThemeControlId::AccentColor)?;
            if hint.support.is_unsupported() {
                Some(format!(
                    "GNOME {major}: {}",
                    hint.support.reason().unwrap_or("unsupported"),
                ))
            } else {
                None
            }
        })
        .collect();
    if !accent_warnings.is_empty() {
        report.push((ThemeControlId::AccentColor, accent_warnings));
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gnome_45_no_accent_color() {
        let profile = VersionProfile::for_version(&ShellVersion::new(45, 0));
        let hint = profile.hint_for(ThemeControlId::AccentColor).unwrap();
        assert!(hint.support.is_unsupported());
    }

    #[test]
    fn gnome_47_has_accent_color() {
        let profile = VersionProfile::for_version(&ShellVersion::new(47, 0));
        let hint = profile.hint_for(ThemeControlId::AccentColor).unwrap();
        assert!(hint.support.is_full());
    }

    #[test]
    fn gnome_45_panel_radius_capped() {
        let profile = VersionProfile::for_version(&ShellVersion::new(45, 0));
        assert!(profile.check_value(ThemeControlId::PanelRadius, 8.0).is_none());
        assert!(profile.check_value(ThemeControlId::PanelRadius, 16.0).is_some());
    }

    #[test]
    fn gnome_50_full_blur_support() {
        let profile = VersionProfile::for_version(&ShellVersion::new(50, 0));
        let hint = profile.hint_for(ThemeControlId::OverviewBlur).unwrap();
        assert!(hint.support.is_full());
    }

    #[test]
    fn gnome_46_degraded_blur() {
        let profile = VersionProfile::for_version(&ShellVersion::new(46, 0));
        let hint = profile.hint_for(ThemeControlId::OverviewBlur).unwrap();
        assert!(matches!(hint.support, ControlSupport::Degraded { .. }));
    }

    #[test]
    fn cross_version_report() {
        use crate::*;
        let spec = ThemeSpec {
            window_radius: Radius::new(36.0).unwrap(), // exceeds GNOME 45's safe range
            element_radius: Radius::new(6.0).unwrap(),
            panel: PanelSpec {
                radius: Radius::new(20.0).unwrap(), // exceeds GNOME 45-46's safe range
                opacity: Opacity::from_percent(80.0).unwrap(),
                tint: HexColor::new("#1a1a1e").unwrap(),
            },
            dash: DashSpec {
                opacity: Opacity::from_percent(70.0).unwrap(),
            },
            tint: TintSpec {
                accent_hex: HexColor::new("#3584e4").unwrap(),
                intensity: Opacity::from_percent(5.0).unwrap(),
            },
            overview_blur: true,
        };

        let report = compatibility_report(&spec);
        // Window radius 36 should flag GNOME 45 (safe range 0-24)
        let wr = report.iter().find(|(c, _)| *c == ThemeControlId::WindowRadius);
        assert!(wr.is_some());
        // Panel radius 20 should flag GNOME 45-46 (safe range 0-12)
        let pr = report.iter().find(|(c, _)| *c == ThemeControlId::PanelRadius);
        assert!(pr.is_some());
    }
}
