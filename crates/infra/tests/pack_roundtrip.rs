// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for the Experience Pack persistence pipeline.
//!
//! These run the *real* `PackTomlStorage` adapter against a tempdir —
//! no mocks. They catch regressions that unit-level serde tests miss:
//! file layout, atomic writes, round-tripping through the tar.gz
//! export/import path, and schema-version compatibility.

use gnomex_app::ports::PackStorage;
use gnomex_domain::{
    ExperiencePack, PanelPosition, ShellTweak, ShellTweakId, ShellVersion, TweakValue,
};
use gnomex_infra::PackTomlStorage;
use tempfile::TempDir;

fn fixture_pack(id: &str, shell_tweaks: Vec<ShellTweak>) -> ExperiencePack {
    ExperiencePack {
        id: id.into(),
        name: "Round Trip".into(),
        description: "Integration fixture".into(),
        author: "integration".into(),
        created_at: "0".into(),
        shell_version: ShellVersion::new(47, 0),
        pack_format: 2,
        gtk_theme: None,
        shell_theme: None,
        icon_pack: None,
        cursor_pack: None,
        extensions: Vec::new(),
        wallpaper: None,
        gsettings_overrides: Vec::new(),
        shell_tweaks,
    }
}

#[test]
fn save_then_load_preserves_every_tweak_type() {
    let dir = TempDir::new().expect("tempdir");
    let storage = PackTomlStorage::from_dir(dir.path());

    let original = fixture_pack(
        "all-types",
        vec![
            ShellTweak {
                id: ShellTweakId::EnableAnimations,
                value: TweakValue::Bool(true),
            },
            ShellTweak {
                id: ShellTweakId::ClockFormat,
                value: TweakValue::Enum("24h".into()),
            },
            ShellTweak {
                id: ShellTweakId::AppsGridColumns,
                value: TweakValue::Int(6),
            },
            ShellTweak {
                id: ShellTweakId::TopBarPosition,
                value: TweakValue::Position(PanelPosition::Bottom),
            },
        ],
    );

    storage.save_pack(&original).expect("save");
    let loaded = storage.load_pack("all-types").expect("load");

    assert_eq!(loaded.id, original.id);
    assert_eq!(loaded.pack_format, 2);
    assert_eq!(loaded.shell_tweaks, original.shell_tweaks);
}

#[test]
fn pack_toml_file_is_created_on_disk_at_expected_path() {
    let dir = TempDir::new().expect("tempdir");
    let storage = PackTomlStorage::from_dir(dir.path());
    storage
        .save_pack(&fixture_pack("disk-layout", vec![]))
        .expect("save");

    let path = dir.path().join("disk-layout.gnomex-pack.toml");
    assert!(path.exists(), "expected {} to exist", path.display());

    // Quick sanity on the content — TOML header should be there.
    let content = std::fs::read_to_string(&path).expect("read");
    assert!(
        content.contains("[pack]"),
        "expected [pack] section in: {content}"
    );
    assert!(content.contains("id = \"disk-layout\""));
}

#[test]
fn list_packs_returns_every_saved_pack() {
    let dir = TempDir::new().expect("tempdir");
    let storage = PackTomlStorage::from_dir(dir.path());

    storage.save_pack(&fixture_pack("alpha", vec![])).unwrap();
    storage.save_pack(&fixture_pack("beta", vec![])).unwrap();
    storage.save_pack(&fixture_pack("gamma", vec![])).unwrap();

    let summaries = storage.list_packs().expect("list");
    let mut ids: Vec<String> = summaries.into_iter().map(|s| s.id).collect();
    ids.sort();
    assert_eq!(ids, vec!["alpha", "beta", "gamma"]);
}

#[test]
fn export_then_import_round_trips_through_tar_gz() {
    let source_dir = TempDir::new().expect("tempdir");
    let storage = PackTomlStorage::from_dir(source_dir.path());

    let original = fixture_pack(
        "exporter",
        vec![ShellTweak {
            id: ShellTweakId::EnableAnimations,
            value: TweakValue::Bool(false),
        }],
    );
    storage.save_pack(&original).unwrap();

    // Export with a fake screenshot.
    let png = b"FAKE-PNG-BYTES".to_vec();
    let archive = storage
        .export_pack("exporter", Some(&png))
        .expect("export");
    assert!(archive.len() > 100, "archive looks empty: {} bytes", archive.len());

    // Import into a *different* dir to mimic sharing.
    let dest_dir = TempDir::new().expect("tempdir2");
    let dest = PackTomlStorage::from_dir(dest_dir.path());
    let (id, imported_png) = dest.import_pack(&archive).expect("import");
    assert_eq!(id, "exporter");
    assert_eq!(imported_png, Some(png));

    // The imported pack should be loadable from the new dir with the
    // exact same shell-tweak payload.
    let loaded = dest.load_pack("exporter").unwrap();
    assert_eq!(loaded.shell_tweaks, original.shell_tweaks);
}

#[test]
fn v1_toml_on_disk_loads_as_pack_format_1_with_empty_tweaks() {
    // Drop a handwritten pack_format=1 manifest on disk and ensure
    // the loader deserialises it cleanly without shell_tweaks.
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("legacy.gnomex-pack.toml");
    std::fs::write(
        &path,
        r#"
[pack]
id = "legacy"
name = "Legacy"
description = ""
author = "old-tester"
created = "0"
shell_version = "47.0"
pack_format = 1
"#,
    )
    .expect("write legacy toml");

    let storage = PackTomlStorage::from_dir(dir.path());
    let loaded = storage.load_pack("legacy").expect("load v1 pack");
    assert_eq!(loaded.pack_format, 1);
    assert!(loaded.shell_tweaks.is_empty());
    assert!(loaded.validate().is_ok());
}

#[test]
fn delete_pack_removes_toml_from_disk() {
    let dir = TempDir::new().expect("tempdir");
    let storage = PackTomlStorage::from_dir(dir.path());

    storage.save_pack(&fixture_pack("deletable", vec![])).unwrap();
    let path = dir.path().join("deletable.gnomex-pack.toml");
    assert!(path.exists(), "pre-condition");

    storage.delete_pack("deletable").expect("delete");
    assert!(!path.exists(), "expected file removed: {}", path.display());
}
