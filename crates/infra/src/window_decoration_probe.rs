// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Pragmatic SSD/CSD probe for running top-level windows.
//!
//! There is no single clean cross-session API for "does this window
//! draw its own decorations or let the WM do it?". GTK+Adwaita apps
//! decorate themselves; legacy GTK 3 dialogs, Electron without
//! `--enable-features=WaylandWindowDecorations`, Wine/Proton, and a
//! handful of Qt apps without a GNOME platform plugin decorate
//! server-side. Live on GNOME Wayland, the honest query would be
//! `global.get_window_actors()` inside a Shell extension — and we do
//! ship one, but not for this purpose.
//!
//! The MVP here uses the most portable signal available:
//!
//! 1. Run `wmctrl -lx` (X11/XWayland) to enumerate top-level windows
//!    with their `WM_CLASS`. Most legacy SSD apps (Wine, Steam,
//!    Electron without Wayland deco) run on XWayland and are visible.
//! 2. Classify each `WM_CLASS` against two curated heuristic lists:
//!    - Known-CSD toolkits: `org.gnome.*`, `Adwaita`, `io.github.*`,
//!      generic GTK/Libadwaita app-IDs.
//!    - Known-SSD toolkits: Electron apps (Slack, Discord, VS Code
//!      without `--ozone-platform=wayland`), Wine, Steam, Java/AWT.
//!
//! When `wmctrl` isn't installed or the classification heuristic
//! returns `Unknown`, we degrade gracefully: an empty report or a
//! report with `Unknown` entries. The UI treats missing data as
//! "no SSD signal, don't show the banner".
//!
//! **Trade-off acknowledged:** this is a heuristic; false negatives
//! are expected (apps with unusual `WM_CLASS`). False positives
//! (flagging a CSD app as SSD) are avoided by being conservative —
//! we only mark `Ssd` when we have strong signal.
//!
//! Testing: [`probe_with_runner`] accepts an injectable window-list
//! source so the classification logic is fully unit-testable without
//! spawning subprocesses.

use gnomex_app::ports::WindowDecorationProbe;
use gnomex_domain::{DecorationMode, DecorationReport, WindowDecorationInfo};
use std::process::Command;

/// Single raw window record as observed from the windowing system.
/// De-coupled from any particular command so the classifier is
/// testable without process I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawWindow {
    pub app_class: String,
    pub title: Option<String>,
}

/// Concrete probe that shells out to `wmctrl -lx`. Cheap to construct.
///
/// When `wmctrl` is not on `$PATH` (pure-Wayland session with no
/// XWayland legacy fallback installed), `detect_decoration_mix`
/// returns an empty report and logs at `debug` — the UI will simply
/// not show the banner.
pub struct WmctrlDecorationProbe;

impl WmctrlDecorationProbe {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WmctrlDecorationProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowDecorationProbe for WmctrlDecorationProbe {
    fn detect_decoration_mix(&self) -> DecorationReport {
        match run_wmctrl() {
            Ok(raws) => probe_with_runner(&raws),
            Err(e) => {
                tracing::debug!(
                    "wmctrl decoration probe unavailable ({e}); returning empty report",
                );
                DecorationReport::default()
            }
        }
    }
}

/// Classification entry point — pure function over the raw window
/// list. Public so tests can feed synthetic input.
pub fn probe_with_runner(raws: &[RawWindow]) -> DecorationReport {
    let windows = raws
        .iter()
        .map(|raw| WindowDecorationInfo {
            app_class: raw.app_class.clone(),
            title: raw.title.clone(),
            mode: classify(&raw.app_class),
        })
        .collect();
    DecorationReport { windows }
}

/// Classify a single `WM_CLASS` / app-id. Conservative: returns
/// `Unknown` rather than guessing when the class doesn't match either
/// curated list.
fn classify(app_class: &str) -> DecorationMode {
    let lower = app_class.to_ascii_lowercase();

    // SSD signals — these apps paint their own window chrome via the
    // WM. Matched before CSD because some (e.g. "code" for VSCode)
    // also look generic.
    for needle in SSD_APP_CLASS_SUBSTRINGS {
        if lower.contains(needle) {
            return DecorationMode::Ssd;
        }
    }

    // CSD signals — GNOME / Libadwaita app-ID prefixes are a strong
    // signal. GTK4 apps using reverse-DNS IDs dominate this bucket.
    for prefix in CSD_APP_CLASS_PREFIXES {
        if lower.starts_with(prefix) {
            return DecorationMode::Csd;
        }
    }
    for needle in CSD_APP_CLASS_SUBSTRINGS {
        if lower.contains(needle) {
            return DecorationMode::Csd;
        }
    }

    DecorationMode::Unknown
}

/// Substrings in `WM_CLASS` (lowercased) that indicate server-side
/// decoration. Kept conservative — we'd rather admit "unknown" than
/// falsely warn the user that a CSD app won't theme.
const SSD_APP_CLASS_SUBSTRINGS: &[&str] = &[
    // Electron apps that don't opt into Wayland CSD.
    "slack",
    "discord",
    "signal",
    "obsidian",
    "spotify",
    // Wine / Proton virtual windows.
    "wine",
    "explorer.exe",
    // Steam client.
    "steam",
    // Java AWT/Swing legacy.
    "sun-awt",
    // XTerm family (classic SSD terminal emulators).
    "xterm",
    "uxterm",
];

/// Prefixes in app-ID (lowercased) strongly indicating a GNOME / GTK4
/// / Libadwaita CSD app.
const CSD_APP_CLASS_PREFIXES: &[&str] = &[
    "org.gnome.",
    "io.github.gnomex.",
    "io.github.",
    "org.gtk.",
    "org.freedesktop.",
    "net.nokyan.",   // Resources
    "com.belmoussaoui.",
    "app.drey.",
    "de.haeckerfelix.",
];

/// Free-form substrings that also indicate CSD — catches common
/// app-IDs that don't fit a prefix.
const CSD_APP_CLASS_SUBSTRINGS: &[&str] = &[
    "nautilus",
    "gnome-text-editor",
    "gnome-terminal",
    "gnome-calculator",
    "gnome-weather",
    "gnome-clocks",
    "gnome-maps",
    "gnome-control-center",
    "gedit",
    "epiphany",
    "evince",
    "eog",
    "gnome-software",
];

/// Spawn `wmctrl -lx` and parse its stdout into [`RawWindow`]s. The
/// command emits one line per window:
///
/// ```text
/// 0x04200003 -1 org.gnome.Nautilus.Org.gnome.Nautilus hostname Files
/// ```
///
/// We want columns 3 (`WM_CLASS`) and 5+ (title).
fn run_wmctrl() -> Result<Vec<RawWindow>, String> {
    let output = Command::new("wmctrl")
        .arg("-lx")
        .output()
        .map_err(|e| format!("spawn failed: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "wmctrl exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_wmctrl_output(&stdout))
}

/// Pure parser — split out for testability. wmctrl pads columns with
/// variable whitespace, so we tokenise via `split_whitespace` and
/// take the first four tokens as the fixed-width prefix (id, desktop,
/// WM_CLASS, hostname). Everything after the hostname is the title.
pub fn parse_wmctrl_output(stdout: &str) -> Vec<RawWindow> {
    stdout
        .lines()
        .filter_map(|line| {
            if line.trim().is_empty() {
                return None;
            }
            let cols: Vec<&str> = line.split_whitespace().collect();
            if cols.len() < 4 {
                return None;
            }
            let wm_class = cols[2].to_string();
            if wm_class.is_empty() {
                return None;
            }
            let title = if cols.len() > 4 {
                Some(cols[4..].join(" "))
            } else {
                None
            }
            .filter(|s| !s.is_empty());
            Some(RawWindow {
                app_class: wm_class,
                title,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_csd_apps() {
        assert_eq!(classify("org.gnome.Nautilus.Org.gnome.Nautilus"), DecorationMode::Csd);
        assert_eq!(classify("io.github.gnomex.GnomeX.io.github.gnomex.GnomeX"), DecorationMode::Csd);
        assert_eq!(classify("gedit.Gedit"), DecorationMode::Csd);
    }

    #[test]
    fn classifies_known_ssd_apps() {
        assert_eq!(classify("slack.Slack"), DecorationMode::Ssd);
        assert_eq!(classify("steam.Steam"), DecorationMode::Ssd);
        assert_eq!(classify("xterm.XTerm"), DecorationMode::Ssd);
        assert_eq!(classify("discord.discord"), DecorationMode::Ssd);
    }

    #[test]
    fn classifies_unknown_toolkits_as_unknown() {
        // Conservative: we don't know, so we don't falsely warn.
        assert_eq!(classify("some-mystery-app.Whatever"), DecorationMode::Unknown);
        assert_eq!(classify(""), DecorationMode::Unknown);
    }

    #[test]
    fn probe_with_runner_produces_expected_report() {
        let raws = vec![
            RawWindow {
                app_class: "org.gnome.Nautilus.Org.gnome.Nautilus".into(),
                title: Some("Files".into()),
            },
            RawWindow {
                app_class: "slack.Slack".into(),
                title: Some("Slack | General".into()),
            },
            RawWindow {
                app_class: "mystery.Unknown".into(),
                title: None,
            },
        ];
        let report = probe_with_runner(&raws);
        assert_eq!(report.csd_count(), 1);
        assert_eq!(report.ssd_count(), 1);
        assert_eq!(report.unknown_count(), 1);
        assert!(report.has_ssd_windows());
        assert_eq!(report.ssd_app_classes(), vec!["slack.Slack".to_string()]);
    }

    #[test]
    fn parse_wmctrl_output_extracts_class_and_title() {
        let sample = "0x04200003 -1 org.gnome.Nautilus.Org.gnome.Nautilus hostname Files\n\
                      0x04400004  0 slack.Slack  hostname Slack | General\n";
        let raws = parse_wmctrl_output(sample);
        assert_eq!(raws.len(), 2);
        assert_eq!(raws[0].app_class, "org.gnome.Nautilus.Org.gnome.Nautilus");
        assert_eq!(raws[0].title.as_deref(), Some("Files"));
        assert_eq!(raws[1].app_class, "slack.Slack");
    }

    #[test]
    fn parse_wmctrl_output_tolerates_missing_fields() {
        // Windows with no title — wmctrl emits just four fields.
        let sample = "0x04200003 -1 foo.Foo hostname";
        let raws = parse_wmctrl_output(sample);
        assert_eq!(raws.len(), 1);
        assert_eq!(raws[0].app_class, "foo.Foo");
        assert!(raws[0].title.is_none());
    }

    #[test]
    fn parse_wmctrl_output_skips_blank_lines() {
        let sample = "\n0x04200003 -1 org.gnome.Files.Org.gnome.Files host Files\n\n";
        let raws = parse_wmctrl_output(sample);
        assert_eq!(raws.len(), 1);
    }
}
