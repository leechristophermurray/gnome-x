// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use gnomex_app::ports::{PackStorage, PackSummary};
use gnomex_app::AppError;
use gnomex_domain::{
    CursorPackRef, ExperiencePack, ExtensionRef, GSettingOverride, IconPackRef, ShellTweak,
    ShellTweakId, ShellVersion, ThemeRef, TweakValue,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Concrete adapter: TOML-based experience pack storage.
///
/// Packs are stored as `.gnomex-pack.toml` files under
/// `~/.local/share/gnome-x/packs/`.
pub struct PackTomlStorage {
    packs_dir: PathBuf,
}

impl PackTomlStorage {
    pub fn new() -> Self {
        let packs_dir = directories::BaseDirs::new()
            .map(|d| d.data_local_dir().join("gnome-x/packs"))
            .unwrap_or_else(|| PathBuf::from("/tmp/gnome-x/packs"));
        Self { packs_dir }
    }

    pub fn from_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            packs_dir: dir.into(),
        }
    }

    fn pack_path(&self, id: &str) -> PathBuf {
        self.packs_dir.join(format!("{id}.gnomex-pack.toml"))
    }
}

// --- TOML schema (serde DTOs — boundary objects, NOT domain entities) ---

#[derive(Serialize, Deserialize)]
struct PackFile {
    pack: PackMeta,
    #[serde(default)]
    theme: Option<ThemeSection>,
    #[serde(default)]
    icons: Option<ContentRef>,
    #[serde(default)]
    cursor: Option<ContentRef>,
    #[serde(default)]
    extensions: Vec<ExtensionEntry>,
    #[serde(default)]
    settings: Vec<SettingEntry>,
    /// Present in `pack_format = 2` packs. `#[serde(default)]` keeps
    /// v1 packs deserializing cleanly.
    #[serde(default, rename = "shell_tweaks")]
    shell_tweaks: Vec<ShellTweakEntry>,
}

#[derive(Serialize, Deserialize)]
struct PackMeta {
    id: String,
    name: String,
    description: String,
    author: String,
    created: String,
    shell_version: String,
    pack_format: u32,
    #[serde(default)]
    wallpaper: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct ThemeSection {
    #[serde(default)]
    gtk: Option<ContentRef>,
    #[serde(default)]
    shell: Option<ContentRef>,
}

#[derive(Serialize, Deserialize)]
struct ContentRef {
    name: String,
    source: String,
    content_id: u64,
    file_id: u64,
}

#[derive(Serialize, Deserialize)]
struct ExtensionEntry {
    uuid: String,
    name: String,
    #[serde(default = "default_true")]
    required: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize)]
struct SettingEntry {
    key: String,
    value: String,
}

/// DTO for a single shell tweak in `[[shell_tweaks]]`.
/// `id` is a stable snake_case slug; `value` is a string whose shape
/// depends on the id's domain type (bool/int/enum/position).
#[derive(Serialize, Deserialize)]
struct ShellTweakEntry {
    id: String,
    value: String,
}

// --- DTO ↔ Domain mapping (boundary enforcement) ---

fn content_ref_to_theme(cr: &ContentRef) -> ThemeRef {
    ThemeRef {
        name: cr.name.clone(),
        source: cr.source.clone(),
        content_id: cr.content_id,
        file_id: cr.file_id,
    }
}

fn theme_to_content_ref(t: &ThemeRef) -> ContentRef {
    ContentRef {
        name: t.name.clone(),
        source: t.source.clone(),
        content_id: t.content_id,
        file_id: t.file_id,
    }
}

fn content_ref_to_icon(cr: &ContentRef) -> IconPackRef {
    IconPackRef {
        name: cr.name.clone(),
        source: cr.source.clone(),
        content_id: cr.content_id,
        file_id: cr.file_id,
    }
}

fn icon_to_content_ref(i: &IconPackRef) -> ContentRef {
    ContentRef {
        name: i.name.clone(),
        source: i.source.clone(),
        content_id: i.content_id,
        file_id: i.file_id,
    }
}

fn content_ref_to_cursor(cr: &ContentRef) -> CursorPackRef {
    CursorPackRef {
        name: cr.name.clone(),
        source: cr.source.clone(),
        content_id: cr.content_id,
        file_id: cr.file_id,
    }
}

fn cursor_to_content_ref(c: &CursorPackRef) -> ContentRef {
    ContentRef {
        name: c.name.clone(),
        source: c.source.clone(),
        content_id: c.content_id,
        file_id: c.file_id,
    }
}

fn pack_file_to_domain(pf: PackFile) -> Result<ExperiencePack, AppError> {
    let shell_version = ShellVersion::parse(&pf.pack.shell_version)
        .map_err(|e| AppError::Storage(e.to_string()))?;

    Ok(ExperiencePack {
        id: pf.pack.id,
        name: pf.pack.name,
        description: pf.pack.description,
        author: pf.pack.author,
        created_at: pf.pack.created,
        shell_version,
        pack_format: pf.pack.pack_format,
        gtk_theme: pf
            .theme
            .as_ref()
            .and_then(|t| t.gtk.as_ref())
            .map(content_ref_to_theme),
        shell_theme: pf
            .theme
            .as_ref()
            .and_then(|t| t.shell.as_ref())
            .map(content_ref_to_theme),
        icon_pack: pf.icons.as_ref().map(content_ref_to_icon),
        cursor_pack: pf.cursor.as_ref().map(content_ref_to_cursor),
        extensions: pf
            .extensions
            .iter()
            .map(|e| ExtensionRef {
                uuid: e.uuid.clone(),
                name: e.name.clone(),
                required: e.required,
            })
            .collect(),
        wallpaper: pf.pack.wallpaper,
        gsettings_overrides: pf
            .settings
            .iter()
            .map(|s| GSettingOverride {
                key: s.key.clone(),
                value: s.value.clone(),
            })
            .collect(),
        shell_tweaks: pf
            .shell_tweaks
            .iter()
            .filter_map(|e| {
                // Forward-compat: tweaks a newer version introduced
                // and this build doesn't know about just get dropped
                // with a debug log. Malformed values likewise.
                let id = ShellTweakId::from_slug(&e.id).or_else(|| {
                    tracing::debug!("pack load: unknown shell_tweak id '{}' — skipped", e.id);
                    None
                })?;
                let value = TweakValue::parse_for_id(id, &e.value).or_else(|| {
                    tracing::warn!(
                        "pack load: can't parse shell_tweak '{}' value '{}' — skipped",
                        e.id,
                        e.value
                    );
                    None
                })?;
                Some(ShellTweak { id, value })
            })
            .collect(),
    })
}

fn domain_to_pack_file(pack: &ExperiencePack) -> PackFile {
    let theme = if pack.gtk_theme.is_some() || pack.shell_theme.is_some() {
        Some(ThemeSection {
            gtk: pack.gtk_theme.as_ref().map(theme_to_content_ref),
            shell: pack.shell_theme.as_ref().map(theme_to_content_ref),
        })
    } else {
        None
    };

    PackFile {
        pack: PackMeta {
            id: pack.id.clone(),
            name: pack.name.clone(),
            description: pack.description.clone(),
            author: pack.author.clone(),
            created: pack.created_at.clone(),
            shell_version: pack.shell_version.to_string(),
            pack_format: pack.pack_format,
            wallpaper: pack.wallpaper.clone(),
        },
        theme,
        icons: pack.icon_pack.as_ref().map(icon_to_content_ref),
        cursor: pack.cursor_pack.as_ref().map(cursor_to_content_ref),
        extensions: pack
            .extensions
            .iter()
            .map(|e| ExtensionEntry {
                uuid: e.uuid.clone(),
                name: e.name.clone(),
                required: e.required,
            })
            .collect(),
        settings: pack
            .gsettings_overrides
            .iter()
            .map(|s| SettingEntry {
                key: s.key.clone(),
                value: s.value.clone(),
            })
            .collect(),
        shell_tweaks: pack
            .shell_tweaks
            .iter()
            .map(|t| ShellTweakEntry {
                id: t.id.slug().to_owned(),
                value: t.value.as_toml_string(),
            })
            .collect(),
    }
}

impl PackStorage for PackTomlStorage {
    fn save_pack(&self, pack: &ExperiencePack) -> Result<String, AppError> {
        std::fs::create_dir_all(&self.packs_dir)
            .map_err(|e| AppError::Storage(format!("mkdir failed: {e}")))?;

        let pf = domain_to_pack_file(pack);
        let toml_str =
            toml::to_string_pretty(&pf).map_err(|e| AppError::Storage(e.to_string()))?;

        let path = self.pack_path(&pack.id);
        std::fs::write(&path, toml_str)
            .map_err(|e| AppError::Storage(format!("write failed: {e}")))?;

        tracing::info!("saved pack {} to {}", pack.id, path.display());
        Ok(pack.id.clone())
    }

    fn load_pack(&self, id: &str) -> Result<ExperiencePack, AppError> {
        let path = self.pack_path(id);
        load_pack_from_path(&path)
    }

    fn list_packs(&self) -> Result<Vec<PackSummary>, AppError> {
        if !self.packs_dir.exists() {
            return Ok(vec![]);
        }
        let entries = std::fs::read_dir(&self.packs_dir)
            .map_err(|e| AppError::Storage(e.to_string()))?;

        let mut summaries = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|ext| ext == "toml")
            {
                if let Ok(pack) = load_pack_from_path(&path) {
                    summaries.push(PackSummary {
                        id: pack.id,
                        name: pack.name,
                        author: pack.author,
                        description: pack.description,
                    });
                }
            }
        }
        Ok(summaries)
    }

    fn delete_pack(&self, id: &str) -> Result<(), AppError> {
        let path = self.pack_path(id);
        if path.exists() {
            std::fs::remove_file(&path)
                .map_err(|e| AppError::Storage(format!("delete failed: {e}")))?;
        }
        Ok(())
    }

    fn export_pack(&self, id: &str, screenshot: Option<&[u8]>) -> Result<Vec<u8>, AppError> {
        let pack = self.load_pack(id)?;
        let pf = domain_to_pack_file(&pack);
        let toml_str =
            toml::to_string_pretty(&pf).map_err(|e| AppError::Storage(e.to_string()))?;

        let mut buf = Vec::new();
        {
            let encoder = flate2::write::GzEncoder::new(&mut buf, flate2::Compression::default());
            let mut archive = tar::Builder::new(encoder);

            // Add the manifest
            let toml_bytes = toml_str.as_bytes();
            let mut header = tar::Header::new_gnu();
            header.set_size(toml_bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive
                .append_data(&mut header, "pack.toml", toml_bytes)
                .map_err(|e| AppError::Storage(format!("tar append manifest: {e}")))?;

            // Add screenshot if provided
            if let Some(png) = screenshot {
                let mut ss_header = tar::Header::new_gnu();
                ss_header.set_size(png.len() as u64);
                ss_header.set_mode(0o644);
                ss_header.set_cksum();
                archive
                    .append_data(&mut ss_header, "screenshot.png", png)
                    .map_err(|e| AppError::Storage(format!("tar append screenshot: {e}")))?;
            }

            archive
                .finish()
                .map_err(|e| AppError::Storage(format!("tar finish: {e}")))?;
        }

        Ok(buf)
    }

    fn import_pack(&self, archive_data: &[u8]) -> Result<(String, Option<Vec<u8>>), AppError> {
        let decoder = flate2::read::GzDecoder::new(std::io::Cursor::new(archive_data));
        let mut archive = tar::Archive::new(decoder);

        let mut toml_content: Option<String> = None;
        let mut screenshot: Option<Vec<u8>> = None;

        for entry in archive
            .entries()
            .map_err(|e| AppError::Storage(format!("read archive: {e}")))?
        {
            let mut entry =
                entry.map_err(|e| AppError::Storage(format!("read entry: {e}")))?;
            let path = entry
                .path()
                .map_err(|e| AppError::Storage(format!("entry path: {e}")))?
                .to_path_buf();

            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("");

            if name == "pack.toml" {
                let mut s = String::new();
                std::io::Read::read_to_string(&mut entry, &mut s)
                    .map_err(|e| AppError::Storage(format!("read manifest: {e}")))?;
                toml_content = Some(s);
            } else if name == "screenshot.png" {
                let mut data = Vec::new();
                std::io::Read::read_to_end(&mut entry, &mut data)
                    .map_err(|e| AppError::Storage(format!("read screenshot: {e}")))?;
                screenshot = Some(data);
            }
        }

        let toml_str = toml_content
            .ok_or_else(|| AppError::Storage("archive missing pack.toml".into()))?;
        let pf: PackFile =
            toml::from_str(&toml_str).map_err(|e| AppError::Storage(e.to_string()))?;
        let pack = pack_file_to_domain(pf)?;
        let id = self.save_pack(&pack)?;

        // Save screenshot alongside the pack if present
        if let Some(ref png) = screenshot {
            let ss_path = self.packs_dir.join(format!("{id}.screenshot.png"));
            std::fs::write(&ss_path, png).ok();
        }

        Ok((id, screenshot))
    }
}

fn load_pack_from_path(path: &Path) -> Result<ExperiencePack, AppError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Storage(format!("read {}: {e}", path.display())))?;
    let pf: PackFile =
        toml::from_str(&content).map_err(|e| AppError::Storage(e.to_string()))?;
    pack_file_to_domain(pf)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gnomex_domain::{PanelPosition, ShellTweakId, TweakValue};

    fn pack_fixture(shell_tweaks: Vec<ShellTweak>) -> ExperiencePack {
        ExperiencePack {
            id: "fixture".into(),
            name: "Fixture".into(),
            description: "For tests".into(),
            author: "tester".into(),
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
    fn shell_tweaks_round_trip_through_toml() {
        let tweaks = vec![
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
        ];
        let pack = pack_fixture(tweaks.clone());
        let pf = domain_to_pack_file(&pack);
        let toml_str = toml::to_string_pretty(&pf).expect("serialize");
        let back: PackFile = toml::from_str(&toml_str).expect("deserialize");
        let loaded = pack_file_to_domain(back).expect("domain round trip");
        assert_eq!(loaded.shell_tweaks, tweaks);
    }

    #[test]
    fn v1_toml_without_shell_tweaks_still_loads() {
        // A pack written by v0.2 (pack_format = 1) — no [[shell_tweaks]]
        // table at all. It must deserialize cleanly with an empty tweak
        // vector, not an error.
        let v1_toml = r#"
[pack]
id = "legacy"
name = "Legacy Pack"
description = "Pre-v2"
author = "tester"
created = "0"
shell_version = "47.0"
pack_format = 1
"#;
        let pf: PackFile = toml::from_str(v1_toml).expect("v1 parses");
        let pack = pack_file_to_domain(pf).expect("domain map");
        assert_eq!(pack.pack_format, 1);
        assert!(pack.shell_tweaks.is_empty());
        assert!(pack.validate().is_ok());
    }

    #[test]
    fn unknown_shell_tweak_slug_is_skipped_not_fatal() {
        // Forward-compat: a pack written by a *newer* version of the
        // app may carry tweak ids we don't know about. Drop them with
        // a log line rather than failing the whole load.
        let forward_compat_toml = r#"
[pack]
id = "forward"
name = "Forward-Compat"
description = ""
author = "tester"
created = "0"
shell_version = "47.0"
pack_format = 2

[[shell_tweaks]]
id = "enable_animations"
value = "true"

[[shell_tweaks]]
id = "invented_in_v3"
value = "whatever"
"#;
        let pf: PackFile = toml::from_str(forward_compat_toml).expect("parses");
        let pack = pack_file_to_domain(pf).expect("domain map");
        assert_eq!(pack.shell_tweaks.len(), 1);
        assert_eq!(pack.shell_tweaks[0].id, ShellTweakId::EnableAnimations);
    }

    #[test]
    fn malformed_shell_tweak_value_is_skipped() {
        let toml_str = r#"
[pack]
id = "malformed"
name = "Malformed Values"
description = ""
author = "tester"
created = "0"
shell_version = "47.0"
pack_format = 2

[[shell_tweaks]]
id = "apps_grid_columns"
value = "not_a_number"
"#;
        let pf: PackFile = toml::from_str(toml_str).expect("parses");
        let pack = pack_file_to_domain(pf).expect("domain map");
        assert!(pack.shell_tweaks.is_empty());
    }
}
