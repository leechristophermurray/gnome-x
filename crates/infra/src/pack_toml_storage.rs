// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use gnomex_app::ports::{PackStorage, PackSummary};
use gnomex_app::AppError;
use gnomex_domain::{
    CursorPackRef, ExperiencePack, ExtensionRef, GSettingOverride, IconPackRef, ShellVersion,
    ThemeRef,
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
}

fn load_pack_from_path(path: &Path) -> Result<ExperiencePack, AppError> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| AppError::Storage(format!("read {}: {e}", path.display())))?;
    let pf: PackFile =
        toml::from_str(&content).map_err(|e| AppError::Storage(e.to_string()))?;
    pack_file_to_domain(pf)
}
