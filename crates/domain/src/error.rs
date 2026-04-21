// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use core::fmt;

/// Domain-level validation errors.
///
/// These represent invariant violations in value objects and entities.
/// No external error types — this is pure domain logic.
#[derive(Debug, Clone, PartialEq)]
pub enum DomainError {
    /// Extension UUID is missing the required '@' separator.
    InvalidExtensionUuid(String),
    /// Shell version string could not be parsed.
    InvalidShellVersion(String),
    /// Content rating is outside the valid 0.0–5.0 range.
    InvalidContentRating(String),
    /// Pack manifest failed validation.
    InvalidPack(String),
    /// Radius value is out of range.
    InvalidRadius(f64),
    /// Opacity value is out of range.
    InvalidOpacity(f64),
    /// Color hex string is malformed.
    InvalidColor(String),
    /// `text-scaling-factor` is outside the accepted 0.5..=3.0 range.
    InvalidTextScaling(f64),
    /// HiDPI scale factor is not one of the allowed presets.
    InvalidScaleFactor(f64),
    /// `.desktop` app-id contains characters forbidden in a filename
    /// (path separators or leading dot), or is empty.
    InvalidAppId(String),
}

impl fmt::Display for DomainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidExtensionUuid(s) => {
                write!(f, "invalid extension UUID (must contain '@'): {s}")
            }
            Self::InvalidShellVersion(s) => write!(f, "invalid shell version: {s}"),
            Self::InvalidContentRating(s) => {
                write!(f, "content rating out of range 0.0–5.0: {s}")
            }
            Self::InvalidPack(s) => write!(f, "invalid pack: {s}"),
            Self::InvalidRadius(v) => write!(f, "radius out of range 0–48: {v}"),
            Self::InvalidOpacity(v) => write!(f, "opacity out of range 0–100: {v}"),
            Self::InvalidColor(s) => write!(f, "invalid hex color: {s}"),
            Self::InvalidTextScaling(v) => {
                write!(f, "text-scaling-factor out of range 0.5\u{2013}3.0: {v}")
            }
            Self::InvalidScaleFactor(v) => {
                write!(f, "scale factor not in allowed presets (0.5,1.0,1.25,1.5,1.75,2.0): {v}")
            }
            Self::InvalidAppId(s) => write!(f, "invalid .desktop app id: {s}"),
        }
    }
}

impl std::error::Error for DomainError {}
