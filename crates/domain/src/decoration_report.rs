// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Window decoration probe reports — classifies currently-open windows
//! as CSD (client-side decorated, e.g. Libadwaita) vs SSD (server-side
//! decorated, e.g. legacy GTK 3, Electron without Wayland decoration,
//! Wine/Proton).
//!
//! Motivation: GNOME X's theme builder exposes controls like headerbar
//! height, window corner radius, and CSD drop-shadow. **Those controls
//! only reach apps that draw their own decorations.** SSD apps look
//! unthemed no matter what — the window manager draws their titlebar
//! using Mutter's own rules. Surfacing the mix lets users know which
//! apps will and won't pick up the theme.
//!
//! See GXF-021. This is a *snapshot* report — probed once when the
//! user opens the Theme Builder or Diagnostics view, not a live feed.

/// How a single window is decorated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecorationMode {
    /// Client-side decorations: the app draws its own titlebar.
    /// GTK4 / Libadwaita / GTK3 with `AdwHeaderBar` all land here.
    /// GNOME X's theme builder **does** reach these apps.
    Csd,
    /// Server-side decorations: Mutter draws the titlebar. Legacy GTK
    /// 3 dialogs, Electron without `--ozone-platform-hint=auto` +
    /// Wayland, Wine/Proton, some Qt apps without a GNOME platform
    /// plugin. GNOME X's theme builder **does not** reach these.
    Ssd,
    /// Probe couldn't classify the window — toolkit unrecognised or
    /// missing API access. Counted separately so we don't lie about
    /// coverage.
    Unknown,
}

impl DecorationMode {
    pub fn label(&self) -> &'static str {
        match self {
            DecorationMode::Csd => "CSD",
            DecorationMode::Ssd => "SSD",
            DecorationMode::Unknown => "Unknown",
        }
    }
}

/// One window observed by the probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowDecorationInfo {
    /// Window class / app-id as reported by the windowing system.
    /// E.g. `"org.gnome.Nautilus"`, `"slack"`, `"Code"`.
    pub app_class: String,
    /// Human-readable window title at probe time. Best-effort — some
    /// windowing backends don't expose this.
    pub title: Option<String>,
    pub mode: DecorationMode,
}

/// Aggregate report. `windows` is unsorted; consumers that render a
/// list should sort by `mode` then `app_class` for stable UI.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DecorationReport {
    pub windows: Vec<WindowDecorationInfo>,
}

impl DecorationReport {
    /// Shorthand: how many SSD windows are open right now? Zero means
    /// "the theme builder reaches everything you have open".
    pub fn ssd_count(&self) -> usize {
        self.windows
            .iter()
            .filter(|w| w.mode == DecorationMode::Ssd)
            .count()
    }

    pub fn csd_count(&self) -> usize {
        self.windows
            .iter()
            .filter(|w| w.mode == DecorationMode::Csd)
            .count()
    }

    pub fn unknown_count(&self) -> usize {
        self.windows
            .iter()
            .filter(|w| w.mode == DecorationMode::Unknown)
            .count()
    }

    /// Distinct SSD app classes in a stable order — the "interesting"
    /// list the banner surfaces ("Slack, Steam will not pick up the
    /// headerbar styling").
    pub fn ssd_app_classes(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .windows
            .iter()
            .filter(|w| w.mode == DecorationMode::Ssd)
            .map(|w| w.app_class.clone())
            .collect();
        out.sort();
        out.dedup();
        out
    }

    /// True when any SSD window is present — the banner's gate.
    pub fn has_ssd_windows(&self) -> bool {
        self.ssd_count() > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn win(class: &str, mode: DecorationMode) -> WindowDecorationInfo {
        WindowDecorationInfo {
            app_class: class.into(),
            title: None,
            mode,
        }
    }

    #[test]
    fn counts_tally_by_mode() {
        let r = DecorationReport {
            windows: vec![
                win("org.gnome.Nautilus", DecorationMode::Csd),
                win("org.gnome.TextEditor", DecorationMode::Csd),
                win("slack", DecorationMode::Ssd),
                win("steam", DecorationMode::Ssd),
                win("weird", DecorationMode::Unknown),
            ],
        };
        assert_eq!(r.csd_count(), 2);
        assert_eq!(r.ssd_count(), 2);
        assert_eq!(r.unknown_count(), 1);
        assert!(r.has_ssd_windows());
    }

    #[test]
    fn ssd_app_classes_are_sorted_and_deduped() {
        let r = DecorationReport {
            windows: vec![
                win("steam", DecorationMode::Ssd),
                win("slack", DecorationMode::Ssd),
                win("slack", DecorationMode::Ssd), // second Slack window
                win("org.gnome.Files", DecorationMode::Csd),
            ],
        };
        assert_eq!(r.ssd_app_classes(), vec!["slack", "steam"]);
    }

    #[test]
    fn empty_report_reports_no_ssd() {
        let r = DecorationReport::default();
        assert!(!r.has_ssd_windows());
        assert_eq!(r.ssd_count(), 0);
        assert_eq!(r.ssd_app_classes(), Vec::<String>::new());
    }
}
