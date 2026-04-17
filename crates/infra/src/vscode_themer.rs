// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! VS Code-family adapter.
//!
//! Merges a GNOME X-owned `workbench.colorCustomizations` block into the
//! user settings files of every VS Code-family editor we can detect
//! (VS Code, VSCodium, Cursor, Windsurf, VS Code Insiders).
//!
//! ## Ownership / safety contract
//!
//! VS Code's `settings.json` is a JSONC (JSON-with-comments) file that is
//! frequently hand-edited. We therefore:
//!
//! 1. Read the existing `settings.json` if present.
//! 2. Parse it leniently (treat as plain JSON — if the user has comments
//!    or trailing commas we bail out and log, never overwrite).
//! 3. Take ownership of **only** the `workbench.colorCustomizations` key,
//!    writing a small set of recognized keys to it. All other keys
//!    (including keys the user may have added inside
//!    `workbench.colorCustomizations`) are preserved.
//! 4. Keep our write atomic: write to a sibling temp file then rename.

use gnomex_app::ports::ExternalAppThemer;
use gnomex_app::AppError;
use gnomex_domain::{ColorScheme, ExternalThemeSpec};
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};

/// One VS Code family member. The `dir_name` is the folder under
/// `~/.config/` where that distribution stores its `User/settings.json`.
struct VscodeFlavor {
    name: &'static str,
    dir_name: &'static str,
}

const FLAVORS: &[VscodeFlavor] = &[
    VscodeFlavor { name: "VS Code",          dir_name: "Code" },
    VscodeFlavor { name: "VS Code Insiders", dir_name: "Code - Insiders" },
    VscodeFlavor { name: "VSCodium",         dir_name: "VSCodium" },
    VscodeFlavor { name: "Cursor",           dir_name: "Cursor" },
    VscodeFlavor { name: "Windsurf",         dir_name: "Windsurf" },
    VscodeFlavor { name: "Antigravity",      dir_name: "Antigravity" },
];

/// Key inside `settings.json` that we own. Keys we add are documented in
/// `colors_for_spec()`; any user-authored keys inside this map are kept.
const COLOR_CUSTOMIZATIONS: &str = "workbench.colorCustomizations";

/// Marker keys we use inside `workbench.colorCustomizations` so we can
/// later reset cleanly. Any key in this list that the user already set
/// gets overwritten by us; anything else stays untouched.
const OWNED_KEYS: &[&str] = &[
    // Title bar
    "titleBar.activeBackground",
    "titleBar.inactiveBackground",
    "titleBar.activeForeground",
    "titleBar.inactiveForeground",
    "titleBar.border",
    // Activity bar (the left strip with icons)
    "activityBar.background",
    "activityBar.foreground",
    "activityBar.inactiveForeground",
    "activityBar.activeBackground",
    "activityBar.activeBorder",
    "activityBarBadge.background",
    "activityBarBadge.foreground",
    // Side bar
    "sideBar.border",
    "sideBarTitle.foreground",
    "sideBarSectionHeader.border",
    // Status bar
    "statusBar.background",
    "statusBar.foreground",
    "statusBar.border",
    "statusBarItem.remoteBackground",
    // Panels / editor chrome
    "panel.border",
    "editorGroupHeader.tabsBorder",
    "tab.activeBorder",
    "tab.activeBorderTop",
    // Accents on interactive controls
    "focusBorder",
    "button.background",
    "button.hoverBackground",
    "button.foreground",
    "progressBar.background",
    "badge.background",
    "badge.foreground",
    "scrollbarSlider.activeBackground",
];

pub struct VscodeThemer {
    home: PathBuf,
}

impl VscodeThemer {
    pub fn new() -> Self {
        let home = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        Self { home }
    }

    /// The on-disk path to `settings.json` for a given flavor, or None if
    /// that flavor's config directory doesn't exist on this system.
    fn settings_path(&self, flavor: &VscodeFlavor) -> Option<PathBuf> {
        let dir = self.home.join(".config").join(flavor.dir_name);
        if dir.is_dir() {
            Some(dir.join("User").join("settings.json"))
        } else {
            None
        }
    }

    /// Apply `spec` to a single settings.json path, creating parent
    /// directories and the file itself if needed.
    fn apply_to(&self, path: &Path, spec: &ExternalThemeSpec) -> Result<(), AppError> {
        let root = load_or_empty(path)?;
        let new_root = merge_owned_colors(root, spec);
        write_atomic(path, &new_root)?;
        tracing::info!("vscode: wrote color customizations to {}", path.display());
        Ok(())
    }

    fn reset_at(&self, path: &Path) -> Result<(), AppError> {
        if !path.exists() {
            return Ok(());
        }
        let root = load_or_empty(path)?;
        let new_root = strip_owned_colors(root);
        write_atomic(path, &new_root)?;
        tracing::info!("vscode: cleared color customizations at {}", path.display());
        Ok(())
    }
}

impl Default for VscodeThemer {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalAppThemer for VscodeThemer {
    fn name(&self) -> &str {
        "vscode"
    }

    fn apply(&self, spec: &ExternalThemeSpec) -> Result<(), AppError> {
        let mut seen = false;
        for flavor in FLAVORS {
            if let Some(path) = self.settings_path(flavor) {
                seen = true;
                if let Err(e) = self.apply_to(&path, spec) {
                    tracing::warn!("vscode ({}): {e}", flavor.name);
                }
            }
        }
        if !seen {
            tracing::debug!("vscode: no installations detected");
        }
        Ok(())
    }

    fn reset(&self) -> Result<(), AppError> {
        for flavor in FLAVORS {
            if let Some(path) = self.settings_path(flavor) {
                if let Err(e) = self.reset_at(&path) {
                    tracing::warn!("vscode ({}): {e}", flavor.name);
                }
            }
        }
        Ok(())
    }
}

// --- Pure helpers (unit tested) ------------------------------------------

/// Build the set of VS Code color keys we want to assert for this spec.
///
/// Palette strategy: match the **libadwaita** look. The window chrome
/// (titlebar / sidebar / activity bar) uses the libadwaita neutral
/// surface colors for the current color scheme — dark or light —
/// regardless of the user's GNOME Shell panel tint. The accent is used
/// only as a highlight on interactive elements (active tab, status bar,
/// focus ring, buttons, badges). This gives every VS Code-family editor
/// a subtle, GNOME-X-consistent look without flooding chrome with color.
fn colors_for_spec(spec: &ExternalThemeSpec) -> Map<String, Value> {
    let p = adwaita_palette(spec.color_scheme);
    let accent = spec.accent.as_str().to_string();
    let accent_hover = lighten(accent.as_str(), 0.08);
    let accent_fg = contrast_fg(spec.accent.to_rgb(), spec.color_scheme);

    let mut m = Map::new();

    // Title bar — libadwaita headerbar neutral.
    m.insert("titleBar.activeBackground".into(), Value::String(p.headerbar.into()));
    m.insert("titleBar.inactiveBackground".into(), Value::String(p.headerbar.into()));
    m.insert("titleBar.activeForeground".into(), Value::String(p.fg.into()));
    m.insert("titleBar.inactiveForeground".into(), Value::String(p.fg_dim.into()));
    m.insert("titleBar.border".into(), Value::String(p.border.into()));

    // Activity bar — libadwaita sidebar neutral, accent highlights.
    m.insert("activityBar.background".into(), Value::String(p.sidebar.into()));
    m.insert("activityBar.foreground".into(), Value::String(p.fg.into()));
    m.insert("activityBar.inactiveForeground".into(), Value::String(p.fg_dim.into()));
    m.insert("activityBar.activeBorder".into(), Value::String(accent.clone()));
    m.insert("activityBar.activeBackground".into(), Value::String(p.sidebar_hover.into()));
    m.insert("activityBarBadge.background".into(), Value::String(accent.clone()));
    m.insert("activityBarBadge.foreground".into(), Value::String(accent_fg.clone()));

    // Side bar — libadwaita sidebar neutral.
    m.insert("sideBar.border".into(), Value::String(p.border.into()));
    m.insert("sideBarTitle.foreground".into(), Value::String(p.fg.into()));
    m.insert("sideBarSectionHeader.border".into(), Value::String(p.border.into()));

    // Status bar — accent strip (the one place we flood-color, matching
    // how most "accent" VS Code themes behave).
    m.insert("statusBar.background".into(), Value::String(accent.clone()));
    m.insert("statusBar.foreground".into(), Value::String(accent_fg.clone()));
    m.insert("statusBar.border".into(), Value::String(accent.clone()));
    m.insert("statusBarItem.remoteBackground".into(), Value::String(accent_hover.clone()));

    // Panels / editor chrome — subtle accent only on active item.
    m.insert("panel.border".into(), Value::String(p.border.into()));
    m.insert("editorGroupHeader.tabsBorder".into(), Value::String(p.border.into()));
    m.insert("tab.activeBorder".into(), Value::String(accent.clone()));
    m.insert("tab.activeBorderTop".into(), Value::String(accent.clone()));

    // Interactive accents.
    m.insert("focusBorder".into(), Value::String(accent.clone()));
    m.insert("button.background".into(), Value::String(accent.clone()));
    m.insert("button.hoverBackground".into(), Value::String(accent_hover));
    m.insert("button.foreground".into(), Value::String(accent_fg.clone()));
    m.insert("progressBar.background".into(), Value::String(accent.clone()));
    m.insert("badge.background".into(), Value::String(accent.clone()));
    m.insert("badge.foreground".into(), Value::String(accent_fg));
    m.insert("scrollbarSlider.activeBackground".into(), Value::String(accent));
    m
}

/// Libadwaita-neutral palette for a given color scheme. Values picked
/// to match libadwaita's standard dark/light window surfaces.
struct AdwaitaPalette {
    headerbar: &'static str,
    sidebar: &'static str,
    sidebar_hover: &'static str,
    border: &'static str,
    fg: &'static str,
    fg_dim: &'static str,
}

fn adwaita_palette(scheme: ColorScheme) -> AdwaitaPalette {
    if scheme.is_dark() {
        AdwaitaPalette {
            headerbar: "#2e2e2e",
            sidebar: "#242424",
            sidebar_hover: "#333333",
            border: "#1c1c1c",
            fg: "#ededed",
            fg_dim: "#9a9a9a",
        }
    } else {
        AdwaitaPalette {
            headerbar: "#ebebeb",
            sidebar: "#fafafa",
            sidebar_hover: "#e6e6e6",
            border: "#d0d0d0",
            fg: "#2e2e2e",
            fg_dim: "#6b6b6b",
        }
    }
}

/// Merge our owned keys into an existing settings.json root, preserving
/// every user-authored key (including unrelated keys inside
/// `workbench.colorCustomizations`).
fn merge_owned_colors(mut root: Value, spec: &ExternalThemeSpec) -> Value {
    let root_obj = root.as_object_mut().expect("root guaranteed object");
    let customs = root_obj
        .entry(COLOR_CUSTOMIZATIONS.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    let Some(customs) = customs.as_object_mut() else {
        // The user set `workbench.colorCustomizations` to a non-object.
        // Don't clobber — skip this file.
        tracing::warn!("vscode: workbench.colorCustomizations is not an object, skipping");
        return root;
    };
    for (k, v) in colors_for_spec(spec) {
        customs.insert(k, v);
    }
    root
}

/// Remove only the keys we own from `workbench.colorCustomizations`,
/// leaving the rest untouched. Removes the whole `workbench.colorCustomizations`
/// key if it becomes empty.
fn strip_owned_colors(mut root: Value) -> Value {
    let Some(root_obj) = root.as_object_mut() else {
        return root;
    };
    let Some(customs) = root_obj
        .get_mut(COLOR_CUSTOMIZATIONS)
        .and_then(|v| v.as_object_mut())
    else {
        return root;
    };
    for k in OWNED_KEYS {
        customs.remove(*k);
    }
    if customs.is_empty() {
        root_obj.remove(COLOR_CUSTOMIZATIONS);
    }
    root
}

fn load_or_empty(path: &Path) -> Result<Value, AppError> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| AppError::Settings(format!("read {}: {e}", path.display())))?;
    if raw.trim().is_empty() {
        return Ok(Value::Object(Map::new()));
    }
    let value: Value = serde_json::from_str(&raw).map_err(|e| {
        AppError::Settings(format!(
            "{} is not plain JSON (has comments/trailing commas?): {e}",
            path.display()
        ))
    })?;
    if !value.is_object() {
        return Err(AppError::Settings(format!(
            "{} is not a JSON object",
            path.display()
        )));
    }
    Ok(value)
}

fn write_atomic(path: &Path, value: &Value) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Settings(format!("mkdir {}: {e}", parent.display())))?;
    }
    let tmp = path.with_extension("json.gnomex-tmp");
    let pretty = serde_json::to_string_pretty(value)
        .map_err(|e| AppError::Settings(format!("serialize: {e}")))?;
    std::fs::write(&tmp, pretty)
        .map_err(|e| AppError::Settings(format!("write {}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| AppError::Settings(format!("rename {}: {e}", path.display())))?;
    Ok(())
}

/// Choose a legible foreground for a given background RGB under the
/// user's dark/light preference. Uses the simple Rec. 601 luma cut-off.
fn contrast_fg(bg: (u8, u8, u8), scheme: ColorScheme) -> String {
    let (r, g, b) = bg;
    let luma = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    let prefer_dark_fg = luma > 160.0;
    if prefer_dark_fg || !scheme.is_dark() {
        "#1e1e1e".to_string()
    } else {
        "#f0f0f0".to_string()
    }
}

/// Lighten a `#rrggbb` color by mixing it toward white.
fn lighten(hex: &str, amount: f32) -> String {
    let s = hex.trim_start_matches('#');
    let Some((r, g, b)) = parse_hex(s) else {
        return hex.to_string();
    };
    let mix = |c: u8| ((c as f32) + (255.0 - c as f32) * amount).clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", mix(r), mix(g), mix(b))
}

fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    if s.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gnomex_domain::HexColor;

    fn spec() -> ExternalThemeSpec {
        ExternalThemeSpec {
            accent: HexColor::new("#3584e4").unwrap(),
            panel_tint: HexColor::new("#1a1a1e").unwrap(),
            color_scheme: ColorScheme::Dark,
        }
    }

    #[test]
    fn merge_into_empty_settings_creates_full_block() {
        let merged = merge_owned_colors(Value::Object(Map::new()), &spec());
        let customs = merged
            .get(COLOR_CUSTOMIZATIONS)
            .and_then(|v| v.as_object())
            .expect("block created");
        for key in OWNED_KEYS {
            assert!(customs.contains_key(*key), "missing {key}");
        }
        assert_eq!(
            customs.get("button.background").and_then(|v| v.as_str()),
            Some("#3584e4")
        );
    }

    #[test]
    fn merge_preserves_unrelated_user_keys() {
        let raw = serde_json::json!({
            "editor.fontSize": 15,
            "workbench.colorCustomizations": {
                "myCustom.color": "#ff00ff",
            },
            "workbench.colorTheme": "One Dark"
        });
        let merged = merge_owned_colors(raw, &spec());

        assert_eq!(merged.get("editor.fontSize").and_then(|v| v.as_i64()), Some(15));
        assert_eq!(
            merged.get("workbench.colorTheme").and_then(|v| v.as_str()),
            Some("One Dark")
        );
        let customs = merged
            .get(COLOR_CUSTOMIZATIONS)
            .and_then(|v| v.as_object())
            .unwrap();
        assert_eq!(
            customs.get("myCustom.color").and_then(|v| v.as_str()),
            Some("#ff00ff"),
            "user's unrelated key preserved"
        );
        assert!(
            customs.contains_key("button.background"),
            "our key added alongside"
        );
    }

    #[test]
    fn apply_is_idempotent() {
        let once = merge_owned_colors(Value::Object(Map::new()), &spec());
        let twice = merge_owned_colors(once.clone(), &spec());
        assert_eq!(once, twice);
    }

    #[test]
    fn strip_removes_only_owned_keys() {
        let with_ours = merge_owned_colors(
            serde_json::json!({
                "workbench.colorCustomizations": { "myCustom.color": "#abc123" }
            }),
            &spec(),
        );
        let stripped = strip_owned_colors(with_ours);
        let customs = stripped
            .get(COLOR_CUSTOMIZATIONS)
            .and_then(|v| v.as_object())
            .expect("user key kept the block alive");
        for key in OWNED_KEYS {
            assert!(!customs.contains_key(*key), "{key} not removed");
        }
        assert_eq!(
            customs.get("myCustom.color").and_then(|v| v.as_str()),
            Some("#abc123")
        );
    }

    #[test]
    fn strip_removes_empty_block() {
        let only_ours = merge_owned_colors(Value::Object(Map::new()), &spec());
        let stripped = strip_owned_colors(only_ours);
        assert!(
            stripped.get(COLOR_CUSTOMIZATIONS).is_none(),
            "block removed when empty"
        );
    }

    #[test]
    fn non_object_customizations_are_skipped() {
        let raw = serde_json::json!({
            "workbench.colorCustomizations": "some-string",
            "keep.me": 42
        });
        let merged = merge_owned_colors(raw.clone(), &spec());
        // Returned unchanged — we refuse to clobber a non-object.
        assert_eq!(merged, raw);
    }
}
