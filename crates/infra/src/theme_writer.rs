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
/// Writes both the GTK4 override (`gtk-4.0/gtk.css`, primary target)
/// and the GTK3 override (`gtk-3.0/gtk.css`, for legacy and sandboxed
/// Chromium/Electron apps that still render under GTK3) on every
/// `write_gtk_css`. Both files are backed up once before the first
/// overwrite so `clear_overrides` can restore the user's pre-GNOME-X
/// state.
///
/// Related tracker items: GXF-010 (multi-path resolution).
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

    fn gtk_override_files(&self) -> GtkOverrideFiles {
        self.paths.gtk_overrides()
    }
}

impl ThemeWriter for FilesystemThemeWriter {
    fn write_gtk_css(&self, css: &str) -> Result<(), AppError> {
        // Both GTK4 (primary) and GTK3 (for legacy / sandboxed apps
        // that still render on the older toolkit) get the same CSS.
        // Each path is independently backed up once.
        let targets = self.gtk_override_files();
        for path in [&targets.gtk4, &targets.gtk3] {
            write_gtk_override(path, css)?;
        }
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

        writer.write_gtk_css("/* test */").unwrap();

        let gtk4 = dir.path().join(".config/gtk-4.0/gtk.css");
        let gtk3 = dir.path().join(".config/gtk-3.0/gtk.css");
        assert!(gtk4.exists(), "missing {}", gtk4.display());
        assert!(gtk3.exists(), "missing {}", gtk3.display());
        assert_eq!(std::fs::read_to_string(&gtk4).unwrap(), "/* test */");
        assert_eq!(std::fs::read_to_string(&gtk3).unwrap(), "/* test */");
    }

    #[test]
    fn first_write_backs_up_user_authored_gtk_css() {
        let dir = TempDir::new().unwrap();
        let writer = tmp_writer(dir.path());

        let gtk4 = dir.path().join(".config/gtk-4.0/gtk.css");
        std::fs::create_dir_all(gtk4.parent().unwrap()).unwrap();
        std::fs::write(&gtk4, "/* user-authored */").unwrap();

        writer.write_gtk_css("/* gnome-x */").unwrap();

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

        writer.write_gtk_css("/* new */").unwrap();
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

        writer.write_gtk_css("/* gnome-x */").unwrap();
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
        writer.write_gtk_css("/* gnome-x */").unwrap();
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
