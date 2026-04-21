// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Hermetic integration test for
//! `FilesystemInstaller::list_shadowed_resources`. Uses
//! [`ResourcePaths::explicit`] + [`FilesystemInstaller::with_paths`]
//! so nothing depends on `$HOME` / `$XDG_*` — safe to run in parallel
//! with every other test in the workspace.
//!
//! See GXF-012.

use gnomex_app::ports::LocalInstaller;
use gnomex_domain::ResourceKind;
use gnomex_infra::theme_paths::ResourcePaths;
use gnomex_infra::FilesystemInstaller;
use std::fs;
use std::path::PathBuf;

fn make_installer(root: &std::path::Path) -> (FilesystemInstaller, PathBuf, PathBuf, PathBuf) {
    // Build a fake XDG layout under `root`:
    //   root/user/.local/share/{themes,icons}   (user-writable)
    //   root/system/share/{themes,icons}        (system)
    let user_data = root.join("user/.local/share");
    let user_legacy_themes = root.join("user/.themes");
    let user_legacy_icons = root.join("user/.icons");
    let sys = root.join("system/share");
    fs::create_dir_all(user_data.join("themes")).unwrap();
    fs::create_dir_all(user_data.join("icons")).unwrap();
    fs::create_dir_all(&user_legacy_themes).unwrap();
    fs::create_dir_all(&user_legacy_icons).unwrap();
    fs::create_dir_all(sys.join("themes")).unwrap();
    fs::create_dir_all(sys.join("icons")).unwrap();
    let paths = ResourcePaths::explicit(
        root.join("user"),
        user_data.clone(),
        vec![sys.clone()],
        root.join("user/.config"),
    );
    let installer = FilesystemInstaller::with_paths(user_data.clone(), paths);
    (installer, user_data, sys, user_legacy_themes)
}

#[test]
fn detects_theme_shadowed_between_user_and_system() {
    let tmp = tempfile::tempdir().unwrap();
    let (installer, user_data, sys, _legacy_themes) = make_installer(tmp.path());

    // "Yaru" installed in both user and system.
    fs::create_dir_all(user_data.join("themes/Yaru")).unwrap();
    fs::create_dir_all(sys.join("themes/Yaru")).unwrap();
    // "OnlyUser" in user only — not shadowed.
    fs::create_dir_all(user_data.join("themes/OnlyUser")).unwrap();

    let shadowed = installer
        .list_shadowed_resources(ResourceKind::Theme)
        .unwrap();
    assert_eq!(shadowed.len(), 1, "only Yaru should be shadowed");
    let yaru = &shadowed[0];
    assert_eq!(yaru.name, "Yaru");
    assert_eq!(yaru.kind, ResourceKind::Theme);
    assert_eq!(yaru.locations.len(), 2);
    // User location wins (first in XDG order).
    let winner = yaru.winning_location();
    assert!(winner.user_writable);
    assert_eq!(winner.path, user_data.join("themes/Yaru"));
    // System location is masked.
    let masked = &yaru.masked_locations()[0];
    assert!(!masked.user_writable);
    assert_eq!(masked.path, sys.join("themes/Yaru"));
}

#[test]
fn returns_empty_when_nothing_is_shadowed() {
    let tmp = tempfile::tempdir().unwrap();
    let (installer, user_data, _sys, _legacy) = make_installer(tmp.path());
    fs::create_dir_all(user_data.join("themes/UniqueTheme")).unwrap();

    let shadowed = installer
        .list_shadowed_resources(ResourceKind::Theme)
        .unwrap();
    assert!(shadowed.is_empty());
}

#[test]
fn detects_icon_pack_shadowed_across_legacy_and_xdg() {
    // The legacy ~/.icons path ought to shadow/mask icons found also
    // under the system /usr/share/icons path. ~/.local/share/icons
    // still wins overall. Verify ordering is XDG user > legacy ~ >
    // system.
    let tmp = tempfile::tempdir().unwrap();
    let (installer, user_data, sys, _legacy_themes) = make_installer(tmp.path());
    let legacy_icons = tmp.path().join("user/.icons");

    fs::create_dir_all(user_data.join("icons/Papirus")).unwrap();
    fs::create_dir_all(legacy_icons.join("Papirus")).unwrap();
    fs::create_dir_all(sys.join("icons/Papirus")).unwrap();

    let shadowed = installer
        .list_shadowed_resources(ResourceKind::Icon)
        .unwrap();
    assert_eq!(shadowed.len(), 1);
    let p = &shadowed[0];
    assert_eq!(p.name, "Papirus");
    assert_eq!(p.locations.len(), 3);
    // Order: XDG user, legacy ~/.icons, system.
    assert_eq!(p.locations[0].path, user_data.join("icons/Papirus"));
    assert_eq!(p.locations[1].path, legacy_icons.join("Papirus"));
    assert_eq!(p.locations[2].path, sys.join("icons/Papirus"));
    assert!(p.locations[0].user_writable);
    assert!(p.locations[1].user_writable);
    assert!(!p.locations[2].user_writable);
}

#[test]
fn cursor_kind_only_returns_entries_with_cursors_subdir() {
    // A name present as an icon-only theme in both locations must NOT
    // show up as a shadowed cursor. Only names with a `cursors/`
    // subdir somewhere should.
    let tmp = tempfile::tempdir().unwrap();
    let (installer, user_data, sys, _legacy) = make_installer(tmp.path());

    // Icon-only: "Papirus" — no cursors subdir.
    fs::create_dir_all(user_data.join("icons/Papirus")).unwrap();
    fs::create_dir_all(sys.join("icons/Papirus")).unwrap();

    // Actual cursor: "Breeze" — has a cursors subdir in both locations.
    fs::create_dir_all(user_data.join("icons/Breeze/cursors")).unwrap();
    fs::create_dir_all(sys.join("icons/Breeze/cursors")).unwrap();

    let shadowed = installer
        .list_shadowed_resources(ResourceKind::Cursor)
        .unwrap();
    assert_eq!(shadowed.len(), 1, "only Breeze is a shadowed cursor");
    assert_eq!(shadowed[0].name, "Breeze");
}
