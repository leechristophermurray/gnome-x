// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{DomainError, ShellVersion};

/// Reference to a theme within an experience pack.
#[derive(Debug, Clone)]
pub struct ThemeRef {
    pub name: String,
    pub source: String,
    pub content_id: u64,
    pub file_id: u64,
}

/// Reference to an icon pack within an experience pack.
#[derive(Debug, Clone)]
pub struct IconPackRef {
    pub name: String,
    pub source: String,
    pub content_id: u64,
    pub file_id: u64,
}

/// Reference to a cursor pack within an experience pack.
#[derive(Debug, Clone)]
pub struct CursorPackRef {
    pub name: String,
    pub source: String,
    pub content_id: u64,
    pub file_id: u64,
}

/// Reference to an extension within an experience pack.
#[derive(Debug, Clone)]
pub struct ExtensionRef {
    pub uuid: String,
    pub name: String,
    pub required: bool,
}

/// A GSettings key-value override.
#[derive(Debug, Clone)]
pub struct GSettingOverride {
    pub key: String,
    pub value: String,
}

/// Aggregate Root: a complete desktop configuration snapshot.
///
/// An Experience Pack captures theme + icons + cursor + extensions + settings
/// into a shareable TOML manifest. This is the core differentiator of GNOME X.
#[derive(Debug, Clone)]
pub struct ExperiencePack {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub created_at: String,
    pub shell_version: ShellVersion,
    pub pack_format: u32,

    pub gtk_theme: Option<ThemeRef>,
    pub shell_theme: Option<ThemeRef>,
    pub icon_pack: Option<IconPackRef>,
    pub cursor_pack: Option<CursorPackRef>,
    pub extensions: Vec<ExtensionRef>,
    pub wallpaper: Option<String>,
    pub gsettings_overrides: Vec<GSettingOverride>,
}

impl ExperiencePack {
    /// Validate the pack's structural integrity.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.name.trim().is_empty() {
            return Err(DomainError::InvalidPack(
                "pack name must not be empty".into(),
            ));
        }
        if self.pack_format != 1 {
            return Err(DomainError::InvalidPack(format!(
                "unsupported pack format: {}",
                self.pack_format,
            )));
        }
        // Validate that extension UUIDs contain '@'
        for ext in &self.extensions {
            if !ext.uuid.contains('@') {
                return Err(DomainError::InvalidExtensionUuid(ext.uuid.clone()));
            }
        }
        Ok(())
    }

    /// List extensions that are required for this pack.
    pub fn required_extensions(&self) -> Vec<&ExtensionRef> {
        self.extensions.iter().filter(|e| e.required).collect()
    }

    /// List extensions that are optional enhancements.
    pub fn optional_extensions(&self) -> Vec<&ExtensionRef> {
        self.extensions.iter().filter(|e| !e.required).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pack() -> ExperiencePack {
        ExperiencePack {
            id: "test-pack".into(),
            name: "Test Pack".into(),
            description: "A test experience pack".into(),
            author: "Tester".into(),
            created_at: "2026-04-13T12:00:00Z".into(),
            shell_version: ShellVersion::new(47, 0),
            pack_format: 1,
            gtk_theme: None,
            shell_theme: None,
            icon_pack: None,
            cursor_pack: None,
            extensions: vec![
                ExtensionRef {
                    uuid: "required@ext.com".into(),
                    name: "Required".into(),
                    required: true,
                },
                ExtensionRef {
                    uuid: "optional@ext.com".into(),
                    name: "Optional".into(),
                    required: false,
                },
            ],
            wallpaper: None,
            gsettings_overrides: vec![],
        }
    }

    #[test]
    fn validate_valid_pack() {
        assert!(sample_pack().validate().is_ok());
    }

    #[test]
    fn validate_empty_name() {
        let mut pack = sample_pack();
        pack.name = "  ".into();
        assert!(pack.validate().is_err());
    }

    #[test]
    fn validate_bad_format() {
        let mut pack = sample_pack();
        pack.pack_format = 99;
        assert!(pack.validate().is_err());
    }

    #[test]
    fn validate_bad_extension_uuid() {
        let mut pack = sample_pack();
        pack.extensions.push(ExtensionRef {
            uuid: "no-at-sign".into(),
            name: "Bad".into(),
            required: true,
        });
        assert!(pack.validate().is_err());
    }

    #[test]
    fn required_and_optional() {
        let pack = sample_pack();
        assert_eq!(pack.required_extensions().len(), 1);
        assert_eq!(pack.optional_extensions().len(), 1);
    }
}
