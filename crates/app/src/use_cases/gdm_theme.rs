// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! GXF-003 — GDM (login-screen) theming via polkit-elevated helper.
//!
//! This module is **pure** — no I/O, no subprocesses, no GSettings.
//! It holds the three things we want unit-testable without touching
//! a real polkit / dconf / filesystem:
//!
//! 1. Input validation (`validate_theme_name`, `validate_accent_hex`).
//!    The main app feeds these before ever invoking `pkexec`; the
//!    elevated helper ALSO feeds them to harden against a compromised
//!    caller. Both sides rejecting the same set of inputs is the
//!    point — don't trust the caller twice.
//! 2. The target dconf-snippet *path* (`gdm_dconf_snippet_path`) —
//!    a `Path` on Fedora/Ubuntu/Arch defaults to
//!    `/etc/dconf/db/gdm.d/90-gnome-x`.
//! 3. The snippet *contents* (`render_gdm_dconf_snippet`) — a small
//!    INI-ish text blob with the managed-region `"GNOME X"` marker
//!    so we can recognise our own file on a subsequent run.
//!
//! The impure pieces (shelling out to `dconf update`, `restorecon`,
//! writing the file with root permissions) live in the infrastructure
//! helper (CLI subcommand on `experiencectl`) and the `PkexecGdmThemer`
//! adapter.

use crate::AppError;
use gnomex_domain::HexColor;
use std::path::PathBuf;

/// The dconf.d snippet path the elevated helper writes to on
/// Fedora / Ubuntu / Arch. The `90-` prefix orders it after
/// distro-shipped defaults so our overrides win.
pub const GDM_DCONF_SNIPPET_FILENAME: &str = "90-gnome-x";

/// The dconf database directory we target. All files we write under
/// this path must carry the `GNOME X` marker (see
/// [`render_gdm_dconf_snippet`]) so `reset` can distinguish our
/// files from distro / sysadmin-managed ones.
pub const GDM_DCONF_DB_DIR: &str = "/etc/dconf/db/gdm.d";

/// The managed-region marker embedded in every snippet we author.
/// Used by the reset path to decide whether a file at our target
/// path is safe to remove.
pub const GDM_MANAGED_MARKER: &str = "GNOME X";

/// Maximum length for a user-supplied theme name. Keeps us well
/// under any distro's path-length limits and prevents a caller from
/// stuffing an unbounded blob into an argv slot.
pub const MAX_THEME_NAME_LEN: usize = 64;

/// Validate a shell-theme name before it reaches the elevated helper.
///
/// Accepts only `[A-Za-z0-9._-]` with at most [`MAX_THEME_NAME_LEN`]
/// characters. Rejects anything that could be:
/// - a path (contains `/` or `\`)
/// - a path traversal (`..`)
/// - a shell metacharacter (`;`, `$`, `` ` ``, `"`, `'`, `|`, `&`, `>`, `<`, `*`, `?`, whitespace)
/// - an empty string
///
/// Both the UI layer (pre-pkexec) and the elevated helper
/// (post-pkexec) call this. Re-validating on the trusted side is the
/// point — a compromised/broken caller cannot smuggle a path through.
pub fn validate_theme_name(name: &str) -> Result<&str, AppError> {
    if name.is_empty() {
        return Err(AppError::Settings("gdm theme name is empty".into()));
    }
    if name.len() > MAX_THEME_NAME_LEN {
        return Err(AppError::Settings(format!(
            "gdm theme name too long ({} > {MAX_THEME_NAME_LEN})",
            name.len()
        )));
    }
    if name.contains("..") {
        return Err(AppError::Settings(
            "gdm theme name contains path traversal".into(),
        ));
    }
    for ch in name.chars() {
        let ok = ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-');
        if !ok {
            return Err(AppError::Settings(format!(
                "gdm theme name contains disallowed character: {ch:?}"
            )));
        }
    }
    Ok(name)
}

/// Validate an accent hex. Delegates to `HexColor::new`, which
/// already enforces `#rrggbb` with lowercased hex digits.
///
/// Wrapped here so callers can pass raw strings at argv boundaries
/// without importing domain types.
pub fn validate_accent_hex(raw: &str) -> Result<HexColor, AppError> {
    HexColor::new(raw).map_err(AppError::from)
}

/// Build the absolute path to our dconf.d snippet file for GDM.
///
/// The default is `/etc/dconf/db/gdm.d/90-gnome-x` which matches the
/// Fedora / Ubuntu / Arch default dconf-db layout for GDM.
/// `root` lets tests point at a temp directory without touching real
/// system paths.
pub fn gdm_dconf_snippet_path(root: Option<&str>) -> PathBuf {
    let dir = root.map(PathBuf::from).unwrap_or_else(|| PathBuf::from(GDM_DCONF_DB_DIR));
    dir.join(GDM_DCONF_SNIPPET_FILENAME)
}

/// Render the dconf snippet applied to the GDM user's DB.
///
/// We keep the scope intentionally narrow in v1:
/// - the **shell theme NAME** at `/org/gnome/shell/theme-name`
/// - the **accent color NAME** at `/org/gnome/desktop/interface/accent-color`
///   (we map the hex back to the nearest GNOME accent name, because
///   GDM's GNOME Shell reads a name-string, not an arbitrary hex)
/// - a **colour-scheme hint** (`prefer-dark`) so the login screen
///   matches the user's current setting
///
/// The literal `"GNOME X"` substring is present in the header
/// comment as our managed-region marker. Do not remove it — the
/// reset path uses it to decide whether a pre-existing file is
/// safe to delete.
pub fn render_gdm_dconf_snippet(theme_name: &str, accent: &HexColor) -> String {
    let accent_name = nearest_accent_name(accent);
    format!(
        "# GNOME X — managed GDM overrides. Do not edit by hand.\n\
         # This file is regenerated every time the user applies a theme\n\
         # with \"Also apply to login screen\" enabled. Delete it (and\n\
         # re-run `dconf update`) to revert to the distro default.\n\
         \n\
         [org/gnome/shell]\n\
         theme-name='{theme_name}'\n\
         \n\
         [org/gnome/desktop/interface]\n\
         accent-color='{accent_name}'\n\
         color-scheme='prefer-dark'\n"
    )
}

/// Is the given file content something we wrote? (Used by the
/// elevated reset path to avoid stomping distro-authored or
/// sysadmin-authored `90-*` files at the same path.)
pub fn looks_like_managed_snippet(content: &str) -> bool {
    content.contains(GDM_MANAGED_MARKER)
}

/// Map an accent hex to the nearest GNOME accent NAME that Mutter /
/// GNOME Shell accepts in `accent-color`. We intentionally don't
/// try to set an arbitrary hex — GDM's Shell ships the stock palette
/// and rejects unknown names, so the pragmatic v1 is "pick the
/// closest stock swatch".
fn nearest_accent_name(hex: &HexColor) -> &'static str {
    let (r, g, b) = hex.to_rgb();
    const PALETTE: &[(&str, u8, u8, u8)] = &[
        ("blue",   0x35, 0x84, 0xe4),
        ("teal",   0x21, 0x90, 0xa4),
        ("green",  0x3a, 0x94, 0x4a),
        ("yellow", 0xc8, 0x88, 0x00),
        ("orange", 0xed, 0x5b, 0x00),
        ("red",    0xe6, 0x2d, 0x42),
        ("pink",   0xd5, 0x61, 0x99),
        ("purple", 0x91, 0x41, 0xac),
        ("slate",  0x6f, 0x83, 0x96),
    ];
    PALETTE
        .iter()
        .min_by_key(|(_, pr, pg, pb)| {
            let dr = (r as i32) - (*pr as i32);
            let dg = (g as i32) - (*pg as i32);
            let db_ = (b as i32) - (*pb as i32);
            dr * dr + dg * dg + db_ * db_
        })
        .map(|(name, ..)| *name)
        .unwrap_or("blue")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_name_accepts_shell_identifier() {
        assert!(validate_theme_name("Adwaita").is_ok());
        assert!(validate_theme_name("GNOME-X-Custom").is_ok());
        assert!(validate_theme_name("my.theme_2024").is_ok());
    }

    #[test]
    fn theme_name_rejects_slashes_and_traversal() {
        assert!(validate_theme_name("../etc/passwd").is_err());
        assert!(validate_theme_name("themes/Adwaita").is_err());
        assert!(validate_theme_name("..").is_err());
        assert!(validate_theme_name("a/b").is_err());
    }

    #[test]
    fn theme_name_rejects_shell_metacharacters() {
        for bad in [
            "a;b", "a$b", "a`b", "a\"b", "a'b", "a|b", "a&b", "a>b", "a<b", "a*b", "a?b", "a b",
        ] {
            assert!(validate_theme_name(bad).is_err(), "{bad} should be rejected");
        }
    }

    #[test]
    fn theme_name_rejects_empty_and_oversize() {
        assert!(validate_theme_name("").is_err());
        let oversized = "a".repeat(MAX_THEME_NAME_LEN + 1);
        assert!(validate_theme_name(&oversized).is_err());
    }

    #[test]
    fn accent_hex_delegates_to_hex_color() {
        assert!(validate_accent_hex("#3584e4").is_ok());
        assert!(validate_accent_hex("not-a-color").is_err());
        assert!(validate_accent_hex("3584e4").is_err()); // missing '#'
    }

    #[test]
    fn snippet_path_defaults_to_etc_dconf_gdm_d() {
        let p = gdm_dconf_snippet_path(None);
        assert_eq!(p, PathBuf::from("/etc/dconf/db/gdm.d/90-gnome-x"));
    }

    #[test]
    fn snippet_path_honours_root_override() {
        let p = gdm_dconf_snippet_path(Some("/tmp/fake-gdm.d"));
        assert_eq!(p, PathBuf::from("/tmp/fake-gdm.d/90-gnome-x"));
    }

    #[test]
    fn snippet_includes_managed_marker() {
        let accent = HexColor::new("#3584e4").unwrap();
        let s = render_gdm_dconf_snippet("GNOME-X-Custom", &accent);
        assert!(s.contains(GDM_MANAGED_MARKER), "missing marker: {s}");
    }

    #[test]
    fn snippet_routes_theme_name_and_accent_name() {
        let accent = HexColor::new("#3584e4").unwrap(); // blue
        let s = render_gdm_dconf_snippet("GNOME-X-Custom", &accent);
        assert!(s.contains("theme-name='GNOME-X-Custom'"));
        assert!(s.contains("accent-color='blue'"));
    }

    #[test]
    fn snippet_maps_off_palette_hex_to_nearest_name() {
        // Bright red-ish should snap to 'red'.
        let accent = HexColor::new("#ee2233").unwrap();
        let s = render_gdm_dconf_snippet("Adwaita", &accent);
        assert!(s.contains("accent-color='red'"), "{s}");

        // Slate-like should snap to 'slate'.
        let accent = HexColor::new("#708090").unwrap();
        let s = render_gdm_dconf_snippet("Adwaita", &accent);
        assert!(s.contains("accent-color='slate'"), "{s}");
    }

    #[test]
    fn snippet_is_stable_across_calls_with_same_inputs() {
        let a = HexColor::new("#3584e4").unwrap();
        let one = render_gdm_dconf_snippet("GNOME-X-Custom", &a);
        let two = render_gdm_dconf_snippet("GNOME-X-Custom", &a);
        assert_eq!(one, two, "same input must produce byte-identical output");
    }

    #[test]
    fn looks_like_managed_snippet_detects_marker() {
        let accent = HexColor::new("#3584e4").unwrap();
        let ours = render_gdm_dconf_snippet("Adwaita", &accent);
        assert!(looks_like_managed_snippet(&ours));
        assert!(!looks_like_managed_snippet(
            "# written by sysadmin\n[org/gnome/shell]\ntheme-name='Custom'\n"
        ));
    }
}
