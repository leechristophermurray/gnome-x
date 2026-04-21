// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::theme_paths::{GtkOverrideFiles, ResourcePaths};
use gnomex_app::ports::ThemeWriter;
use gnomex_app::AppError;
use std::path::{Path, PathBuf};

const BACKUP_SUFFIX: &str = ".gnomex-backup";
const SHELL_THEME_SUBDIR: &str = "themes";
const CUSTOM_SHELL_THEME_NAME: &str = "GNOME-X-Custom";

/// Concrete adapter: write theme CSS to the local filesystem.
///
/// Writes the GTK4 override (`gtk-4.0/gtk.css`, primary target) and a
/// *separately-generated* GTK3 override (`gtk-3.0/gtk.css`, for legacy
/// and sandboxed Chromium/Electron apps that still render under GTK3)
/// on every `write_gtk_css`. The two payloads target different token
/// namespaces (`@window_bg_color` vs `@theme_bg_color`) and different
/// widget selectors (`.navigation-sidebar` vs `.sidebar`, etc.), so
/// they are produced independently by the CSS generator (GXF-001).
/// Both files are backed up once before the first overwrite so
/// `clear_overrides` can restore the user's pre-GNOME-X state.
///
/// Related tracker items: GXF-001 (GTK3 vs GTK4 divergence),
/// GXF-010 (multi-path resolution).
pub struct FilesystemThemeWriter {
    paths: ResourcePaths,
    shell_theme_dir: PathBuf,
}

impl FilesystemThemeWriter {
    pub fn new() -> Self {
        let paths = ResourcePaths::from_env();
        let shell_theme_dir = paths.preferred_user_dir(SHELL_THEME_SUBDIR);
        Self {
            paths,
            shell_theme_dir,
        }
    }

    /// Construct with explicit [`ResourcePaths`] and a `shell_theme_dir`
    /// override. Used by integration tests to route writes into a
    /// tempdir without mutating `$HOME` / `$XDG_*` env vars — each
    /// parallel test sees its own isolated path table.
    pub fn with_paths(paths: ResourcePaths, shell_theme_dir: PathBuf) -> Self {
        Self {
            paths,
            shell_theme_dir,
        }
    }

    fn gtk_override_files(&self) -> GtkOverrideFiles {
        self.paths.gtk_overrides()
    }
}

impl ThemeWriter for FilesystemThemeWriter {
    fn write_gtk_css(&self, gtk4_css: &str, gtk3_css: &str) -> Result<(), AppError> {
        // GTK4 gets the Libadwaita-flavoured payload; GTK3 gets a
        // parallel payload built against the legacy `@theme_*` token
        // namespace and GTK3 widget tree. Each path is independently
        // backed up once.
        let targets = self.gtk_override_files();
        write_gtk_override(&targets.gtk4, gtk4_css)?;
        write_gtk_override(&targets.gtk3, gtk3_css)?;
        Ok(())
    }

    fn write_shell_css(&self, css: &str, theme_name: &str) -> Result<(), AppError> {
        let dir = self.shell_theme_dir.join(theme_name).join("gnome-shell");
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::Settings(format!("mkdir: {e}")))?;
        std::fs::write(dir.join("gnome-shell.css"), css)
            .map_err(|e| AppError::Settings(format!("write shell css: {e}")))?;
        tracing::info!("wrote Shell CSS to {}", dir.display());
        Ok(())
    }

    fn clear_overrides(&self) -> Result<(), AppError> {
        let targets = self.gtk_override_files();
        for path in [&targets.gtk4, &targets.gtk3] {
            restore_or_remove_gtk_override(path);
        }
        let custom_dir = self.shell_theme_dir.join(CUSTOM_SHELL_THEME_NAME);
        let _ = std::fs::remove_dir_all(custom_dir);
        tracing::info!("cleared theme overrides");
        Ok(())
    }
}

/// Write `css` to `path`, first backing up any pre-existing file not
/// authored by GNOME X.
fn write_gtk_override(path: &Path, css: &str) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| AppError::Settings(format!("mkdir {}: {e}", parent.display())))?;
    }
    let backup = backup_path(path);
    if path.exists() && !backup.exists() {
        if let Ok(content) = std::fs::read_to_string(path) {
            if !content.contains("GNOME X") {
                std::fs::copy(path, &backup).ok();
                tracing::info!("backed up existing {}", path.display());
            }
        }
    }
    std::fs::write(path, css)
        .map_err(|e| AppError::Settings(format!("write {}: {e}", path.display())))?;
    tracing::info!("wrote GTK CSS to {}", path.display());
    Ok(())
}

/// Restore the user's original override from its backup, or remove
/// our copy if no backup exists (meaning the user didn't have one
/// pre-GNOME-X).
fn restore_or_remove_gtk_override(path: &Path) {
    let backup = backup_path(path);
    if backup.exists() {
        if std::fs::rename(&backup, path).is_ok() {
            tracing::info!("restored {} from backup", path.display());
        }
    } else {
        let _ = std::fs::remove_file(path);
    }
}

fn backup_path(p: &Path) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(BACKUP_SUFFIX);
    PathBuf::from(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gnomex_app::ports::ThemeWriter;
    use tempfile::TempDir;

    fn tmp_writer(root: &Path) -> FilesystemThemeWriter {
        FilesystemThemeWriter {
            paths: ResourcePaths::explicit(
                root,
                root.join(".local/share"),
                vec![],
                root.join(".config"),
            ),
            shell_theme_dir: root.join(".local/share/themes"),
        }
    }

    #[test]
    fn write_gtk_css_creates_both_gtk3_and_gtk4_files() {
        let dir = TempDir::new().unwrap();
        let writer = tmp_writer(dir.path());

        writer.write_gtk_css("/* gtk4 */", "/* gtk3 */").unwrap();

        let gtk4 = dir.path().join(".config/gtk-4.0/gtk.css");
        let gtk3 = dir.path().join(".config/gtk-3.0/gtk.css");
        assert!(gtk4.exists(), "missing {}", gtk4.display());
        assert!(gtk3.exists(), "missing {}", gtk3.display());
        assert_eq!(std::fs::read_to_string(&gtk4).unwrap(), "/* gtk4 */");
        assert_eq!(std::fs::read_to_string(&gtk3).unwrap(), "/* gtk3 */");
    }

    #[test]
    fn write_gtk_css_routes_payloads_to_correct_targets() {
        // Pins the core invariant of this feature: GTK4 CSS must land
        // at `gtk-4.0/gtk.css`, GTK3 CSS must land at `gtk-3.0/gtk.css`,
        // and the two must never bleed into each other.
        let dir = TempDir::new().unwrap();
        let writer = tmp_writer(dir.path());

        writer
            .write_gtk_css(
                "/* GNOME X — GTK4 overrides */\n@define-color window_bg_color #abc;",
                "/* GNOME X — GTK3 overrides */\n@define-color theme_bg_color #def;",
            )
            .unwrap();

        let gtk4 = std::fs::read_to_string(dir.path().join(".config/gtk-4.0/gtk.css")).unwrap();
        let gtk3 = std::fs::read_to_string(dir.path().join(".config/gtk-3.0/gtk.css")).unwrap();
        assert!(gtk4.contains("window_bg_color"), "GTK4 file missing GTK4 token");
        assert!(!gtk4.contains("theme_bg_color"), "GTK4 file contaminated with GTK3 token");
        assert!(gtk3.contains("theme_bg_color"), "GTK3 file missing GTK3 token");
        assert!(!gtk3.contains("window_bg_color"), "GTK3 file contaminated with GTK4 token");
    }

    #[test]
    fn first_write_backs_up_user_authored_gtk_css() {
        let dir = TempDir::new().unwrap();
        let writer = tmp_writer(dir.path());

        let gtk4 = dir.path().join(".config/gtk-4.0/gtk.css");
        std::fs::create_dir_all(gtk4.parent().unwrap()).unwrap();
        std::fs::write(&gtk4, "/* user-authored */").unwrap();

        writer.write_gtk_css("/* gnome-x */", "/* gnome-x-3 */").unwrap();

        let backup = dir
            .path()
            .join(".config/gtk-4.0/gtk.css.gnomex-backup");
        assert!(backup.exists(), "expected backup at {}", backup.display());
        assert_eq!(
            std::fs::read_to_string(&backup).unwrap(),
            "/* user-authored */"
        );
    }

    #[test]
    fn writer_does_not_back_up_its_own_output() {
        let dir = TempDir::new().unwrap();
        let writer = tmp_writer(dir.path());

        // Seed with a file that looks like it's ours.
        let gtk4 = dir.path().join(".config/gtk-4.0/gtk.css");
        std::fs::create_dir_all(gtk4.parent().unwrap()).unwrap();
        std::fs::write(&gtk4, "/* GNOME X — Shell overrides */").unwrap();

        writer.write_gtk_css("/* new */", "/* new-3 */").unwrap();
        let backup = dir
            .path()
            .join(".config/gtk-4.0/gtk.css.gnomex-backup");
        assert!(!backup.exists(), "should not back up our own output");
    }

    #[test]
    fn clear_overrides_restores_backup_when_present() {
        let dir = TempDir::new().unwrap();
        let writer = tmp_writer(dir.path());

        let gtk4 = dir.path().join(".config/gtk-4.0/gtk.css");
        std::fs::create_dir_all(gtk4.parent().unwrap()).unwrap();
        std::fs::write(&gtk4, "/* user-authored */").unwrap();

        writer.write_gtk_css("/* gnome-x */", "/* gnome-x-3 */").unwrap();
        writer.clear_overrides().unwrap();

        // Backup should be gone, gtk.css should hold the original.
        assert_eq!(
            std::fs::read_to_string(&gtk4).unwrap(),
            "/* user-authored */"
        );
        assert!(
            !dir.path()
                .join(".config/gtk-4.0/gtk.css.gnomex-backup")
                .exists()
        );
    }

    #[test]
    fn clear_overrides_removes_file_when_no_backup() {
        let dir = TempDir::new().unwrap();
        let writer = tmp_writer(dir.path());

        // No prior user file; writer writes its own, then clears.
        writer.write_gtk_css("/* gnome-x */", "/* gnome-x-3 */").unwrap();
        writer.clear_overrides().unwrap();

        assert!(!dir.path().join(".config/gtk-4.0/gtk.css").exists());
        assert!(!dir.path().join(".config/gtk-3.0/gtk.css").exists());
    }

    #[test]
    fn write_shell_css_creates_custom_theme_dir() {
        let dir = TempDir::new().unwrap();
        let writer = tmp_writer(dir.path());

        writer
            .write_shell_css("/* shell */", "GNOME-X-Custom")
            .unwrap();

        let css = dir
            .path()
            .join(".local/share/themes/GNOME-X-Custom/gnome-shell/gnome-shell.css");
        assert!(css.exists(), "missing {}", css.display());
    }
}
