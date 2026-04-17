// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Architectural invariant: no GNOME-version-major strings appear in
//! Green (`crates/domain`), Blue (`crates/app`), or Yellow
//! (`crates/ui`). Version-conditional logic lives only in Red, under
//! `crates/infra/src/shell_customizer/` and `crates/infra/src/theme_css/`.
//!
//! This test fails loudly if a future change leaks `gnome47` or
//! `Gnome50` into a layer that's supposed to be version-agnostic.

use std::path::{Path, PathBuf};

const FORBIDDEN: &[&str] = &["gnome45", "gnome46", "gnome47", "gnome48", "gnome50"];

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is crates/infra; ../.. is the workspace root.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .canonicalize()
        .expect("workspace root resolves")
}

#[test]
fn version_strings_do_not_leak_outside_red() {
    let root = workspace_root();
    let mut leaks: Vec<String> = Vec::new();

    for crate_name in ["domain", "app", "ui"] {
        let crate_src = root.join("crates").join(crate_name).join("src");
        scan_dir(&crate_src, &crate_src, &mut leaks);
    }

    assert!(
        leaks.is_empty(),
        "Version-major strings leaked outside Red:\n  {}",
        leaks.join("\n  "),
    );
}

fn scan_dir(root: &Path, dir: &Path, leaks: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_dir(root, &path, leaks);
            continue;
        }
        if path.extension().and_then(|s| s.to_str()) != Some("rs") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(&path) else {
            continue;
        };
        for (lineno, line) in content.lines().enumerate() {
            // Strip line + block comments so doc-comments referring
            // to GNOME versions (e.g. "// GNOME 47+ adds X") don't fail.
            let code = strip_comment(line);
            for token in FORBIDDEN {
                if code.to_ascii_lowercase().contains(token) {
                    let rel = path.strip_prefix(root).unwrap_or(&path);
                    leaks.push(format!("{}:{} — '{}'", rel.display(), lineno + 1, token));
                }
            }
        }
    }
}

fn strip_comment(line: &str) -> &str {
    // Naive: anything after `//` is a comment. We don't try to handle
    // block comments because they're rare in this codebase and a single
    // false positive in one is cheaper than a brittle parser.
    line.split_once("//").map(|(code, _)| code).unwrap_or(line)
}
