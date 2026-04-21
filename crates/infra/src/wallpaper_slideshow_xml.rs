// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Concrete adapter: emit a [`WallpaperSlideshow`] as a GNOME
//! `<background>` XML file under
//! `$XDG_DATA_HOME/gnome-background-properties/<name>.xml`.
//!
//! GNOME Shell + gnome-backgrounds parse this format natively, so
//! once the XML exists on disk and `picture-uri` references it, the
//! compositor handles playback. We deliberately generate absolute
//! picture paths inside each `<file>` / `<from>` / `<to>` element so
//! the XML is location-independent of whoever consumes it — unlike
//! the stock XMLs that rely on `/usr/share/backgrounds/` being on
//! path.
//!
//! The writer is atomic: XML is rendered into a sibling `*.xml.tmp`
//! under the same directory and `rename(2)`d into place so readers
//! never observe half-written content.

use gnomex_app::ports::WallpaperSlideshowWriter;
use gnomex_app::AppError;
use gnomex_domain::{
    slideshow_xml_relative_path, FixedClock, SlideshowClock, StartTime,
    WallpaperSlideshow,
};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Default XDG-compliant adapter: writes to
/// `$XDG_DATA_HOME/gnome-background-properties/`.
pub struct XdgWallpaperSlideshowWriter {
    /// Root directory that holds `*.xml` slideshow manifests. Usually
    /// `$XDG_DATA_HOME/gnome-background-properties`.
    dir: PathBuf,
    /// Injected clock. Production code uses the wall-clock constant
    /// [`StartTime::EPOCH`] which keeps `<starttime>` in the past
    /// (GNOME rewinds to the most recent cycle boundary on read) and
    /// avoids leaking the host clock into the emitted XML. Tests can
    /// swap in a [`FixedClock`] — already the default, so the same
    /// knob covers both worlds.
    clock: Box<dyn SlideshowClock>,
}

impl XdgWallpaperSlideshowWriter {
    /// Production constructor: derive the directory from
    /// `XDG_DATA_HOME` with the usual fallback to `~/.local/share`.
    pub fn new() -> Self {
        let base = directories::BaseDirs::new()
            .map(|d| d.data_local_dir().to_owned())
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let dir = base.join("gnome-background-properties");
        Self::with_dir(dir)
    }

    /// Test / override constructor: supply an explicit directory.
    /// Pairs with [`tempfile::TempDir`] in integration tests.
    pub fn with_dir(dir: impl Into<PathBuf>) -> Self {
        Self {
            dir: dir.into(),
            clock: Box::new(FixedClock(StartTime::EPOCH)),
        }
    }

    /// Override the clock — primarily for hermetic tests that want to
    /// assert the exact `<starttime>` bytes in the rendered XML.
    pub fn with_clock(mut self, clock: Box<dyn SlideshowClock>) -> Self {
        self.clock = clock;
        self
    }

    fn xml_path(&self, name: &str) -> PathBuf {
        // Re-use the domain-level relative path helper so both
        // adapter and any future callers converge on the same layout.
        let rel = slideshow_xml_relative_path(name);
        // `slideshow_xml_relative_path` prepends the subdir; strip it
        // because our `dir` already points at that subdir.
        let stem = rel.file_name().expect("helper yields a file name");
        self.dir.join(stem)
    }
}

impl Default for XdgWallpaperSlideshowWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl WallpaperSlideshowWriter for XdgWallpaperSlideshowWriter {
    fn write(&self, slideshow: &WallpaperSlideshow) -> Result<PathBuf, AppError> {
        std::fs::create_dir_all(&self.dir).map_err(|e| {
            AppError::Storage(format!(
                "failed to create {}: {e}",
                self.dir.display()
            ))
        })?;

        let xml = render_slideshow_xml(slideshow, &*self.clock);
        let final_path = self.xml_path(slideshow.name());

        // Atomic write: tmp sibling + rename.
        let tmp_path = final_path.with_extension("xml.tmp");
        {
            let mut f = std::fs::File::create(&tmp_path).map_err(|e| {
                AppError::Storage(format!(
                    "create {}: {e}",
                    tmp_path.display()
                ))
            })?;
            f.write_all(xml.as_bytes()).map_err(|e| {
                AppError::Storage(format!(
                    "write {}: {e}",
                    tmp_path.display()
                ))
            })?;
            f.sync_all().ok();
        }
        std::fs::rename(&tmp_path, &final_path).map_err(|e| {
            AppError::Storage(format!(
                "rename {} -> {}: {e}",
                tmp_path.display(),
                final_path.display()
            ))
        })?;

        tracing::info!(
            "wrote slideshow '{}' ({} pictures) to {}",
            slideshow.name(),
            slideshow.pictures().len(),
            final_path.display()
        );
        Ok(final_path)
    }

    fn delete(&self, name: &str) -> Result<(), AppError> {
        let path = self.xml_path(name);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                AppError::Storage(format!(
                    "delete {}: {e}",
                    path.display()
                ))
            })?;
        }
        Ok(())
    }
}

/// Pure XML renderer. Separated from the adapter so it can be
/// exercised directly by both `cargo test -p gnomex-infra` and the
/// integration test suite under `crates/infra/tests/`.
///
/// Produces UTF-8 text with a trailing newline. Picture paths are
/// rendered verbatim but XML-escaped — `<`, `>`, `&`, `'`, `"` would
/// otherwise break the document if a user ever has them in a path.
///
/// The picture ordering used inside the emitted XML is *not* shuffled
/// by this function — the caller owns that decision. The domain's
/// `shuffle` flag is a persisted preference; the UI / use case may
/// shuffle in place before calling if it wants randomised rotations.
pub fn render_slideshow_xml(
    slideshow: &WallpaperSlideshow,
    clock: &dyn SlideshowClock,
) -> String {
    use gnomex_domain::SlideshowMode;
    let st = clock.now();
    let mut out = String::with_capacity(256 + slideshow.pictures().len() * 128);
    out.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    out.push_str("<!-- Generated by GNOME X - do not edit by hand -->\n");
    out.push_str("<background>\n");

    match slideshow.mode() {
        SlideshowMode::Interval => {
            write_starttime(&mut out, st);
            let pictures = slideshow.pictures();
            let interval = slideshow.interval_seconds() as f64;
            let transition = slideshow.transition_seconds();
            let n = pictures.len();
            for i in 0..n {
                let cur = &pictures[i];
                let next = &pictures[(i + 1) % n];
                write_static(&mut out, interval, cur);
                write_transition(&mut out, transition, cur, next);
            }
        }
        SlideshowMode::TimeOfDay {
            sunrise_at,
            day_at,
            sunset_at,
            night_at,
        } => {
            // GNOME interprets the XML relative to <starttime> and
            // loops by wrapping the LAST <transition>'s `to` back to
            // the FIRST <static>. The file MUST end on a transition
            // (not a static) or GNOME treats the cycle as terminal
            // and freezes — matching the Fedora / stock
            // `/usr/share/backgrounds/*/<theme>.xml` pattern.
            //
            // Pin <starttime> to today at sunrise_at so at `sunrise`
            // (t=0 relative) GNOME shows the sunrise picture. The
            // cycle progresses through day → sunset → night → wraps
            // back to sunrise.
            let start = StartTime {
                year: st.year,
                month: st.month,
                day: st.day,
                hour: sunrise_at.hour,
                minute: sunrise_at.minute,
                second: 0,
            };
            write_starttime(&mut out, start);

            let pics = slideshow.pictures();
            let transition = slideshow.transition_seconds();
            let t_sr = sunrise_at.seconds_since_midnight() as f64;
            let t_d = day_at.seconds_since_midnight() as f64;
            let t_ss = sunset_at.seconds_since_midnight() as f64;
            let t_n = night_at.seconds_since_midnight() as f64;

            // Static durations are the held-picture times. Each one
            // runs from its own start moment to the next boundary
            // minus the cross-fade transition that follows it.
            let sunrise_dur = (t_d - t_sr - transition).max(1.0);
            let day_dur = (t_ss - t_d - transition).max(1.0);
            let sunset_dur = (t_n - t_ss - transition).max(1.0);
            // Night spans from night_at through midnight back to
            // sunrise_at of the NEXT day. Subtract the wrap transition.
            let night_dur = (86_400.0 - (t_n - t_sr) - transition).max(1.0);

            // 4 statics + 4 transitions in alternating order. The
            // final transition wraps from night → sunrise so GNOME
            // knows to loop back to the first static.
            write_static(&mut out, sunrise_dur, &pics[0]);
            write_transition(&mut out, transition, &pics[0], &pics[1]);
            write_static(&mut out, day_dur, &pics[1]);
            write_transition(&mut out, transition, &pics[1], &pics[2]);
            write_static(&mut out, sunset_dur, &pics[2]);
            write_transition(&mut out, transition, &pics[2], &pics[3]);
            write_static(&mut out, night_dur, &pics[3]);
            write_transition(&mut out, transition, &pics[3], &pics[0]);
        }
    }

    out.push_str("</background>\n");
    out
}

fn write_starttime(out: &mut String, st: StartTime) {
    out.push_str("  <starttime>\n");
    out.push_str(&format!("    <year>{}</year>\n", st.year));
    out.push_str(&format!("    <month>{}</month>\n", st.month));
    out.push_str(&format!("    <day>{}</day>\n", st.day));
    out.push_str(&format!("    <hour>{}</hour>\n", st.hour));
    out.push_str(&format!("    <minute>{}</minute>\n", st.minute));
    out.push_str(&format!("    <second>{}</second>\n", st.second));
    out.push_str("  </starttime>\n");
}

fn write_static(out: &mut String, duration_seconds: f64, file: &Path) {
    out.push_str("  <static>\n");
    out.push_str(&format!(
        "    <duration>{}</duration>\n",
        format_seconds(duration_seconds)
    ));
    out.push_str("    <file>");
    out.push_str(&xml_escape(&file.to_string_lossy()));
    out.push_str("</file>\n");
    out.push_str("  </static>\n");
}

fn write_transition(
    out: &mut String,
    duration_seconds: f64,
    from: &Path,
    to: &Path,
) {
    out.push_str("  <transition type=\"overlay\">\n");
    out.push_str(&format!(
        "    <duration>{}</duration>\n",
        format_seconds(duration_seconds)
    ));
    out.push_str("    <from>");
    out.push_str(&xml_escape(&from.to_string_lossy()));
    out.push_str("</from>\n");
    out.push_str("    <to>");
    out.push_str(&xml_escape(&to.to_string_lossy()));
    out.push_str("</to>\n");
    out.push_str("  </transition>\n");
}

/// GNOME expects durations formatted with a decimal point. Integers
/// render as e.g. `600.0` to make the `double` nature explicit and
/// match the style of stock `/usr/share/gnome-background-properties/*.xml`.
fn format_seconds(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{v:.1}")
    } else {
        // Trim to at most 3 decimal places — spec allows floats but
        // GNOME rounds to milliseconds internally anyway.
        let s = format!("{v:.3}");
        // Strip trailing zeros past the decimal point.
        let trimmed = s.trim_end_matches('0').trim_end_matches('.').to_owned();
        if trimmed.contains('.') {
            trimmed
        } else {
            format!("{trimmed}.0")
        }
    }
}

fn xml_escape(s: &str) -> String {
    // Only five characters are strictly reserved in XML character
    // data. Keep the helper local so we don't pull in quick-xml just
    // for this.
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&apos;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn slideshow() -> WallpaperSlideshow {
        WallpaperSlideshow::new(
            "dusk",
            vec![
                PathBuf::from("/srv/pics/a.jpg"),
                PathBuf::from("/srv/pics/b.jpg"),
            ],
            600,
            false,
        )
        .unwrap()
    }

    #[test]
    fn render_contains_xml_prolog_and_root_element() {
        let xml = render_slideshow_xml(
            &slideshow(),
            &FixedClock(StartTime::EPOCH),
        );
        assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
        assert!(xml.contains("<background>"));
        assert!(xml.trim_end().ends_with("</background>"));
    }

    #[test]
    fn render_emits_one_static_and_one_transition_per_picture() {
        let xml = render_slideshow_xml(
            &slideshow(),
            &FixedClock(StartTime::EPOCH),
        );
        assert_eq!(xml.matches("<static>").count(), 2);
        assert_eq!(xml.matches("<transition").count(), 2);
    }

    #[test]
    fn render_wraps_last_transition_back_to_first_picture() {
        let xml = render_slideshow_xml(
            &slideshow(),
            &FixedClock(StartTime::EPOCH),
        );
        // The last <transition> should have from=b.jpg, to=a.jpg —
        // i.e. we loop.
        let last_idx = xml.rfind("<transition").unwrap();
        let tail = &xml[last_idx..];
        assert!(tail.contains("<from>/srv/pics/b.jpg</from>"));
        assert!(tail.contains("<to>/srv/pics/a.jpg</to>"));
    }

    #[test]
    fn format_seconds_has_decimal_point_even_for_integers() {
        assert_eq!(format_seconds(600.0), "600.0");
        assert_eq!(format_seconds(1.0), "1.0");
        assert_eq!(format_seconds(1.5), "1.5");
        assert_eq!(format_seconds(0.25), "0.25");
    }

    #[test]
    fn xml_escape_handles_reserved_characters() {
        assert_eq!(
            xml_escape("a<b>&c'\"d"),
            "a&lt;b&gt;&amp;c&apos;&quot;d"
        );
    }

    #[test]
    fn starttime_uses_injected_clock() {
        let custom = StartTime {
            year: 2030,
            month: 7,
            day: 4,
            hour: 12,
            minute: 34,
            second: 56,
        };
        let xml = render_slideshow_xml(&slideshow(), &FixedClock(custom));
        assert!(xml.contains("<year>2030</year>"));
        assert!(xml.contains("<month>7</month>"));
        assert!(xml.contains("<second>56</second>"));
    }

    fn tod_slideshow() -> WallpaperSlideshow {
        use gnomex_domain::TimeOfDay;
        WallpaperSlideshow::time_of_day(
            "tod",
            (PathBuf::from("/p/sunrise.jpg"), TimeOfDay::new(6, 0)),
            (PathBuf::from("/p/day.jpg"), TimeOfDay::new(9, 0)),
            (PathBuf::from("/p/sunset.jpg"), TimeOfDay::new(18, 0)),
            (PathBuf::from("/p/night.jpg"), TimeOfDay::new(21, 0)),
        )
        .unwrap()
    }

    #[test]
    fn time_of_day_xml_pins_starttime_to_sunrise_at() {
        // tod_slideshow has sunrise_at = 06:00; <starttime> must be
        // today at 06:00 so GNOME's cycle phase lines up with the
        // user's declared moments.
        let xml = render_slideshow_xml(
            &tod_slideshow(),
            &FixedClock(StartTime {
                year: 2030,
                month: 3,
                day: 15,
                hour: 14,
                minute: 22,
                second: 0,
            }),
        );
        assert!(xml.contains("<year>2030</year>"));
        assert!(xml.contains("<month>3</month>"));
        assert!(xml.contains("<day>15</day>"));
        assert!(xml.contains("<hour>6</hour>"));
        assert!(xml.contains("<minute>0</minute>"));
        assert!(xml.contains("<second>0</second>"));
    }

    #[test]
    fn time_of_day_xml_emits_four_slot_cycle_in_correct_order() {
        let xml = render_slideshow_xml(
            &tod_slideshow(),
            &FixedClock(StartTime::EPOCH),
        );
        // Ordered: sunrise → day → sunset → night, ending with a
        // wrap transition back to sunrise.
        let sunrise = xml.find("/p/sunrise.jpg").unwrap();
        let day = xml.find("/p/day.jpg").unwrap();
        let sunset = xml.find("/p/sunset.jpg").unwrap();
        let night = xml.find("/p/night.jpg").unwrap();
        assert!(sunrise < day);
        assert!(day < sunset);
        assert!(sunset < night);

        // The XML must END with a <transition>, not a <static> —
        // otherwise GNOME's slideshow engine treats the cycle as
        // terminal and freezes on the last static.
        let trimmed = xml.trim_end().trim_end_matches("</background>").trim_end();
        assert!(
            trimmed.ends_with("</transition>"),
            "XML must end on a <transition> so GNOME loops — got tail: {:?}",
            &trimmed[trimmed.len().saturating_sub(60)..],
        );
    }

    #[test]
    fn time_of_day_xml_wraps_from_night_to_sunrise() {
        let xml = render_slideshow_xml(
            &tod_slideshow(),
            &FixedClock(StartTime::EPOCH),
        );
        // Last <transition> block must go night → sunrise so the
        // cycle wraps on loop. Scrape from the last <transition> tag
        // to end and look for both filenames.
        let last_trans = xml.rfind("<transition").unwrap();
        let tail = &xml[last_trans..];
        assert!(tail.contains("<from>/p/night.jpg</from>"));
        assert!(tail.contains("<to>/p/sunrise.jpg</to>"));
    }

    #[test]
    fn time_of_day_xml_durations_sum_to_a_full_day() {
        use std::cell::RefCell;
        // Scrape every <duration>N.M</duration> numeric value and
        // require the sum to equal 86400 ± 0.5 s (floating point).
        let xml = render_slideshow_xml(
            &tod_slideshow(),
            &FixedClock(StartTime::EPOCH),
        );
        let total = RefCell::new(0.0f64);
        let mut rest = xml.as_str();
        while let Some(i) = rest.find("<duration>") {
            let after = &rest[i + "<duration>".len()..];
            let j = after.find("</duration>").unwrap();
            let value: f64 = after[..j].trim().parse().unwrap();
            *total.borrow_mut() += value;
            rest = &after[j + "</duration>".len()..];
        }
        let got = *total.borrow();
        assert!(
            (got - 86_400.0).abs() < 1.0,
            "durations summed to {got}, expected ~86400",
        );
    }
}
