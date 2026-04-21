// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Use case: apply a [`WallpaperSlideshow`] so GNOME Shell plays it
//! back on its own.
//!
//! Flow:
//!   1. Adapter writes `<name>.xml` to disk.
//!   2. We turn the returned absolute path into a `file://` URI.
//!   3. `picture-uri` + `picture-uri-dark` are pointed at that URI so
//!      GNOME begins the rotation on the next compositor tick.
//!
//! Step 3 is a best-effort write: failing to update GSettings is a
//! recoverable toast for the UI (e.g. running inside a sandbox with no
//! dconf). The XML is still on disk and can be re-applied later.

use crate::ports::{AppearanceSettings, WallpaperSlideshowWriter};
use crate::AppError;
use gnomex_domain::WallpaperSlideshow;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct WallpaperSlideshowUseCase {
    writer: Arc<dyn WallpaperSlideshowWriter>,
    appearance: Arc<dyn AppearanceSettings>,
}

/// Outcome of a successful [`WallpaperSlideshowUseCase::apply`].
///
/// Split into XML-write and GSettings-write so the UI can surface a
/// precise toast ("slideshow saved but couldn't update wallpaper
/// setting — is dconf available?") instead of collapsing to a generic
/// failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlideshowApplied {
    pub xml_path: PathBuf,
    pub uri: String,
    pub gsettings_updated: bool,
}

impl WallpaperSlideshowUseCase {
    pub fn new(
        writer: Arc<dyn WallpaperSlideshowWriter>,
        appearance: Arc<dyn AppearanceSettings>,
    ) -> Self {
        Self { writer, appearance }
    }

    pub fn apply(&self, slideshow: &WallpaperSlideshow) -> Result<SlideshowApplied, AppError> {
        let xml_path = self.writer.write(slideshow)?;
        let uri = file_uri(&xml_path);
        let gsettings_updated = match self.appearance.set_wallpaper(&uri) {
            Ok(()) => true,
            Err(e) => {
                tracing::warn!(
                    "slideshow XML written to {} but set_wallpaper failed: {e}",
                    xml_path.display()
                );
                false
            }
        };
        Ok(SlideshowApplied {
            xml_path,
            uri,
            gsettings_updated,
        })
    }

    pub fn delete(&self, name: &str) -> Result<(), AppError> {
        self.writer.delete(name)
    }
}

/// Convert an absolute filesystem path into a `file://` URI. Pure —
/// no I/O. Separated out so the UI can pre-compute the URI for
/// previews without invoking the use case.
pub fn file_uri(p: &Path) -> String {
    // We only feed absolute paths into the slideshow domain type, so
    // a naïve `file://` prefix is correct. Percent-encode spaces and
    // other ASCII-reserved characters that would otherwise break
    // GSettings' URI parser.
    let s = p.to_string_lossy();
    let mut out = String::with_capacity(s.len() + 8);
    out.push_str("file://");
    for c in s.chars() {
        match c {
            'A'..='Z' | 'a'..='z' | '0'..='9' | '/' | '-' | '_' | '.' | '~' => out.push(c),
            _ => {
                let mut buf = [0u8; 4];
                for b in c.encode_utf8(&mut buf).bytes() {
                    out.push('%');
                    out.push_str(&format!("{b:02X}"));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AppError;
    use gnomex_domain::{WallpaperSlideshow, WallpaperSlideshowError};
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    #[derive(Default)]
    struct SpyAppearance {
        wallpaper: Mutex<Option<String>>,
        fail: Mutex<Option<String>>,
    }

    impl AppearanceSettings for SpyAppearance {
        fn get_gtk_theme(&self) -> Result<String, AppError> {
            Ok(String::new())
        }
        fn set_gtk_theme(&self, _: &str) -> Result<(), AppError> {
            Ok(())
        }
        fn get_icon_theme(&self) -> Result<String, AppError> {
            Ok(String::new())
        }
        fn set_icon_theme(&self, _: &str) -> Result<(), AppError> {
            Ok(())
        }
        fn get_cursor_theme(&self) -> Result<String, AppError> {
            Ok(String::new())
        }
        fn set_cursor_theme(&self, _: &str) -> Result<(), AppError> {
            Ok(())
        }
        fn get_shell_theme(&self) -> Result<String, AppError> {
            Ok(String::new())
        }
        fn set_shell_theme(&self, _: &str) -> Result<(), AppError> {
            Ok(())
        }
        fn get_wallpaper(&self) -> Result<String, AppError> {
            Ok(self.wallpaper.lock().unwrap().clone().unwrap_or_default())
        }
        fn set_wallpaper(&self, uri: &str) -> Result<(), AppError> {
            if let Some(msg) = self.fail.lock().unwrap().clone() {
                return Err(AppError::Settings(msg));
            }
            *self.wallpaper.lock().unwrap() = Some(uri.to_owned());
            Ok(())
        }
    }

    #[derive(Default)]
    struct SpyWriter {
        written: Mutex<Vec<String>>,
        deleted: Mutex<Vec<String>>,
        path_for: Mutex<Option<PathBuf>>,
        fail_write: Mutex<bool>,
    }

    impl WallpaperSlideshowWriter for SpyWriter {
        fn write(
            &self,
            slideshow: &WallpaperSlideshow,
        ) -> Result<PathBuf, AppError> {
            if *self.fail_write.lock().unwrap() {
                return Err(AppError::Storage("boom".into()));
            }
            self.written.lock().unwrap().push(slideshow.name().into());
            Ok(self
                .path_for
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| {
                    PathBuf::from(format!(
                        "/tmp/gnome-background-properties/{}.xml",
                        slideshow.name()
                    ))
                }))
        }
        fn delete(&self, name: &str) -> Result<(), AppError> {
            self.deleted.lock().unwrap().push(name.into());
            Ok(())
        }
    }

    fn sample() -> WallpaperSlideshow {
        WallpaperSlideshow::new(
            "dusk",
            vec![
                PathBuf::from("/a.jpg"),
                PathBuf::from("/b.jpg"),
            ],
            600,
            false,
        )
        .unwrap()
    }

    #[test]
    fn apply_writes_xml_and_updates_appearance() {
        let writer = Arc::new(SpyWriter::default());
        *writer.path_for.lock().unwrap() = Some(PathBuf::from("/x/dusk.xml"));
        let appearance = Arc::new(SpyAppearance::default());
        let uc =
            WallpaperSlideshowUseCase::new(writer.clone(), appearance.clone());

        let out = uc.apply(&sample()).expect("apply");
        assert_eq!(out.xml_path, Path::new("/x/dusk.xml"));
        assert_eq!(out.uri, "file:///x/dusk.xml");
        assert!(out.gsettings_updated);
        assert_eq!(
            appearance.wallpaper.lock().unwrap().as_deref(),
            Some("file:///x/dusk.xml")
        );
        assert_eq!(writer.written.lock().unwrap().as_slice(), &["dusk"]);
    }

    #[test]
    fn apply_returns_partial_success_when_gsettings_fails() {
        let writer = Arc::new(SpyWriter::default());
        *writer.path_for.lock().unwrap() = Some(PathBuf::from("/x/dusk.xml"));
        let appearance = Arc::new(SpyAppearance::default());
        *appearance.fail.lock().unwrap() = Some("no dconf".into());
        let uc =
            WallpaperSlideshowUseCase::new(writer.clone(), appearance.clone());

        let out = uc.apply(&sample()).expect("apply");
        assert!(!out.gsettings_updated);
        assert_eq!(writer.written.lock().unwrap().len(), 1);
    }

    #[test]
    fn apply_bubbles_writer_errors_without_touching_gsettings() {
        let writer = Arc::new(SpyWriter::default());
        *writer.fail_write.lock().unwrap() = true;
        let appearance = Arc::new(SpyAppearance::default());
        let uc =
            WallpaperSlideshowUseCase::new(writer.clone(), appearance.clone());

        let err = uc.apply(&sample()).unwrap_err();
        assert!(matches!(err, AppError::Storage(_)));
        assert!(appearance.wallpaper.lock().unwrap().is_none());
    }

    #[test]
    fn delete_routes_to_adapter() {
        let writer = Arc::new(SpyWriter::default());
        let appearance = Arc::new(SpyAppearance::default());
        let uc = WallpaperSlideshowUseCase::new(writer.clone(), appearance);
        uc.delete("dusk").unwrap();
        assert_eq!(writer.deleted.lock().unwrap().as_slice(), &["dusk"]);
    }

    #[test]
    fn file_uri_percent_encodes_spaces_and_unicode() {
        let u = file_uri(Path::new("/home/jo/My Pics/\u{00e9}.jpg"));
        assert_eq!(u, "file:///home/jo/My%20Pics/%C3%A9.jpg");
    }

    #[test]
    fn sanity_slideshow_invariants_surface_from_domain() {
        let err = WallpaperSlideshow::new("x", vec![], 60, false).unwrap_err();
        assert_eq!(err, WallpaperSlideshowError::TooFewPictures);
    }
}
