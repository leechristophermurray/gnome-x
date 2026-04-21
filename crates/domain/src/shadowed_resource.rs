// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Shadowed resource reports — surfaced to the user when the same
//! theme / icon pack / cursor is installed in more than one location
//! on the XDG data search path.
//!
//! Per the freedesktop icon-theme spec (and GTK's theme lookup), the
//! first path in search order wins; later copies are *shadowed*. A
//! common user-visible failure: installing a theme to
//! `~/.local/share/themes/<Name>` when `/usr/share/themes/<Name>`
//! already exists and is earlier in the search order — the user's
//! fresh install is silently masked.
//!
//! The [`crate::ShadowedResource`] value-object captures that
//! situation as pure data; the infrastructure layer populates it by
//! walking the actual filesystem. See GXF-012.

use std::path::PathBuf;

/// Which family of resource is being shadow-checked. Matches the three
/// directory hierarchies GTK/GNOME scan independently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    /// GTK widget themes — `themes/<Name>/gtk-N.N/gtk.css`.
    Theme,
    /// Icon themes — `icons/<Name>/index.theme`.
    Icon,
    /// Cursor themes — `icons/<Name>/cursors/`.
    Cursor,
}

impl ResourceKind {
    /// Short, user-facing label ("Theme", "Icon pack", "Cursor").
    pub fn label(&self) -> &'static str {
        match self {
            ResourceKind::Theme => "Theme",
            ResourceKind::Icon => "Icon pack",
            ResourceKind::Cursor => "Cursor",
        }
    }

    /// XDG-style subdirectory name (`"themes"` / `"icons"`).
    pub fn xdg_subdir(&self) -> &'static str {
        match self {
            ResourceKind::Theme => "themes",
            // Cursors live under icons/<Name>/cursors — they share the
            // icons directory tree, but are discriminated in the
            // shadow walk by the presence of a `cursors/` subdir.
            ResourceKind::Icon | ResourceKind::Cursor => "icons",
        }
    }
}

/// One place on disk where a resource lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowedLocation {
    pub path: PathBuf,
    /// `true` when the directory is under `$XDG_DATA_HOME` or the
    /// legacy `~/.themes` / `~/.icons` path — i.e. GNOME X (and the
    /// user) can modify it without elevated privileges. `false` for
    /// system dirs under `$XDG_DATA_DIRS` (typically `/usr/share/...`).
    pub user_writable: bool,
}

/// A named resource that appears in more than one search path. The
/// `locations` vector is **ordered by search precedence** — the first
/// entry is the one GTK/GNOME resolves to; all subsequent entries are
/// masked copies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowedResource {
    pub kind: ResourceKind,
    /// The directory name (e.g. `"Adwaita"`, `"Yaru-blue"`) — this is
    /// what the user and GSettings refer to the resource by.
    pub name: String,
    /// All matching locations, winning one first. Length >= 2 by
    /// construction (length-1 lists are not shadowed — callers should
    /// filter before surfacing).
    pub locations: Vec<ShadowedLocation>,
}

impl ShadowedResource {
    /// The location GTK will actually resolve to. Always the first
    /// entry since `locations` is sorted in search order.
    pub fn winning_location(&self) -> &ShadowedLocation {
        // Invariant: `locations` is never empty — a ShadowedResource
        // only makes sense with at least two entries. Callers that
        // build this struct must uphold that.
        &self.locations[0]
    }

    /// Locations that are masked by the winner. Always `locations[1..]`.
    pub fn masked_locations(&self) -> &[ShadowedLocation] {
        &self.locations[1..]
    }

    /// Heuristic: the most common user-confusing case is a fresh
    /// install to the user-writable path that loses to an older
    /// system-wide copy. This returns `true` when the winner is
    /// system-wide but a user-writable copy also exists.
    pub fn user_install_masked(&self) -> bool {
        !self.winning_location().user_writable
            && self.masked_locations().iter().any(|l| l.user_writable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn loc(path: &str, writable: bool) -> ShadowedLocation {
        ShadowedLocation {
            path: PathBuf::from(path),
            user_writable: writable,
        }
    }

    #[test]
    fn winning_location_is_first_entry() {
        let r = ShadowedResource {
            kind: ResourceKind::Theme,
            name: "Adwaita".into(),
            locations: vec![
                loc("/home/u/.local/share/themes/Adwaita", true),
                loc("/usr/share/themes/Adwaita", false),
            ],
        };
        assert_eq!(r.winning_location().path, PathBuf::from("/home/u/.local/share/themes/Adwaita"));
        assert_eq!(r.masked_locations().len(), 1);
    }

    #[test]
    fn user_install_masked_flags_the_common_failure_case() {
        // System copy wins because the user's copy is actually on an
        // unusual path ordering (e.g. XDG_DATA_DIRS includes user dir
        // at a lower-priority slot) — or more realistically: the user
        // installed the theme to ~/.themes but their gtk version only
        // honours /usr/share/themes for shell themes.
        let system_wins = ShadowedResource {
            kind: ResourceKind::Theme,
            name: "Yaru".into(),
            locations: vec![
                loc("/usr/share/themes/Yaru", false),
                loc("/home/u/.local/share/themes/Yaru", true),
            ],
        };
        assert!(system_wins.user_install_masked());

        let user_wins = ShadowedResource {
            kind: ResourceKind::Theme,
            name: "Yaru".into(),
            locations: vec![
                loc("/home/u/.local/share/themes/Yaru", true),
                loc("/usr/share/themes/Yaru", false),
            ],
        };
        assert!(!user_wins.user_install_masked());
    }

    #[test]
    fn kind_labels_and_subdirs() {
        assert_eq!(ResourceKind::Theme.xdg_subdir(), "themes");
        assert_eq!(ResourceKind::Icon.xdg_subdir(), "icons");
        assert_eq!(ResourceKind::Cursor.xdg_subdir(), "icons");
        assert_eq!(ResourceKind::Cursor.label(), "Cursor");
    }
}
