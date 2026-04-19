// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for shadow detection (GXF-012).
//!
//! Uses `FilesystemInstaller::with_paths` so each test gets its own
//! hermetic tempdir — no shared $HOME/$XDG_* state between parallel
//! tests.

use gnomex_app::ports::{LocalInstaller, ResourceKind};
use gnomex_infra::theme_paths::ResourcePaths;
use gnomex_infra::FilesystemInstaller;
use std::path::PathBuf;
use tempfile::TempDir;

/// Build an installer whose resolver sees only the given tempdir's
/// fake XDG layout. Layout:
///
/// ```text
/// root/.local/share/themes/<themes_user>/       (XDG user)
/// root/.themes/<themes_legacy>/                 (legacy user)
/// root/usr/share/themes/<themes_system>/        (system)
/// root/.local/share/icons/<icons_user>/
/// root/usr/share/icons/<icons_system>/
/// ```
fn setup(
    themes_user: &[&str],
    themes_legacy: &[&str],
    themes_system: &[&str],
    icons_user: &[&str],
    icons_system: &[&str],
) -> (TempDir, FilesystemInstaller) {
    let dir = TempDir::new().expect("tempdir");
    let root = dir.path();

    let mk = |base: &str, name: &str| {
        std::fs::create_dir_all(root.join(base).join(name)).unwrap();
    };
    for n in themes_user {
        mk(".local/share/themes", n);
    }
    for n in themes_legacy {
        mk(".themes", n);
    }
    for n in themes_system {
        mk("usr/share/themes", n);
    }
    for n in icons_user {
        mk(".local/share/icons", n);
    }
    for n in icons_system {
        mk("usr/share/icons", n);
    }

    let paths = ResourcePaths::explicit(
        root,
        root.join(".local/share"),
        vec![root.join("usr/share")],
        root.join(".config"),
    );
    let installer = FilesystemInstaller::with_paths(
        PathBuf::from(root).join(".local/share"),
        paths,
    );
    (dir, installer)
}

#[test]
fn detects_theme_shadowed_across_user_and_system() {
    let (_tmp, installer) = setup(&["Adwaita"], &[], &["Adwaita"], &[], &[]);
    let shadows = installer
        .list_shadowed_resources(ResourceKind::Theme)
        .expect("list");
    assert_eq!(shadows.len(), 1);
    assert_eq!(shadows[0].name, "Adwaita");
    assert_eq!(shadows[0].locations.len(), 2);
    // First entry is user-writable; GTK resolves it first.
    assert!(shadows[0].locations[0].user_writable);
    assert!(!shadows[0].locations[1].user_writable);
}

#[test]
fn detects_three_way_shadow_in_resolution_order() {
    let (_tmp, installer) = setup(&["Nordic"], &["Nordic"], &["Nordic"], &[], &[]);
    let shadows = installer
        .list_shadowed_resources(ResourceKind::Theme)
        .expect("list");
    assert_eq!(shadows.len(), 1);
    assert_eq!(shadows[0].locations.len(), 3);
    // XDG user first, legacy second, system last.
    assert!(shadows[0].locations[0].path.contains(".local/share/themes"));
    assert!(shadows[0].locations[1].path.contains(".themes"));
    assert!(shadows[0].locations[2].path.contains("usr/share/themes"));
}

#[test]
fn non_shadowed_names_are_not_returned() {
    let (_tmp, installer) = setup(&["OnlyUser"], &[], &["OnlyOnSystem"], &[], &[]);
    let shadows = installer
        .list_shadowed_resources(ResourceKind::Theme)
        .expect("list");
    assert!(
        shadows.is_empty(),
        "expected no shadows when each name appears in one path: {:?}",
        shadows
    );
}

#[test]
fn empty_filesystem_returns_empty_shadow_list() {
    let (_tmp, installer) = setup(&[], &[], &[], &[], &[]);
    let shadows = installer
        .list_shadowed_resources(ResourceKind::Theme)
        .expect("list");
    assert!(shadows.is_empty());
}

#[test]
fn icon_kind_walks_icon_paths_independently_from_themes() {
    // Themes unshadowed but icons shadowed — make sure the icon query
    // uses the icon paths, not themes.
    let (_tmp, installer) =
        setup(&["UserTheme"], &[], &[], &["Papirus"], &["Papirus"]);

    let theme_shadows = installer
        .list_shadowed_resources(ResourceKind::Theme)
        .expect("themes");
    assert!(theme_shadows.is_empty());

    let icon_shadows = installer
        .list_shadowed_resources(ResourceKind::Icon)
        .expect("icons");
    assert_eq!(icon_shadows.len(), 1);
    assert_eq!(icon_shadows[0].name, "Papirus");
}

#[test]
fn cursor_kind_requires_cursors_subdir_to_count() {
    // Create an icon theme named "Bibata" in two paths, but only the
    // system copy has a cursors/ subdir. It should still count as a
    // cursor because at least one path qualifies, and the shadow list
    // carries both locations so the UI can show the collision.
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(
        root.join(".local/share/icons/Bibata"),
    )
    .unwrap();
    std::fs::create_dir_all(
        root.join("usr/share/icons/Bibata/cursors"),
    )
    .unwrap();

    let paths = ResourcePaths::explicit(
        root,
        root.join(".local/share"),
        vec![root.join("usr/share")],
        root.join(".config"),
    );
    let installer = FilesystemInstaller::with_paths(
        PathBuf::from(root).join(".local/share"),
        paths,
    );

    let cursor_shadows = installer
        .list_shadowed_resources(ResourceKind::Cursor)
        .expect("cursors");
    assert_eq!(cursor_shadows.len(), 1);
    assert_eq!(cursor_shadows[0].name, "Bibata");
}
