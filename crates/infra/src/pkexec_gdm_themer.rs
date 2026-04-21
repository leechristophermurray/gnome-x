// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! `GdmThemer` adapter that elevates through `pkexec` + the
//! `experiencectl gdm-apply` / `gdm-reset` subcommands.
//!
//! We deliberately pass ONLY a validated theme name and accent hex
//! across the polkit boundary. The CLI subcommand on the other side
//! re-validates both before writing anything to
//! `/etc/dconf/db/gdm.d/`. See
//! `gnomex_app::use_cases::gdm_theme` for the shared validators
//! and the pure snippet generator.

use gnomex_app::ports::GdmThemer;
use gnomex_app::use_cases::gdm_theme;
use gnomex_app::AppError;
use gnomex_domain::HexColor;
use std::process::Command;

/// Elevated-adapter that invokes `pkexec experiencectl gdm-apply ...`.
pub struct PkexecGdmThemer {
    pkexec_binary: String,
    helper_binary: String,
}

impl PkexecGdmThemer {
    pub fn new() -> Self {
        Self {
            pkexec_binary: "pkexec".to_owned(),
            // `experiencectl` must be on PATH for pkexec to locate it
            // via the polkit action's `exec.path`. Both packaged (.rpm/
            // .deb) and install.sh routes drop it at /usr/bin, which
            // matches the polkit action exec.path we ship in
            // data/org.gnomex.gdm-theme.policy.
            helper_binary: "experiencectl".to_owned(),
        }
    }

    #[cfg(test)]
    fn with_binaries(pkexec: &str, helper: &str) -> Self {
        Self {
            pkexec_binary: pkexec.to_owned(),
            helper_binary: helper.to_owned(),
        }
    }
}

impl Default for PkexecGdmThemer {
    fn default() -> Self {
        Self::new()
    }
}

impl GdmThemer for PkexecGdmThemer {
    fn apply(&self, theme_name: &str, accent: &HexColor) -> Result<(), AppError> {
        // Client-side validation — the elevated helper re-validates too.
        let theme = gdm_theme::validate_theme_name(theme_name)?;
        let accent_str = accent.as_str().to_owned();

        tracing::info!(
            "requesting GDM theme apply via pkexec: theme={theme} accent={accent_str}"
        );
        let status = Command::new(&self.pkexec_binary)
            .arg(&self.helper_binary)
            .arg("gdm-apply")
            .arg("--theme")
            .arg(theme)
            .arg("--accent")
            .arg(&accent_str)
            .status()
            .map_err(|e| {
                AppError::Settings(format!("failed to spawn pkexec for gdm-apply: {e}"))
            })?;
        if !status.success() {
            return Err(AppError::Settings(format!(
                "pkexec experiencectl gdm-apply exited with status {status}"
            )));
        }
        Ok(())
    }

    fn reset(&self) -> Result<(), AppError> {
        tracing::info!("requesting GDM theme reset via pkexec");
        let status = Command::new(&self.pkexec_binary)
            .arg(&self.helper_binary)
            .arg("gdm-reset")
            .status()
            .map_err(|e| {
                AppError::Settings(format!("failed to spawn pkexec for gdm-reset: {e}"))
            })?;
        if !status.success() {
            return Err(AppError::Settings(format!(
                "pkexec experiencectl gdm-reset exited with status {status}"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_rejects_bad_theme_name_before_spawning_pkexec() {
        // If validation is working, we never reach Command::spawn and
        // therefore never touch a real pkexec binary. Constructing the
        // themer with bogus binaries proves this: if we did spawn, the
        // test would fail with an ENOENT rather than an AppError::Settings
        // from the validator.
        let themer = PkexecGdmThemer::with_binaries(
            "/definitely/does/not/exist/pkexec",
            "experiencectl",
        );
        let accent = HexColor::new("#3584e4").unwrap();
        let err = themer.apply("../etc/passwd", &accent).unwrap_err();
        assert!(
            matches!(err, AppError::Settings(ref s) if s.contains("path traversal")),
            "expected validation error, got {err:?}"
        );
    }
}
