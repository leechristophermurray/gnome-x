// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::DomainError;
use core::fmt;

/// A GNOME Shell version (for example, 46 or 47).
///
/// Value Object — immutable after construction, validated on creation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ShellVersion {
    pub major: u32,
    pub minor: u32,
}

impl ShellVersion {
    pub fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Parse from a version string like `"47"` or `"47.1"`.
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        let s = s.trim();
        if let Some((maj, min)) = s.split_once('.') {
            let major = maj
                .parse::<u32>()
                .map_err(|_| DomainError::InvalidShellVersion(s.to_owned()))?;
            let minor = min
                .parse::<u32>()
                .map_err(|_| DomainError::InvalidShellVersion(s.to_owned()))?;
            Ok(Self { major, minor })
        } else {
            let major = s
                .parse::<u32>()
                .map_err(|_| DomainError::InvalidShellVersion(s.to_owned()))?;
            Ok(Self { major, minor: 0 })
        }
    }

    /// Check whether `self` is compatible with a given `target` shell version.
    ///
    /// In GNOME, extensions built for shell version N generally work on N.x
    /// but not on N+1.
    pub fn is_compatible_with(&self, target: &ShellVersion) -> bool {
        self.major == target.major
    }
}

impl fmt::Display for ShellVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.minor == 0 {
            write!(f, "{}", self.major)
        } else {
            write!(f, "{}.{}", self.major, self.minor)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_major_only() {
        let v = ShellVersion::parse("47").unwrap();
        assert_eq!(v, ShellVersion::new(47, 0));
        assert_eq!(v.to_string(), "47");
    }

    #[test]
    fn parse_major_minor() {
        let v = ShellVersion::parse("46.3").unwrap();
        assert_eq!(v, ShellVersion::new(46, 3));
        assert_eq!(v.to_string(), "46.3");
    }

    #[test]
    fn parse_invalid() {
        assert!(ShellVersion::parse("abc").is_err());
        assert!(ShellVersion::parse("").is_err());
    }

    #[test]
    fn compatibility() {
        let v46 = ShellVersion::new(46, 0);
        let v46_3 = ShellVersion::new(46, 3);
        let v47 = ShellVersion::new(47, 0);

        assert!(v46.is_compatible_with(&v46_3));
        assert!(!v46.is_compatible_with(&v47));
    }
}
