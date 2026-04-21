// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Wallpaper slideshow specification (GXF-072).
//!
//! Pure value type describing a user-authored wallpaper rotation.
//! GNOME's own background subsystem reads the generated `.xml` file
//! referenced by `org.gnome.desktop.background picture-uri`, so once
//! this type is materialised to disk by the infra adapter, GNOME
//! Shell handles playback — we do not run a background daemon.
//!
//! Format reference: the freedesktop/GNOME `background-properties` XML
//! schema as understood by `gnome-background-properties` and Mutter.
//! The minimal playable shape is:
//!
//! ```xml
//! <background>
//!   <starttime>
//!     <year>…</year><month>…</month><day>…</day>
//!     <hour>…</hour><minute>…</minute><second>…</second>
//!   </starttime>
//!   <static>
//!     <duration>600.0</duration>
//!     <file>/abs/path/img-a.jpg</file>
//!   </static>
//!   <transition type="overlay">
//!     <duration>1.0</duration>
//!     <from>/abs/path/img-a.jpg</from>
//!     <to>/abs/path/img-b.jpg</to>
//!   </transition>
//!   …
//! </background>
//! ```
//!
//! This module is pure: no I/O, no xml serialisation, no time source.
//! The adapter supplies a [`SlideshowClock`] when emitting XML so that
//! tests can pin `<starttime>` deterministically.

use std::path::{Path, PathBuf};

/// Default cross-fade duration between pictures, in seconds. Keeps the
/// desktop from flashing on rotation. Matches the value shipped by
/// stock GNOME background XMLs.
pub const DEFAULT_TRANSITION_SECONDS: f64 = 1.0;

/// Minimum picture hold time. Below this, GNOME's compositor spends
/// more time fading than showing the picture — keep the spinner from
/// letting the user foot-gun themselves.
pub const MIN_INTERVAL_SECONDS: u32 = 5;

/// Errors produced when constructing a [`WallpaperSlideshow`].
#[derive(Debug, PartialEq, Eq)]
pub enum WallpaperSlideshowError {
    /// Fewer than two pictures — a slideshow needs something to
    /// rotate between. A single wallpaper should use `picture-uri`
    /// directly.
    TooFewPictures,
    /// Interval under [`MIN_INTERVAL_SECONDS`].
    IntervalTooShort,
    /// A picture path was relative or otherwise non-absolute.
    RelativePath(PathBuf),
    /// Name isn't a safe filesystem stem (empty, starts with `.`,
    /// contains `/` or control chars). The adapter uses it verbatim
    /// to build `<name>.xml`.
    InvalidName(String),
    /// Time-of-day mode requires the four transition moments to be
    /// strictly increasing (sunrise < day < sunset < night).
    TimesNotIncreasing,
}

impl core::fmt::Display for WallpaperSlideshowError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::TooFewPictures => {
                write!(f, "slideshow needs at least two pictures")
            }
            Self::IntervalTooShort => write!(
                f,
                "interval must be at least {MIN_INTERVAL_SECONDS} seconds"
            ),
            Self::RelativePath(p) => {
                write!(f, "picture path must be absolute: {}", p.display())
            }
            Self::InvalidName(n) => write!(f, "invalid slideshow name: {n:?}"),
            Self::TimesNotIncreasing => {
                write!(f, "sunrise/day/sunset/night times must be strictly increasing")
            }
        }
    }
}

impl std::error::Error for WallpaperSlideshowError {}

/// A wall-clock time (hour:minute) within a 24-hour day. Used by
/// [`SlideshowMode::TimeOfDay`] to pin each slot's transition moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeOfDay {
    pub hour: u8,
    pub minute: u8,
}

impl TimeOfDay {
    /// Construct a validated `TimeOfDay`. `hour` must be 0–23 and
    /// `minute` 0–59; otherwise clamps to the nearest legal value —
    /// this lets the UI pass raw spin-row doubles without having to
    /// guard every call-site.
    pub fn new(hour: u8, minute: u8) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
        }
    }

    /// Convert to seconds-since-midnight, useful when computing
    /// slideshow durations.
    pub fn seconds_since_midnight(&self) -> u32 {
        (self.hour as u32) * 3600 + (self.minute as u32) * 60
    }
}

/// Playback mode for a slideshow.
///
/// The XML format GNOME consumes is the same in both cases — a
/// sequence of `<static>` entries with durations — but the meaning
/// differs.
///
/// - `Interval` — hold each picture for `interval_seconds`, cycle
///   forever. Order comes from `pictures()`.
/// - `TimeOfDay` — four pictures pinned to sunrise / day / sunset /
///   night wall-clock transitions. Cycle is exactly 24 hours; the
///   XML `<starttime>` is midnight UTC today so the transitions fire
///   at the user's declared local-time thresholds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlideshowMode {
    Interval,
    TimeOfDay {
        /// Local time at which the sunrise wallpaper activates. The
        /// slot holds until `day_at`.
        sunrise_at: TimeOfDay,
        /// Transition into the "day" wallpaper.
        day_at: TimeOfDay,
        /// Transition into the "sunset" wallpaper.
        sunset_at: TimeOfDay,
        /// Transition into the "night" wallpaper; holds until
        /// sunrise-at rolls around the next morning.
        night_at: TimeOfDay,
    },
}

/// User-authored wallpaper rotation. Immutable after construction —
/// mutate a fresh instance via [`WallpaperSlideshow::new`] to change
/// fields.
#[derive(Debug, Clone, PartialEq)]
pub struct WallpaperSlideshow {
    /// Filesystem-safe stem. The adapter writes `<name>.xml`.
    name: String,
    /// Absolute paths to pictures, in display order. For
    /// [`SlideshowMode::TimeOfDay`] this is exactly four entries
    /// in sunrise / day / sunset / night order.
    pictures: Vec<PathBuf>,
    /// Seconds to hold each picture before fading to the next.
    /// Ignored by the XML renderer when mode is `TimeOfDay`.
    interval_seconds: u32,
    /// Cross-fade duration between adjacent pictures.
    transition_seconds: f64,
    /// When true, the adapter is expected to shuffle `pictures` in
    /// place before emitting XML. We keep the flag in the domain so
    /// the shuffle decision is persisted in pack manifests; the
    /// adapter owns the RNG. Only meaningful for `Interval` mode.
    shuffle: bool,
    /// Playback mode. Defaults to `Interval` so every existing
    /// caller (including `new`) stays source-compatible.
    mode: SlideshowMode,
}

impl WallpaperSlideshow {
    /// Construct a validated slideshow. Errors are domain-level
    /// invariant failures — the UI should surface them as toasts.
    pub fn new(
        name: impl Into<String>,
        pictures: Vec<PathBuf>,
        interval_seconds: u32,
        shuffle: bool,
    ) -> Result<Self, WallpaperSlideshowError> {
        Self::with_transition(
            name,
            pictures,
            interval_seconds,
            DEFAULT_TRANSITION_SECONDS,
            shuffle,
        )
    }

    /// Same as [`Self::new`] but with an explicit transition duration.
    pub fn with_transition(
        name: impl Into<String>,
        pictures: Vec<PathBuf>,
        interval_seconds: u32,
        transition_seconds: f64,
        shuffle: bool,
    ) -> Result<Self, WallpaperSlideshowError> {
        let name = name.into();
        validate_name(&name)?;
        if pictures.len() < 2 {
            return Err(WallpaperSlideshowError::TooFewPictures);
        }
        if interval_seconds < MIN_INTERVAL_SECONDS {
            return Err(WallpaperSlideshowError::IntervalTooShort);
        }
        for p in &pictures {
            if !p.is_absolute() {
                return Err(WallpaperSlideshowError::RelativePath(p.clone()));
            }
        }
        let transition_seconds = if transition_seconds.is_finite() && transition_seconds > 0.0 {
            transition_seconds
        } else {
            DEFAULT_TRANSITION_SECONDS
        };
        Ok(Self {
            name,
            pictures,
            interval_seconds,
            transition_seconds,
            shuffle,
            mode: SlideshowMode::Interval,
        })
    }

    /// Construct a time-of-day slideshow. The four pictures activate
    /// at the declared wall-clock moments and hold until the next.
    ///
    /// Transition times must be strictly increasing and all within a
    /// single day (00:00–23:59). Pictures must all be absolute paths.
    /// Like [`Self::new`], `transition_seconds` defaults to
    /// [`DEFAULT_TRANSITION_SECONDS`] if left unspecified via this
    /// constructor; TimeOfDay cycles use the default.
    pub fn time_of_day(
        name: impl Into<String>,
        sunrise: (PathBuf, TimeOfDay),
        day: (PathBuf, TimeOfDay),
        sunset: (PathBuf, TimeOfDay),
        night: (PathBuf, TimeOfDay),
    ) -> Result<Self, WallpaperSlideshowError> {
        let name = name.into();
        validate_name(&name)?;
        let pictures = vec![sunrise.0, day.0, sunset.0, night.0];
        for p in &pictures {
            if !p.is_absolute() {
                return Err(WallpaperSlideshowError::RelativePath(p.clone()));
            }
        }
        let slots = [sunrise.1, day.1, sunset.1, night.1];
        for w in slots.windows(2) {
            if w[1].seconds_since_midnight() <= w[0].seconds_since_midnight() {
                return Err(WallpaperSlideshowError::TimesNotIncreasing);
            }
        }
        Ok(Self {
            name,
            pictures,
            interval_seconds: 0,
            transition_seconds: DEFAULT_TRANSITION_SECONDS,
            shuffle: false,
            mode: SlideshowMode::TimeOfDay {
                sunrise_at: sunrise.1,
                day_at: day.1,
                sunset_at: sunset.1,
                night_at: night.1,
            },
        })
    }

    pub fn mode(&self) -> &SlideshowMode {
        &self.mode
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn pictures(&self) -> &[PathBuf] {
        &self.pictures
    }

    pub fn interval_seconds(&self) -> u32 {
        self.interval_seconds
    }

    pub fn transition_seconds(&self) -> f64 {
        self.transition_seconds
    }

    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    /// Total wall-clock duration of one full loop (not counting the
    /// last-to-first transition, which GNOME wraps implicitly).
    /// Useful for UI summaries ("Loop: 34 min").
    pub fn total_loop_seconds(&self) -> f64 {
        let n = self.pictures.len() as f64;
        n * self.interval_seconds as f64 + n * self.transition_seconds
    }
}

fn validate_name(name: &str) -> Result<(), WallpaperSlideshowError> {
    if name.is_empty()
        || name.starts_with('.')
        || name.contains('/')
        || name.contains('\\')
        || name.chars().any(|c| c.is_control())
    {
        return Err(WallpaperSlideshowError::InvalidName(name.to_string()));
    }
    Ok(())
}

/// Abstract time source for slideshow XML `<starttime>`. Lives in the
/// domain so the adapter stays replaceable (tests inject a fixed
/// clock; production uses wall-clock UTC). Keeping the trait here —
/// rather than in the port layer — means the adapter's public API
/// can accept any `SlideshowClock` without dragging the app crate into
/// the type signature.
pub trait SlideshowClock: Send + Sync {
    /// Emit year/month/day/hour/minute/second to use in
    /// `<starttime>`. Seconds fractional parts are dropped — GNOME's
    /// schema is integer-only.
    fn now(&self) -> StartTime;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartTime {
    pub year: i32,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl StartTime {
    /// Sentinel epoch GNOME ships in stock slideshows. Stable across
    /// test runs and avoids leaking the host clock into fixtures.
    pub const EPOCH: StartTime = StartTime {
        year: 2011,
        month: 11,
        day: 11,
        hour: 11,
        minute: 11,
        second: 11,
    };
}

/// Zero-argument clock whose `now()` is a compile-time constant.
/// Useful for both the default production wiring (where `<starttime>`
/// just needs to be *some* past moment) and for deterministic tests.
pub struct FixedClock(pub StartTime);

impl SlideshowClock for FixedClock {
    fn now(&self) -> StartTime {
        self.0
    }
}

/// Slideshow path contract — where the adapter writes the XML on
/// disk. Pure helper, no I/O. Lives here so both the adapter and the
/// UI can agree on the URI to stuff into `picture-uri`.
pub fn slideshow_xml_relative_path(name: &str) -> PathBuf {
    Path::new("gnome-background-properties").join(format!("{name}.xml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> WallpaperSlideshow {
        WallpaperSlideshow::new(
            "nightfall",
            vec![
                PathBuf::from("/usr/share/backgrounds/a.jpg"),
                PathBuf::from("/usr/share/backgrounds/b.jpg"),
                PathBuf::from("/usr/share/backgrounds/c.jpg"),
            ],
            600,
            false,
        )
        .unwrap()
    }

    #[test]
    fn valid_slideshow_round_trips_accessors() {
        let s = sample();
        assert_eq!(s.name(), "nightfall");
        assert_eq!(s.pictures().len(), 3);
        assert_eq!(s.interval_seconds(), 600);
        assert!(!s.shuffle());
        assert_eq!(s.transition_seconds(), DEFAULT_TRANSITION_SECONDS);
    }

    #[test]
    fn requires_at_least_two_pictures() {
        let err = WallpaperSlideshow::new(
            "solo",
            vec![PathBuf::from("/x/y.jpg")],
            30,
            false,
        )
        .unwrap_err();
        assert_eq!(err, WallpaperSlideshowError::TooFewPictures);
    }

    #[test]
    fn rejects_sub_minimum_interval() {
        let err = WallpaperSlideshow::new(
            "fast",
            vec![
                PathBuf::from("/a.jpg"),
                PathBuf::from("/b.jpg"),
            ],
            1,
            false,
        )
        .unwrap_err();
        assert_eq!(err, WallpaperSlideshowError::IntervalTooShort);
    }

    #[test]
    fn rejects_relative_picture_paths() {
        let err = WallpaperSlideshow::new(
            "relly",
            vec![
                PathBuf::from("relative.jpg"),
                PathBuf::from("/absolute.jpg"),
            ],
            30,
            false,
        )
        .unwrap_err();
        assert!(matches!(err, WallpaperSlideshowError::RelativePath(_)));
    }

    #[test]
    fn rejects_unsafe_names() {
        for bad in ["", ".hidden", "has/slash", "has\\bslash", "tab\there"] {
            let err = WallpaperSlideshow::new(
                bad,
                vec![PathBuf::from("/a.jpg"), PathBuf::from("/b.jpg")],
                30,
                false,
            )
            .unwrap_err();
            assert!(
                matches!(err, WallpaperSlideshowError::InvalidName(_)),
                "expected InvalidName for {bad:?}, got {err:?}"
            );
        }
    }

    #[test]
    fn negative_or_nan_transition_falls_back_to_default() {
        let s = WallpaperSlideshow::with_transition(
            "neg",
            vec![PathBuf::from("/a.jpg"), PathBuf::from("/b.jpg")],
            30,
            -5.0,
            false,
        )
        .unwrap();
        assert_eq!(s.transition_seconds(), DEFAULT_TRANSITION_SECONDS);

        let s = WallpaperSlideshow::with_transition(
            "nan",
            vec![PathBuf::from("/a.jpg"), PathBuf::from("/b.jpg")],
            30,
            f64::NAN,
            false,
        )
        .unwrap();
        assert_eq!(s.transition_seconds(), DEFAULT_TRANSITION_SECONDS);
    }

    #[test]
    fn total_loop_seconds_accounts_for_transition() {
        let s = WallpaperSlideshow::with_transition(
            "loop",
            vec![
                PathBuf::from("/a.jpg"),
                PathBuf::from("/b.jpg"),
                PathBuf::from("/c.jpg"),
            ],
            60,
            2.0,
            false,
        )
        .unwrap();
        // 3 * (60 + 2) = 186
        assert!((s.total_loop_seconds() - 186.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fixed_clock_is_deterministic() {
        let c = FixedClock(StartTime::EPOCH);
        assert_eq!(c.now(), StartTime::EPOCH);
        assert_eq!(c.now(), c.now());
    }

    #[test]
    fn time_of_day_constructor_builds_four_slots() {
        let s = WallpaperSlideshow::time_of_day(
            "daily",
            (PathBuf::from("/img/sunrise.jpg"), TimeOfDay::new(6, 0)),
            (PathBuf::from("/img/day.jpg"), TimeOfDay::new(9, 0)),
            (PathBuf::from("/img/sunset.jpg"), TimeOfDay::new(18, 0)),
            (PathBuf::from("/img/night.jpg"), TimeOfDay::new(21, 0)),
        )
        .unwrap();
        assert_eq!(s.pictures().len(), 4);
        assert!(matches!(s.mode(), SlideshowMode::TimeOfDay { .. }));
    }

    #[test]
    fn time_of_day_requires_increasing_transitions() {
        let err = WallpaperSlideshow::time_of_day(
            "bad",
            (PathBuf::from("/a.jpg"), TimeOfDay::new(9, 0)),
            // day_at earlier than sunrise_at:
            (PathBuf::from("/b.jpg"), TimeOfDay::new(6, 0)),
            (PathBuf::from("/c.jpg"), TimeOfDay::new(18, 0)),
            (PathBuf::from("/d.jpg"), TimeOfDay::new(21, 0)),
        )
        .unwrap_err();
        assert_eq!(err, WallpaperSlideshowError::TimesNotIncreasing);
    }

    #[test]
    fn time_of_day_rejects_relative_paths() {
        let err = WallpaperSlideshow::time_of_day(
            "rel",
            (PathBuf::from("relative.jpg"), TimeOfDay::new(6, 0)),
            (PathBuf::from("/d.jpg"), TimeOfDay::new(9, 0)),
            (PathBuf::from("/s.jpg"), TimeOfDay::new(18, 0)),
            (PathBuf::from("/n.jpg"), TimeOfDay::new(21, 0)),
        )
        .unwrap_err();
        assert!(matches!(err, WallpaperSlideshowError::RelativePath(_)));
    }

    #[test]
    fn time_of_day_seconds_since_midnight_math() {
        assert_eq!(TimeOfDay::new(0, 0).seconds_since_midnight(), 0);
        assert_eq!(TimeOfDay::new(6, 30).seconds_since_midnight(), 6 * 3600 + 1800);
        assert_eq!(TimeOfDay::new(23, 59).seconds_since_midnight(), 86_340);
    }

    #[test]
    fn time_of_day_out_of_range_clamps() {
        // Defensive: UI may forward raw spin-row values.
        assert_eq!(TimeOfDay::new(25, 70).hour, 23);
        assert_eq!(TimeOfDay::new(25, 70).minute, 59);
    }

    #[test]
    fn xml_path_helper_is_under_background_properties() {
        let p = slideshow_xml_relative_path("xy");
        assert_eq!(
            p,
            PathBuf::from("gnome-background-properties/xy.xml"),
        );
    }
}
