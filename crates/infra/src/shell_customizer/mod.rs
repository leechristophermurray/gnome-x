// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Version-adaptive GNOME Shell behavioral tweaks.
//!
//! Each supported GNOME version has its own adapter implementing
//! [`ShellCustomizer`]. The factory selects the right one at runtime
//! based on the detected shell version. Mirrors `theme_css/`.

mod common;
mod gnome45;
mod gnome46;
mod gnome47;
mod gnome50;

pub use common::ExtensionControllers;

use gnomex_app::ports::ShellCustomizer;
use gnomex_domain::ShellVersion;

/// Select the right shell-customizer adapter for the running version.
///
/// Falls through to the latest known adapter for unrecognised
/// versions, keeping the app functional on day-one of a new release.
/// Extension controllers are injected so adapters can drive live
/// shell effects (overview blur via Blur My Shell, floating dock
/// via Dash to Dock) in addition to persisting the intent via
/// GSettings.
pub fn create_shell_customizer(
    version: &ShellVersion,
    ext: ExtensionControllers,
) -> Box<dyn ShellCustomizer> {
    match version.major {
        45 => Box::new(gnome45::Gnome45ShellCustomizer::new(ext)),
        46 => Box::new(gnome46::Gnome46ShellCustomizer::new(ext)),
        47..=49 => Box::new(gnome47::Gnome47ShellCustomizer::new(ext)),
        _ => Box::new(gnome50::Gnome50ShellCustomizer::new(ext)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gnomex_app::ports::{BlurMyShellController, FloatingDockController, ShellCustomizer};
    use gnomex_app::AppError;
    use std::sync::Arc;

    /// Test doubles that report not-available and no-op on apply.
    /// Keeps the factory tests hermetic — they don't touch system
    /// extension schemas or GSettings.
    struct StubFloatingDock;
    impl FloatingDockController for StubFloatingDock {
        fn is_available(&self) -> bool {
            false
        }
        fn apply(&self, _: bool) -> Result<(), AppError> {
            Ok(())
        }
    }
    struct StubBlurMyShell;
    impl BlurMyShellController for StubBlurMyShell {
        fn is_available(&self) -> bool {
            false
        }
        fn apply(&self, _: bool) -> Result<(), AppError> {
            Ok(())
        }
    }

    fn stub_controllers() -> ExtensionControllers {
        ExtensionControllers {
            floating_dock: Arc::new(StubFloatingDock),
            blur_my_shell: Arc::new(StubBlurMyShell),
        }
    }

    #[test]
    fn factory_routes_each_major_to_the_right_adapter() {
        let cases = [
            (45, "GNOME 45"),
            (46, "GNOME 46"),
            (47, "GNOME 47+"),
            (48, "GNOME 47+"),
            (49, "GNOME 47+"),
            (50, "GNOME 50 (Libadwaita 1.9)"),
            (99, "GNOME 50 (Libadwaita 1.9)"),
        ];
        for (major, expected_label) in cases {
            let adapter =
                create_shell_customizer(&ShellVersion::new(major, 0), stub_controllers());
            assert_eq!(adapter.version_label(), expected_label, "major {major}");
        }
    }

    #[test]
    fn every_adapter_supports_the_baseline_taxonomy() {
        use super::common::BASELINE_SUPPORTED;
        let expected = BASELINE_SUPPORTED.len();
        for major in [45u32, 46, 47, 48, 50] {
            let a = create_shell_customizer(&ShellVersion::new(major, 0), stub_controllers());
            let supported = a.supported_tweaks();
            assert!(
                supported.len() >= expected,
                "version {major} advertises {} tweaks, expected >= {}: {:?}",
                supported.len(),
                expected,
                supported
            );
        }
    }
}
