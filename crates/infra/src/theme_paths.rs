// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Theme / icon / cursor / GTK-override search-path resolver.
//!
//! Canonicalises the full resolution order GNOME honours:
//!
//! ```text
//! Themes:   $XDG_DATA_HOME/themes   (≈ ~/.local/share/themes)
//!           ~/.themes               (legacy but still read by GTK)
//!           for dir in $XDG_DATA_DIRS:   $dir/themes
//!                                (≈ /usr/local/share/themes, /usr/share/themes)
//! Icons:    $XDG_DATA_HOME/icons   · ~/.icons   · $XDG_DATA_DIRS/icons
//! Cursors:  (same dirs as Icons; scan each for a `cursors/` subdir)
//! GTK CSS:  $XDG_CONFIG_HOME/gtk-3.0/gtk.css   (GTK 3 override)
//!           $XDG_CONFIG_HOME/gtk-4.0/gtk.css   (GTK 4 override)
//! ```
//!
//! Paths are emitted in precedence order (user-writable first). The
//! first hit wins for resolution; later hits are *shadowed* copies
//! (see [`resolver::resolve`] for detection).
//!
//! See GXF-010 / GXF-012 in the tracker.

use std::path::{Path, PathBuf};

/// A single search-path entry annotated with whether it's user-writable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchPath {
    pub path: PathBuf,
    pub origin: SearchOrigin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOrigin {
    /// `$XDG_DATA_HOME` — preferred user-writable location.
    XdgUserData,
    /// `~/.themes` / `~/.icons` — legacy user-writable, still honoured
    /// by GTK for backward compatibility.
    LegacyUserHome,
    /// Entries from `$XDG_DATA_DIRS`. Typically system-wide,
    /// non-writable without elevated privileges.
    XdgSystem,
}

impl SearchOrigin {
    /// True when GNOME X can write here without sudo/pkexec.
    pub fn is_user_writable(self) -> bool {
        matches!(self, Self::XdgUserData | Self::LegacyUserHome)
    }
}

/// Resolver that enumerates every search path for each resource kind.
///
/// Construct with [`ResourcePaths::from_env`] to pick up the live
/// `$HOME` / `$XDG_DATA_HOME` / `$XDG_DATA_DIRS` / `$XDG_CONFIG_HOME`,
/// or [`ResourcePaths::explicit`] in tests.
#[derive(Debug, Clone)]
pub struct ResourcePaths {
    home: PathBuf,
    xdg_data_home: PathBuf,
    xdg_data_dirs: Vec<PathBuf>,
    xdg_config_home: PathBuf,
}

impl ResourcePaths {
    /// Build from environment variables, applying XDG defaults when
    /// the variables are unset or empty (per the XDG Base Directory
    /// Specification §4).
    pub fn from_env() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let xdg_data_home = std::env::var_os("XDG_DATA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"));
        let xdg_config_home = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"));
        let xdg_data_dirs = std::env::var("XDG_DATA_DIRS")
            .ok()
            .filter(|v| !v.is_empty())
            .map(|s| s.split(':').map(PathBuf::from).collect())
            .unwrap_or_else(|| {
                // XDG default.
                vec![
                    PathBuf::from("/usr/local/share"),
                    PathBuf::from("/usr/share"),
                ]
            });
        Self { home, xdg_data_home, xdg_data_dirs, xdg_config_home }
    }

    /// Construct with explicit paths — for tests and deterministic
    /// composition roots.
    pub fn explicit(
        home: impl Into<PathBuf>,
        xdg_data_home: impl Into<PathBuf>,
        xdg_data_dirs: Vec<PathBuf>,
        xdg_config_home: impl Into<PathBuf>,
    ) -> Self {
        Self {
            home: home.into(),
            xdg_data_home: xdg_data_home.into(),
            xdg_data_dirs,
            xdg_config_home: xdg_config_home.into(),
        }
    }

    /// Search paths for GTK themes, in resolution order.
    pub fn themes(&self) -> Vec<SearchPath> {
        self.data_backed_paths("themes", Some(".themes"))
    }

    /// Search paths for icon themes, in resolution order.
    pub fn icons(&self) -> Vec<SearchPath> {
        self.data_backed_paths("icons", Some(".icons"))
    }

    /// Search paths for cursor themes. Cursors use the icon search
    /// paths but callers filter to entries with a `cursors/` subdir.
    pub fn cursors(&self) -> Vec<SearchPath> {
        self.icons()
    }

    /// Preferred user-writable target for a new theme/icon/cursor
    /// install. Always returns the `$XDG_DATA_HOME/<subdir>` entry
    /// since that's what GTK4 and icon-cache tooling prefer today,
    /// even when the legacy `~/.themes` path exists.
    pub fn preferred_user_dir(&self, subdir: &str) -> PathBuf {
        self.xdg_data_home.join(subdir)
    }

    /// `gtk.css` override paths, ordered GTK4 first because it's the
    /// primary target today. The GTK3 override is kept so apps still
    /// running on the older toolkit (legacy or sandboxed Electron
    /// wrappers) get themed too.
    pub fn gtk_overrides(&self) -> GtkOverrideFiles {
        GtkOverrideFiles {
            gtk4: self.xdg_config_home.join("gtk-4.0/gtk.css"),
            gtk3: self.xdg_config_home.join("gtk-3.0/gtk.css"),
        }
    }

    fn data_backed_paths(
        &self,
        subdir: &str,
        legacy_home_dir: Option<&str>,
    ) -> Vec<SearchPath> {
        let mut out = Vec::with_capacity(2 + self.xdg_data_dirs.len());
        out.push(SearchPath {
            path: self.xdg_data_home.join(subdir),
            origin: SearchOrigin::XdgUserData,
        });
        if let Some(name) = legacy_home_dir {
            out.push(SearchPath {
                path: self.home.join(name),
                origin: SearchOrigin::LegacyUserHome,
            });
        }
        for dir in &self.xdg_data_dirs {
            out.push(SearchPath {
                path: dir.join(subdir),
                origin: SearchOrigin::XdgSystem,
            });
        }
        out
    }
}

/// GTK override file paths (both toolkit generations). See
/// [`ResourcePaths::gtk_overrides`].
#[derive(Debug, Clone)]
pub struct GtkOverrideFiles {
    pub gtk4: PathBuf,
    pub gtk3: PathBuf,
}

/// Utility: list immediate subdirectory names of `dir`, silently
/// returning an empty list when `dir` doesn't exist or can't be read.
pub fn list_subdirs(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut names = Vec::new();
    for entry in entries.flatten() {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            if let Some(name) = entry.file_name().to_str() {
                names.push(name.to_owned());
            }
        }
    }
    names
}

/// Scan every search path in `paths` and aggregate installed resource
/// names. Entries whose name appears in more than one path are still
/// returned exactly once — shadowing detection lives in
/// [`shadow_map`] (see GXF-012).
pub fn list_all(paths: &[SearchPath]) -> Vec<String> {
    let mut all: Vec<String> = paths
        .iter()
        .flat_map(|p| list_subdirs(&p.path))
        .collect();
    all.sort();
    all.dedup();
    all
}

/// Map each installed resource name to the full list of search paths
/// it was found in. Names appearing in more than one path are
/// *shadowed*: the first entry in the Vec wins at resolution time,
/// later entries are masked. Returned map is unordered; caller can
/// filter `value.len() > 1` to surface conflicts to the user.
pub fn shadow_map(
    paths: &[SearchPath],
) -> std::collections::BTreeMap<String, Vec<SearchPath>> {
    let mut map: std::collections::BTreeMap<String, Vec<SearchPath>> =
        std::collections::BTreeMap::new();
    for sp in paths {
        for name in list_subdirs(&sp.path) {
            map.entry(name).or_default().push(sp.clone());
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rp(
        home: &str,
        xdg_data_home: &str,
        xdg_data_dirs: &[&str],
        xdg_config_home: &str,
    ) -> ResourcePaths {
        ResourcePaths::explicit(
            home,
            xdg_data_home,
            xdg_data_dirs.iter().map(PathBuf::from).collect(),
            xdg_config_home,
        )
    }

    #[test]
    fn themes_order_puts_user_first_legacy_second_system_last() {
        let r = rp(
            "/home/user",
            "/home/user/.local/share",
            &["/usr/local/share", "/usr/share"],
            "/home/user/.config",
        );
        let paths = r.themes();
        assert_eq!(paths.len(), 4);
        assert_eq!(paths[0].path, PathBuf::from("/home/user/.local/share/themes"));
        assert_eq!(paths[0].origin, SearchOrigin::XdgUserData);
        assert_eq!(paths[1].path, PathBuf::from("/home/user/.themes"));
        assert_eq!(paths[1].origin, SearchOrigin::LegacyUserHome);
        assert_eq!(paths[2].path, PathBuf::from("/usr/local/share/themes"));
        assert_eq!(paths[2].origin, SearchOrigin::XdgSystem);
        assert_eq!(paths[3].path, PathBuf::from("/usr/share/themes"));
        assert_eq!(paths[3].origin, SearchOrigin::XdgSystem);
    }

    #[test]
    fn icons_and_cursors_share_the_same_search_dirs() {
        let r = rp(
            "/home/user",
            "/home/user/.local/share",
            &["/usr/share"],
            "/home/user/.config",
        );
        assert_eq!(r.icons(), r.cursors());
    }

    #[test]
    fn gtk_overrides_point_at_canonical_xdg_config_paths() {
        let r = rp(
            "/home/user",
            "/home/user/.local/share",
            &["/usr/share"],
            "/home/user/.config",
        );
        let g = r.gtk_overrides();
        assert_eq!(g.gtk4, PathBuf::from("/home/user/.config/gtk-4.0/gtk.css"));
        assert_eq!(g.gtk3, PathBuf::from("/home/user/.config/gtk-3.0/gtk.css"));
    }

    #[test]
    fn origin_user_writable_classification() {
        assert!(SearchOrigin::XdgUserData.is_user_writable());
        assert!(SearchOrigin::LegacyUserHome.is_user_writable());
        assert!(!SearchOrigin::XdgSystem.is_user_writable());
    }

    #[test]
    fn preferred_user_dir_is_always_xdg_data_home() {
        let r = rp(
            "/h",
            "/h/.local/share",
            &["/usr/share"],
            "/h/.config",
        );
        assert_eq!(
            r.preferred_user_dir("themes"),
            PathBuf::from("/h/.local/share/themes")
        );
    }

    #[test]
    fn shadow_map_detects_same_theme_in_multiple_paths() {
        // Build a live tempdir with the same subdir name in two paths.
        let tmp = tempfile::TempDir::new().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir_all(a.join("Adwaita")).unwrap();
        std::fs::create_dir_all(b.join("Adwaita")).unwrap();
        std::fs::create_dir_all(a.join("OnlyA")).unwrap();

        let paths = vec![
            SearchPath { path: a.clone(), origin: SearchOrigin::XdgUserData },
            SearchPath { path: b.clone(), origin: SearchOrigin::XdgSystem },
        ];
        let map = shadow_map(&paths);
        assert_eq!(map["Adwaita"].len(), 2, "Adwaita should be shadowed");
        assert_eq!(map["OnlyA"].len(), 1);
        // User-writable path comes first (it's the one that wins).
        assert_eq!(map["Adwaita"][0].origin, SearchOrigin::XdgUserData);
    }

    #[test]
    fn list_all_dedups_across_paths() {
        let tmp = tempfile::TempDir::new().unwrap();
        let a = tmp.path().join("a");
        let b = tmp.path().join("b");
        std::fs::create_dir_all(a.join("Adwaita")).unwrap();
        std::fs::create_dir_all(b.join("Adwaita")).unwrap();
        let paths = vec![
            SearchPath { path: a, origin: SearchOrigin::XdgUserData },
            SearchPath { path: b, origin: SearchOrigin::XdgSystem },
        ];
        let names = list_all(&paths);
        assert_eq!(names, vec!["Adwaita".to_owned()]);
    }

    #[test]
    fn xdg_defaults_match_spec_when_env_unset() {
        // Isolate HOME only — leave XDG_DATA_HOME/CONFIG_HOME/DATA_DIRS
        // unset so the defaults fire. `from_env` reads lazily so we
        // have to do this inside a single test context.
        let prev_home = std::env::var_os("HOME");
        let prev_xdg_data_home = std::env::var_os("XDG_DATA_HOME");
        let prev_xdg_cfg_home = std::env::var_os("XDG_CONFIG_HOME");
        let prev_xdg_data_dirs = std::env::var_os("XDG_DATA_DIRS");

        // SAFETY: set/unset around the call; restored in the fn below.
        unsafe {
            std::env::set_var("HOME", "/tmp/fake-home");
            std::env::remove_var("XDG_DATA_HOME");
            std::env::remove_var("XDG_CONFIG_HOME");
            std::env::remove_var("XDG_DATA_DIRS");
        }
        let r = ResourcePaths::from_env();
        // Restore before asserting to keep the suite hermetic when
        // assertions panic.
        unsafe {
            match prev_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            match prev_xdg_data_home {
                Some(v) => std::env::set_var("XDG_DATA_HOME", v),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
            match prev_xdg_cfg_home {
                Some(v) => std::env::set_var("XDG_CONFIG_HOME", v),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
            match prev_xdg_data_dirs {
                Some(v) => std::env::set_var("XDG_DATA_DIRS", v),
                None => std::env::remove_var("XDG_DATA_DIRS"),
            }
        }

        assert_eq!(
            r.gtk_overrides().gtk4,
            PathBuf::from("/tmp/fake-home/.config/gtk-4.0/gtk.css")
        );
        assert_eq!(
            r.themes()[0].path,
            PathBuf::from("/tmp/fake-home/.local/share/themes")
        );
        // XDG_DATA_DIRS default per spec.
        let system_paths: Vec<PathBuf> = r
            .themes()
            .into_iter()
            .filter(|p| p.origin == SearchOrigin::XdgSystem)
            .map(|p| p.path)
            .collect();
        assert_eq!(
            system_paths,
            vec![
                PathBuf::from("/usr/local/share/themes"),
                PathBuf::from("/usr/share/themes"),
            ]
        );
    }
}
