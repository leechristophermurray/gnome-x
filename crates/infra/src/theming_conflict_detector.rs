// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Detects active theming conflicts on the live GNOME session.
//!
//! Red-layer implementation of [`ThemingConflictDetector`]. Reads:
//!
//! - `org.gnome.shell enabled-extensions` for extensions known to
//!   apply their own appearance overrides.
//! - `org.gnome.desktop.interface gtk-theme` for a non-default legacy
//!   GTK theme GSetting (set by older GNOME Tweaks flows) that would
//!   override our theme name.
//! - `~/.config/gtk-4.0/gtk.css` and `~/.config/gtk-3.0/gtk.css` for
//!   hand-edited files missing our `GNOME X` managed-region marker.

use std::path::PathBuf;

use gio::prelude::*;
use gnomex_app::ports::ThemingConflictDetector;
use gnomex_domain::{ConflictKind, ConflictReport};

/// Gio/filesystem-backed detector. Cheap to construct; heavy work
/// happens in [`detect`].
pub struct GioThemingConflictDetector {
    /// Optional `$HOME` override for test hermeticity.
    home: Option<PathBuf>,
}

impl GioThemingConflictDetector {
    pub fn new() -> Self {
        Self { home: None }
    }

    /// Construct a detector that looks for hand-edited gtk.css under
    /// the given home directory rather than the process's `$HOME`.
    /// Extension scanning still goes through the real GSettings bus —
    /// tests that want to stub that should use a mock adapter.
    pub fn with_home(home: PathBuf) -> Self {
        Self { home: Some(home) }
    }

    fn home_dir(&self) -> Option<PathBuf> {
        self.home
            .clone()
            .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
    }

    fn enabled_extensions(&self) -> Vec<String> {
        let schema_exists = gio::SettingsSchemaSource::default()
            .and_then(|src| src.lookup("org.gnome.shell", true))
            .is_some();
        if !schema_exists {
            return Vec::new();
        }
        let s = gio::Settings::new("org.gnome.shell");
        s.strv("enabled-extensions")
            .into_iter()
            .map(|g| g.to_string())
            .collect()
    }

    fn legacy_gtk_theme_set(&self) -> Option<String> {
        let schema_exists = gio::SettingsSchemaSource::default()
            .and_then(|src| src.lookup("org.gnome.desktop.interface", true))
            .is_some();
        if !schema_exists {
            return None;
        }
        let s = gio::Settings::new("org.gnome.desktop.interface");
        let name = s.string("gtk-theme").to_string();
        // Adwaita variants are the GNOME default; anything else is a
        // user-set theme we didn't install.
        if name.is_empty()
            || name == "Adwaita"
            || name == "Adwaita-dark"
            || name == "Default"
            || name == "HighContrast"
            || name == "HighContrastInverse"
        {
            None
        } else {
            Some(name)
        }
    }

    fn unmanaged_gtk_css(&self, subdir: &str) -> Option<PathBuf> {
        let home = self.home_dir()?;
        let path = home.join(".config").join(subdir).join("gtk.css");
        if !path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(&path).ok()?;
        if content.contains("GNOME X") {
            // Managed by us — not a conflict.
            None
        } else if content.trim().is_empty() {
            // Empty file, user probably touched it once; not a conflict.
            None
        } else {
            Some(path)
        }
    }
}

impl Default for GioThemingConflictDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl ThemingConflictDetector for GioThemingConflictDetector {
    fn detect(&self) -> Vec<ConflictReport> {
        let mut reports = Vec::new();

        let extensions = self.enabled_extensions();
        for (uuid_fragment, kind, description, recommendation) in KNOWN_EXTENSION_CONFLICTS {
            if extensions.iter().any(|uuid| uuid.contains(uuid_fragment)) {
                reports.push(ConflictReport::new(*kind, *description, *recommendation));
            }
        }

        if let Some(legacy) = self.legacy_gtk_theme_set() {
            reports.push(ConflictReport::new(
                ConflictKind::LegacyGtkTheme,
                format!(
                    "org.gnome.desktop.interface/gtk-theme is set to '{legacy}', which overrides the theme name GNOME X applies.",
                ),
                "Reset with: gsettings reset org.gnome.desktop.interface gtk-theme",
            ));
        }

        for subdir in ["gtk-4.0", "gtk-3.0"] {
            if let Some(path) = self.unmanaged_gtk_css(subdir) {
                reports.push(ConflictReport::new(
                    ConflictKind::UnmanagedGtkCss,
                    format!(
                        "Hand-edited `{}` exists without a GNOME X managed-region marker. Applying a theme will overwrite it (a .bak backup is created automatically).",
                        path.display(),
                    ),
                    "Review the file before applying, or rename it if you want to keep the customisation separate.",
                ));
            }
        }

        reports
    }
}

/// UUIDs we match against `enabled-extensions` (substring match, since
/// the canonical UUID includes an author suffix like `@aunetx`).
const KNOWN_EXTENSION_CONFLICTS: &[(&str, ConflictKind, &str, &str)] = &[
    (
        "user-theme",
        ConflictKind::UserThemes,
        "User Themes extension is active; it reads org.gnome.shell.extensions.user-theme/name and may apply a shell theme on top of the one GNOME X wrote.",
        "Either let User Themes drive shell theming (leave its 'name' field set to a theme you installed via GNOME X) or disable the extension.",
    ),
    (
        "blur-my-shell",
        ConflictKind::BlurMyShell,
        "Blur My Shell applies its own panel/overview/dash blur on top of GNOME X's accent-tinted surfaces.",
        "Both can coexist, but the blur may dim or desaturate our tint. Reduce Blur My Shell's opacity if the blend looks muddy.",
    ),
    (
        "dash-to-dock",
        ConflictKind::DashToDock,
        "Dash to Dock replaces the overview dash entirely; our `#dash` tint does not reach Dash to Dock's widget tree.",
        "Configure the dock's own background colour in its preferences — or disable Dash to Dock and use our tinted dash.",
    ),
    (
        "dash-to-panel",
        ConflictKind::DashToPanel,
        "Dash to Panel replaces the top panel with its own widget tree; our `#panel` tint is bypassed.",
        "Configure Dash to Panel's background colour in its preferences — or disable it to let our panel tint take effect.",
    ),
    (
        "nightthemeswitcher",
        ConflictKind::NightThemeSwitcher,
        "Night Theme Switcher flips org.gnome.desktop.interface/color-scheme on a schedule, which can revert the light/dark variant GNOME X just applied.",
        "Let Night Theme Switcher drive scheduling and keep GNOME X for the theme content, or disable the extension and use the built-in GNOME dark-mode scheduling.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn unmanaged_gtk_css_flagged_when_marker_absent() {
        let tmp = tempdir();
        let gtk4 = tmp.join(".config").join("gtk-4.0");
        fs::create_dir_all(&gtk4).unwrap();
        fs::write(gtk4.join("gtk.css"), "button { color: red; }").unwrap();

        let detector = GioThemingConflictDetector::with_home(tmp.clone());
        let result = detector.unmanaged_gtk_css("gtk-4.0");
        assert!(result.is_some(), "user-authored gtk.css must flag");
    }

    #[test]
    fn unmanaged_gtk_css_ignored_when_our_marker_present() {
        let tmp = tempdir();
        let gtk4 = tmp.join(".config").join("gtk-4.0");
        fs::create_dir_all(&gtk4).unwrap();
        fs::write(gtk4.join("gtk.css"), "/* GNOME X — GTK4 overrides */\n").unwrap();

        let detector = GioThemingConflictDetector::with_home(tmp.clone());
        let result = detector.unmanaged_gtk_css("gtk-4.0");
        assert!(result.is_none(), "GNOME X-managed gtk.css must not flag");
    }

    #[test]
    fn unmanaged_gtk_css_ignored_when_file_missing() {
        let tmp = tempdir();
        let detector = GioThemingConflictDetector::with_home(tmp);
        assert!(detector.unmanaged_gtk_css("gtk-4.0").is_none());
    }

    #[test]
    fn unmanaged_gtk_css_ignored_when_file_empty() {
        let tmp = tempdir();
        let gtk3 = tmp.join(".config").join("gtk-3.0");
        fs::create_dir_all(&gtk3).unwrap();
        fs::write(gtk3.join("gtk.css"), "   \n\n").unwrap();

        let detector = GioThemingConflictDetector::with_home(tmp);
        assert!(detector.unmanaged_gtk_css("gtk-3.0").is_none());
    }

    fn tempdir() -> PathBuf {
        let id = uuid::Uuid::new_v4();
        let path = std::env::temp_dir().join(format!("gnomex-conflict-test-{id}"));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
