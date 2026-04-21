// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Hermetic integration tests for `XdgWallpaperSlideshowWriter`.
//!
//! Runs the real adapter against a tempdir — never touches
//! `$HOME`. Verifies:
//!   - XML is written atomically under the target directory
//!   - `<starttime>` reflects the injected clock
//!   - `<static>` / `<transition>` counts and ordering match the spec
//!   - picture paths are absolute and XML-escaped
//!   - delete() cleans up without error when the file is absent

use gnomex_app::ports::WallpaperSlideshowWriter;
use gnomex_domain::{FixedClock, StartTime, WallpaperSlideshow};
use gnomex_infra::wallpaper_slideshow_xml::XdgWallpaperSlideshowWriter;
use std::path::PathBuf;
use tempfile::TempDir;

fn writer(dir: &TempDir) -> XdgWallpaperSlideshowWriter {
    // Epoch clock + deterministic stamp so tests can assert exact
    // filenames. Production injects UNIX seconds to defeat
    // GSettings' same-value write suppression.
    XdgWallpaperSlideshowWriter::with_dir(dir.path())
        .with_clock(Box::new(FixedClock(StartTime::EPOCH)))
        .with_stamp_provider(Box::new(|| "t".into()))
}

fn sample(name: &str, pics: &[&str]) -> WallpaperSlideshow {
    WallpaperSlideshow::new(
        name,
        pics.iter().map(PathBuf::from).collect(),
        600,
        false,
    )
    .unwrap()
}

#[test]
fn write_creates_xml_at_expected_path_with_gnome_format() {
    let dir = TempDir::new().expect("tempdir");
    let w = writer(&dir);
    let s = sample("sunset", &["/srv/p/a.jpg", "/srv/p/b.jpg", "/srv/p/c.jpg"]);

    let path = w.write(&s).expect("write");
    assert_eq!(path, dir.path().join("sunset-t.xml"));
    assert!(path.exists());

    let body = std::fs::read_to_string(&path).expect("read back");
    // XML prolog present — gnome-background-properties needs it.
    assert!(body.starts_with("<?xml"));
    // Root element + required starttime.
    assert!(body.contains("<background>"));
    assert!(body.contains("<starttime>"));
    assert!(body.trim_end().ends_with("</background>"));
    // One static + one transition per picture, wrapping.
    assert_eq!(body.matches("<static>").count(), 3);
    assert_eq!(body.matches("<transition").count(), 3);
}

#[test]
fn write_places_absolute_picture_paths_in_file_elements() {
    let dir = TempDir::new().expect("tempdir");
    let w = writer(&dir);
    let s = sample("abs", &["/etc/foo.jpg", "/var/bar.jpg"]);

    let path = w.write(&s).expect("write");
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("<file>/etc/foo.jpg</file>"));
    assert!(body.contains("<file>/var/bar.jpg</file>"));
    assert!(body.contains("<from>/etc/foo.jpg</from>"));
    assert!(body.contains("<to>/var/bar.jpg</to>"));
}

#[test]
fn write_renders_starttime_from_injected_clock() {
    let dir = TempDir::new().expect("tempdir");
    let custom = StartTime {
        year: 2042,
        month: 3,
        day: 14,
        hour: 9,
        minute: 26,
        second: 53,
    };
    let w = XdgWallpaperSlideshowWriter::with_dir(dir.path())
        .with_clock(Box::new(FixedClock(custom)));
    let s = sample("future", &["/a.jpg", "/b.jpg"]);

    let path = w.write(&s).expect("write");
    let body = std::fs::read_to_string(&path).unwrap();
    assert!(body.contains("<year>2042</year>"));
    assert!(body.contains("<month>3</month>"));
    assert!(body.contains("<day>14</day>"));
    assert!(body.contains("<hour>9</hour>"));
    assert!(body.contains("<minute>26</minute>"));
    assert!(body.contains("<second>53</second>"));
}

#[test]
fn write_uses_slideshow_interval_as_static_duration() {
    let dir = TempDir::new().expect("tempdir");
    let w = writer(&dir);
    let s = WallpaperSlideshow::new(
        "timed",
        vec![PathBuf::from("/a.jpg"), PathBuf::from("/b.jpg")],
        1800,
        false,
    )
    .unwrap();

    let body = std::fs::read_to_string(w.write(&s).unwrap()).unwrap();
    // interval_seconds → <static><duration> (float-formatted)
    assert!(body.contains("<duration>1800.0</duration>"));
}

#[test]
fn write_is_atomic_no_tmp_file_left_behind() {
    let dir = TempDir::new().expect("tempdir");
    let w = writer(&dir);
    w.write(&sample("atom", &["/a.jpg", "/b.jpg"]))
        .expect("write");

    // After a successful write, only the final .xml should exist.
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, vec!["atom-t.xml"]);
}

#[test]
fn write_overwrites_existing_xml_for_same_name() {
    let dir = TempDir::new().expect("tempdir");
    let w = writer(&dir);

    let first = sample("same", &["/a.jpg", "/b.jpg"]);
    w.write(&first).expect("first write");

    let second = sample("same", &["/c.jpg", "/d.jpg"]);
    let path = w.write(&second).expect("second write");

    let body = std::fs::read_to_string(path).unwrap();
    // First-run pictures are gone, second-run pictures present.
    assert!(!body.contains("a.jpg"));
    assert!(!body.contains("b.jpg"));
    assert!(body.contains("/c.jpg"));
    assert!(body.contains("/d.jpg"));
}

#[test]
fn write_xml_escapes_paths_with_reserved_characters() {
    let dir = TempDir::new().expect("tempdir");
    let w = writer(&dir);
    let s = WallpaperSlideshow::new(
        "escape",
        vec![
            PathBuf::from("/pics/a&b.jpg"),
            PathBuf::from("/pics/<c>.jpg"),
        ],
        60,
        false,
    )
    .unwrap();

    let body = std::fs::read_to_string(w.write(&s).unwrap()).unwrap();
    // `&` must become `&amp;`, `<` must become `&lt;` — otherwise the
    // XML doesn't parse and GNOME falls back to a solid colour.
    assert!(body.contains("a&amp;b.jpg"));
    assert!(body.contains("&lt;c&gt;.jpg"));
}

#[test]
fn delete_is_idempotent_for_missing_file() {
    let dir = TempDir::new().expect("tempdir");
    let w = writer(&dir);
    // File does not exist yet — delete should not error.
    w.delete("nonexistent").expect("idempotent delete");
}

#[test]
fn delete_removes_previously_written_xml() {
    let dir = TempDir::new().expect("tempdir");
    let w = writer(&dir);
    let s = sample("todelete", &["/a.jpg", "/b.jpg"]);
    let path = w.write(&s).expect("write");
    assert!(path.exists());

    w.delete("todelete").expect("delete");
    assert!(!path.exists());
}

#[test]
fn write_uses_unique_filenames_across_applies() {
    // Prove the GSettings "same value suppression" workaround:
    // consecutive Applies land on distinct paths so `picture-uri`
    // is genuinely a new URI each time.
    use std::cell::Cell;
    let dir = TempDir::new().expect("tempdir");
    let counter = std::sync::Arc::new(std::sync::Mutex::new(0u64));
    let counter_clone = counter.clone();
    let w = XdgWallpaperSlideshowWriter::with_dir(dir.path())
        .with_clock(Box::new(FixedClock(StartTime::EPOCH)))
        .with_stamp_provider(Box::new(move || {
            let mut c = counter_clone.lock().unwrap();
            *c += 1;
            c.to_string()
        }));
    let s = sample("cycle", &["/a.jpg", "/b.jpg"]);

    let first = w.write(&s).expect("first write");
    let second = w.write(&s).expect("second write");
    let third = w.write(&s).expect("third write");
    assert_ne!(first, second);
    assert_ne!(second, third);
    assert_ne!(first, third);
    let _ = Cell::new(()); // silence unused import lint
    // Older applies are garbage-collected so the dir doesn't grow.
    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, vec!["cycle-3.xml"]);
}

#[test]
fn write_cleans_up_legacy_bare_name_xml() {
    // Earlier versions of the writer produced `<name>.xml`. Such a
    // file lingering in the directory would permanently show up in
    // `~/.local/share/gnome-background-properties/` even after
    // applies have rotated to stamped filenames. Garbage-collect it
    // on the next Apply.
    let dir = TempDir::new().expect("tempdir");
    std::fs::write(dir.path().join("legacy.xml"), b"stub").unwrap();

    let w = writer(&dir);
    let s = sample("legacy", &["/a.jpg", "/b.jpg"]);
    let _ = w.write(&s).expect("write");

    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, vec!["legacy-t.xml"]);
}

#[test]
fn delete_removes_stamped_and_legacy_xmls_for_name() {
    let dir = TempDir::new().expect("tempdir");
    // Seed the directory with one legacy and two stamped versions
    // of the same slideshow; `delete` must remove all three.
    for f in ["old.xml", "old-1.xml", "old-2.xml"] {
        std::fs::write(dir.path().join(f), b"stub").unwrap();
    }
    // Plus an unrelated slideshow that must be left alone.
    std::fs::write(dir.path().join("other-5.xml"), b"stub").unwrap();

    let w = writer(&dir);
    w.delete("old").expect("delete");

    let entries: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entries, vec!["other-5.xml"]);
}
