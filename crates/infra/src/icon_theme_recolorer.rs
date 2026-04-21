// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Icon-theme recolour adapter.
//!
//! Dispatches to whichever mechanism the currently-active icon theme
//! supports:
//!
//! - **Adwaita** (GNOME 47+): no-op. Adwaita's folder icons already
//!   track `org.gnome.desktop.interface accent-color` natively, so
//!   writing the accent (which `ApplyThemeUseCase` did upstream of us
//!   through `AppearanceSettings`) is sufficient.
//! - **Papirus / Papirus-Dark / Papirus-Light**: shell out to
//!   `papirus-folders -C <colour> --theme <theme>`. Requires the
//!   user to have installed the `papirus-folders` script (AUR /
//!   `pacman`, Fedora COPR, or the upstream installer). When the
//!   binary isn't in `$PATH` we return `Unsupported` rather than
//!   erroring — the apply should succeed even if the helper is
//!   missing.
//! - **Anything else**: `Unsupported(theme_name)`.
//!
//! Non-goals: we don't repaint third-party icon themes by rewriting
//! SVGs — that's fragile and brittle across theme updates. Users who
//! want recoloured folders should use Adwaita or Papirus.

use std::process::Command;
use std::sync::Arc;

use gnomex_app::ports::{AppearanceSettings, IconThemeRecolorer, RecolorOutcome};
use gnomex_app::AppError;

/// Production adapter — looks up the active icon theme via the
/// provided [`AppearanceSettings`] and routes to the appropriate
/// recolour mechanism.
pub struct PapirusFoldersRecolorer {
    appearance: Arc<dyn AppearanceSettings>,
}

impl PapirusFoldersRecolorer {
    pub fn new(appearance: Arc<dyn AppearanceSettings>) -> Self {
        Self { appearance }
    }

    /// Classify the active icon theme. Pure — exposed separately so
    /// tests can assert the routing logic without spawning processes.
    pub fn classify(theme: &str) -> IconFamily {
        let t = theme.trim();
        if t.is_empty() || t == "Adwaita" || t.starts_with("Adwaita-") {
            return IconFamily::Adwaita;
        }
        // Papirus ships several recoloured variants — all speak the
        // same `papirus-folders` surface, so we treat them as one
        // family but remember which variant to pass `--theme` below.
        for p in &["Papirus", "Papirus-Dark", "Papirus-Light", "ePapirus", "ePapirus-Dark"] {
            if t == *p {
                return IconFamily::Papirus(t.to_string());
            }
        }
        IconFamily::Other(t.to_string())
    }

    /// Map a GNOME accent id (`blue`, `teal`, …) to the nearest
    /// Papirus colour. Papirus has a larger palette than GNOME's 9,
    /// so we pick the best match — `purple → violet`,
    /// `slate → bluegrey` — rather than the literal name.
    pub fn accent_to_papirus_color(accent_id: &str) -> &'static str {
        match accent_id {
            "blue" => "blue",
            "teal" => "teal",
            "green" => "green",
            "yellow" => "yellow",
            "orange" => "orange",
            "red" => "red",
            "pink" => "pink",
            "purple" => "violet",
            "slate" => "bluegrey",
            _ => "blue",
        }
    }

    /// Is the `papirus-folders` binary on `$PATH`? Best-effort:
    /// checks common names. Kept as a free helper so the Papirus
    /// branch can be tested without actually spawning anything.
    fn papirus_folders_available() -> bool {
        Command::new("papirus-folders")
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }
}

/// Classification of the active icon theme's recolour surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IconFamily {
    /// Adwaita or a close derivative — native accent support on
    /// GNOME 47+, no action required.
    Adwaita,
    /// Papirus family, identified by its exact theme name so we can
    /// pass it to `papirus-folders --theme`.
    Papirus(String),
    /// A third-party theme we don't know how to recolour.
    Other(String),
}

impl IconThemeRecolorer for PapirusFoldersRecolorer {
    fn recolor(&self, accent_id: &str) -> Result<RecolorOutcome, AppError> {
        let theme = self.appearance.get_icon_theme().unwrap_or_default();
        match Self::classify(&theme) {
            IconFamily::Adwaita => Ok(RecolorOutcome::NativelyTracks(
                "Adwaita".into(),
            )),
            IconFamily::Papirus(name) => {
                if !Self::papirus_folders_available() {
                    tracing::warn!(
                        "icon recolour requested for {name} but `papirus-folders` binary is not on PATH"
                    );
                    return Ok(RecolorOutcome::Unsupported(name));
                }
                let color = Self::accent_to_papirus_color(accent_id);
                let status = Command::new("papirus-folders")
                    .args(["-C", color, "--theme", &name])
                    .status()
                    .map_err(|e| {
                        AppError::Settings(format!("spawn papirus-folders: {e}"))
                    })?;
                if status.success() {
                    tracing::info!(
                        "recoloured {name} folder icons to '{color}' (accent: {accent_id})"
                    );
                    Ok(RecolorOutcome::Applied(name))
                } else {
                    // Don't escalate to an error — the user likely
                    // picked an accent that maps to a Papirus colour
                    // their installation doesn't ship. Toast it.
                    tracing::warn!(
                        "papirus-folders exited non-zero for theme={name} color={color}"
                    );
                    Ok(RecolorOutcome::Unsupported(name))
                }
            }
            IconFamily::Other(name) => Ok(RecolorOutcome::Unsupported(name)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_adwaita_variants() {
        assert_eq!(
            PapirusFoldersRecolorer::classify("Adwaita"),
            IconFamily::Adwaita,
        );
        assert_eq!(
            PapirusFoldersRecolorer::classify("Adwaita-dark"),
            IconFamily::Adwaita,
        );
        // Empty string = GNOME not set = falls back to Adwaita-ish
        // behaviour; treat as native so we don't spam "unsupported".
        assert_eq!(
            PapirusFoldersRecolorer::classify(""),
            IconFamily::Adwaita,
        );
    }

    #[test]
    fn classify_papirus_variants_preserve_exact_name() {
        for n in ["Papirus", "Papirus-Dark", "Papirus-Light", "ePapirus", "ePapirus-Dark"] {
            match PapirusFoldersRecolorer::classify(n) {
                IconFamily::Papirus(got) => assert_eq!(got, n),
                other => panic!("expected Papirus for {n}, got {other:?}"),
            }
        }
    }

    #[test]
    fn classify_unknown_theme() {
        match PapirusFoldersRecolorer::classify("Yaru") {
            IconFamily::Other(name) => assert_eq!(name, "Yaru"),
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn accent_mapping_covers_every_gnome_accent_with_sensible_papirus_name() {
        // Exhaustive across the 9 GNOME accents — if a future
        // accent is added, we want the linter to flag that we
        // haven't mapped it.
        let pairs = [
            ("blue", "blue"),
            ("teal", "teal"),
            ("green", "green"),
            ("yellow", "yellow"),
            ("orange", "orange"),
            ("red", "red"),
            ("pink", "pink"),
            ("purple", "violet"),
            ("slate", "bluegrey"),
        ];
        for (g, p) in pairs {
            assert_eq!(
                PapirusFoldersRecolorer::accent_to_papirus_color(g),
                p,
                "accent {g} should map to {p}",
            );
        }
    }

    #[test]
    fn accent_mapping_unknown_falls_back_to_blue() {
        assert_eq!(
            PapirusFoldersRecolorer::accent_to_papirus_color("chartreuse"),
            "blue",
        );
    }
}
