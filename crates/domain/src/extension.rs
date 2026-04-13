// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{DomainError, ShellVersion};
use core::fmt;

/// Validated GNOME Shell extension UUID.
///
/// Value Object — must contain an '@' character (for example,
/// `"dash-to-dock@micxgx.gmail.com"`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtensionUuid(String);

impl ExtensionUuid {
    pub fn new(uuid: impl Into<String>) -> Result<Self, DomainError> {
        let uuid = uuid.into();
        if uuid.contains('@') {
            Ok(Self(uuid))
        } else {
            Err(DomainError::InvalidExtensionUuid(uuid))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ExtensionUuid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Lifecycle state of an extension on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExtensionState {
    /// Known to exist on extensions.gnome.org but not installed locally.
    Available,
    /// Installed but currently disabled.
    Installed,
    /// Installed and enabled.
    Enabled,
    /// Installed but disabled by the user.
    Disabled,
    /// Installed but in an error state.
    Error,
}

impl fmt::Display for ExtensionState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Available => write!(f, "Available"),
            Self::Installed => write!(f, "Installed"),
            Self::Enabled => write!(f, "Enabled"),
            Self::Disabled => write!(f, "Disabled"),
            Self::Error => write!(f, "Error"),
        }
    }
}

/// Compatibility assessment of an extension against the running shell version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompatibilityBadge {
    /// Extension explicitly supports the current shell version.
    Compatible,
    /// No version data available for the current shell.
    Untested,
    /// Extension is explicitly unsupported on the current shell.
    Incompatible,
}

/// A GNOME Shell extension entity.
///
/// Represents an extension from extensions.gnome.org (EGO), whether
/// installed locally or discovered via search.
#[derive(Debug, Clone)]
pub struct Extension {
    pub uuid: ExtensionUuid,
    pub name: String,
    pub description: String,
    pub creator: String,
    pub shell_versions: Vec<ShellVersion>,
    pub version: u32,
    pub download_url: Option<String>,
    pub screenshot_url: Option<String>,
    pub homepage_url: Option<String>,
    pub pk: Option<u64>,
    pub state: ExtensionState,
}

impl Extension {
    /// Determine the compatibility badge for a given shell version.
    pub fn compatibility(&self, shell: &ShellVersion) -> CompatibilityBadge {
        if self.shell_versions.is_empty() {
            return CompatibilityBadge::Untested;
        }
        if self.shell_versions.iter().any(|v| v.is_compatible_with(shell)) {
            CompatibilityBadge::Compatible
        } else {
            CompatibilityBadge::Incompatible
        }
    }

    /// Whether this extension is active on the system.
    pub fn is_active(&self) -> bool {
        self.state == ExtensionState::Enabled
    }

    /// Whether this extension is present on disk (any installed state).
    pub fn is_installed(&self) -> bool {
        matches!(
            self.state,
            ExtensionState::Installed
                | ExtensionState::Enabled
                | ExtensionState::Disabled
                | ExtensionState::Error
        )
    }
}

/// Content rating score, validated to the 0.0–5.0 range.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContentRating {
    score: f32,
}

impl ContentRating {
    pub fn new(score: f32) -> Result<Self, DomainError> {
        if (0.0..=5.0).contains(&score) {
            Ok(Self { score })
        } else {
            Err(DomainError::InvalidContentRating(score.to_string()))
        }
    }

    pub fn score(&self) -> f32 {
        self.score
    }
}

/// Paginated search results from a repository.
#[derive(Debug, Clone)]
pub struct SearchResult<T> {
    pub items: Vec<T>,
    pub total: u32,
    pub page: u32,
    pub pages: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_uuid() {
        let uuid = ExtensionUuid::new("dash-to-dock@micxgx.gmail.com").unwrap();
        assert_eq!(uuid.as_str(), "dash-to-dock@micxgx.gmail.com");
    }

    #[test]
    fn invalid_uuid_missing_at() {
        assert!(ExtensionUuid::new("no-at-sign").is_err());
    }

    #[test]
    fn compatibility_badge() {
        let ext = Extension {
            uuid: ExtensionUuid::new("test@test.com").unwrap(),
            name: "Test".into(),
            description: String::new(),
            creator: "author".into(),
            shell_versions: vec![ShellVersion::new(46, 0), ShellVersion::new(47, 0)],
            version: 1,
            download_url: None,
            screenshot_url: None,
            homepage_url: None,
            pk: None,
            state: ExtensionState::Available,
        };

        assert_eq!(
            ext.compatibility(&ShellVersion::new(47, 0)),
            CompatibilityBadge::Compatible
        );
        assert_eq!(
            ext.compatibility(&ShellVersion::new(45, 0)),
            CompatibilityBadge::Incompatible
        );
    }

    #[test]
    fn content_rating_valid() {
        assert!(ContentRating::new(4.5).is_ok());
        assert!(ContentRating::new(0.0).is_ok());
        assert!(ContentRating::new(5.0).is_ok());
    }

    #[test]
    fn content_rating_invalid() {
        assert!(ContentRating::new(-0.1).is_err());
        assert!(ContentRating::new(5.1).is_err());
    }
}
