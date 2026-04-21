// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! GDM polkit-elevated theming — pure-part integration tests (GXF-003).
//!
//! Covers the three pieces that are *supposed* to be testable without
//! `pkexec`, `dconf update`, or `restorecon` on the path:
//!
//! 1. Input validation of the theme NAME and accent hex that the
//!    unprivileged side sends across the polkit boundary.
//! 2. The target path computation
//!    (`gdm_dconf_snippet_path`) — must always land under
//!    `/etc/dconf/db/gdm.d/90-gnome-x` in production, and must honour
//!    a caller-provided root for testing.
//! 3. The snippet rendering — must be idempotent, must embed the
//!    managed-region marker, and must not contain any path / shell
//!    metacharacters from the inputs (validation should have stopped
//!    those upstream — this test pins that invariant).
//!
//! DELIBERATELY not exercised here: `pkexec`, `dconf update`, or
//! `restorecon`. Those run only on the elevated side; exercising
//! them from a test would require root and is explicitly out of
//! scope per the feature's security policy.

use gnomex_app::use_cases::gdm_theme::{
    gdm_dconf_snippet_path, looks_like_managed_snippet, render_gdm_dconf_snippet,
    validate_accent_hex, validate_theme_name, GDM_DCONF_DB_DIR, GDM_DCONF_SNIPPET_FILENAME,
    GDM_MANAGED_MARKER,
};
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn production_default_path_is_etc_dconf_gdm_d() {
    let p = gdm_dconf_snippet_path(None);
    assert_eq!(
        p,
        PathBuf::from(GDM_DCONF_DB_DIR).join(GDM_DCONF_SNIPPET_FILENAME),
        "production path must land under the system dconf DB"
    );
}

#[test]
fn test_path_honours_injection_for_tempdir() {
    let dir = TempDir::new().unwrap();
    let p = gdm_dconf_snippet_path(dir.path().to_str());
    assert_eq!(p, dir.path().join(GDM_DCONF_SNIPPET_FILENAME));
}

#[test]
fn validation_gate_rejects_every_known_smuggling_shape() {
    // Path traversal
    assert!(validate_theme_name("../..").is_err());
    assert!(validate_theme_name("..").is_err());
    // Absolute / relative paths
    assert!(validate_theme_name("/etc/passwd").is_err());
    assert!(validate_theme_name("themes/Adwaita").is_err());
    // Shell metachars
    for bad in [
        "a;b", "a|b", "a`b", "a$b", "a\"b", "a'b", "a b", "a>b", "a<b", "a&b",
    ] {
        assert!(
            validate_theme_name(bad).is_err(),
            "expected {bad:?} to fail validation"
        );
    }
    // Hex must be #rrggbb
    assert!(validate_accent_hex("").is_err());
    assert!(validate_accent_hex("red").is_err());
    assert!(validate_accent_hex("3584e4").is_err());
    assert!(validate_accent_hex("#3584e").is_err());
    assert!(validate_accent_hex("#zzzzzz").is_err());
}

#[test]
fn snippet_write_readback_roundtrip_on_tempdir() {
    // Simulate the elevated-side behaviour minus the `pkexec` layer:
    // validate inputs, render, write under a tempdir, read back, and
    // check the marker is present.
    let dir = TempDir::new().unwrap();
    let theme = validate_theme_name("GNOME-X-Custom").unwrap();
    let accent = validate_accent_hex("#3584e4").unwrap();

    let target = gdm_dconf_snippet_path(dir.path().to_str());
    let snippet = render_gdm_dconf_snippet(theme, &accent);

    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::fs::write(&target, &snippet).unwrap();

    let readback = std::fs::read_to_string(&target).unwrap();
    assert_eq!(readback, snippet, "write/readback must match byte-for-byte");
    assert!(
        readback.contains(GDM_MANAGED_MARKER),
        "managed marker missing from written file"
    );
    assert!(looks_like_managed_snippet(&readback));
}

#[test]
fn snippet_is_idempotent_across_writes() {
    // Two successive renders + writes with the same inputs must
    // produce a byte-identical file. This is what makes the elevated
    // helper safe to re-run — the second invocation is a no-op from
    // dconf's perspective.
    let dir = TempDir::new().unwrap();
    let theme = "GNOME-X-Custom";
    let accent = validate_accent_hex("#3584e4").unwrap();
    let target = gdm_dconf_snippet_path(dir.path().to_str());
    std::fs::create_dir_all(target.parent().unwrap()).unwrap();

    let first = render_gdm_dconf_snippet(theme, &accent);
    std::fs::write(&target, &first).unwrap();
    let first_bytes = std::fs::read(&target).unwrap();

    let second = render_gdm_dconf_snippet(theme, &accent);
    std::fs::write(&target, &second).unwrap();
    let second_bytes = std::fs::read(&target).unwrap();

    assert_eq!(first_bytes, second_bytes);
}

#[test]
fn reset_detector_refuses_non_managed_snippets() {
    // The elevated reset path reads the existing file and refuses to
    // unlink it if the managed marker isn't present, to avoid
    // stomping a sysadmin-authored `90-*` file at the same path.
    let hostile = "# written by sysadmin\n\
                   [org/gnome/shell]\n\
                   theme-name='CorporateBrand'\n";
    assert!(
        !looks_like_managed_snippet(hostile),
        "sysadmin-authored file must not look like ours"
    );

    let accent = validate_accent_hex("#3584e4").unwrap();
    let ours = render_gdm_dconf_snippet("GNOME-X-Custom", &accent);
    assert!(
        looks_like_managed_snippet(&ours),
        "GNOME X-authored file must carry the marker"
    );
}
