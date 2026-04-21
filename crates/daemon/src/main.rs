// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! experienced — GNOME X background daemon.
//!
//! Yellow L2 process entry point. Runs headless (no GTK), reusing the same
//! use cases the GUI does. Responsibilities:
//!
//! 1. **Scheduled accent** — precise timer firing at sunrise/sunset or fixed
//!    schedule. Replaces the bash-based systemd oneshot that polled every 5 min.
//! 2. **Wallpaper reactivity** — on `picture-uri` change, re-extract dominant
//!    color and update accent (Material You auto-mode).
//! 3. **Color-scheme reactivity** — on dark/light flip, re-apply external-app
//!    theming so Chromium and VS Code-family editors follow along.
//! 4. **External app theming** — propagate accent + panel-tint + color-scheme
//!    to Chromium-family browsers and VS Code-family editors via the
//!    `ExternalAppThemer` port.
//!
//! Architecture: pure composition of existing domain+app+infra crates.

use anyhow::{Context, Result};
use gio::prelude::*;
use gnomex_app::ports::ExternalAppThemer;
use gnomex_domain::{color, ColorScheme, ExternalThemeSpec, HexColor};
use gnomex_infra::wallpaper_palette::{
    encode_palette, extract_palette, locked_accent, resolve_wallpaper_image, PaletteEntry,
};
use gnomex_infra::{ChromiumThemer, VscodeThemer};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

const APP_SCHEMA: &str = "io.github.gnomex.GnomeX";
const IFACE_SCHEMA: &str = "org.gnome.desktop.interface";
const BG_SCHEMA: &str = "org.gnome.desktop.background";
const NL_SCHEMA: &str = "org.gnome.settings-daemon.plugins.color";

const ACCENT_COLORS: &[(&str, &str)] = &[
    ("blue", "#3584e4"),
    ("teal", "#2190a4"),
    ("green", "#3a944a"),
    ("yellow", "#c88800"),
    ("orange", "#ed5b00"),
    ("red", "#e62d42"),
    ("pink", "#d56199"),
    ("purple", "#9141ac"),
    ("slate", "#6f8396"),
];

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("experienced=info")),
        )
        .init();

    tracing::info!("starting experienced — GNOME X daemon");

    // Tokio runtime for any async work infra adapters may need in the future.
    // Today the daemon's operations are all synchronous GSettings reads/writes.
    let _runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("tokio runtime")?;

    let app_settings = Rc::new(gio::Settings::new(APP_SCHEMA));

    // External-app themers (Chromium browsers, VS Code-family editors).
    // Constructed once; passed by Rc into signal handlers.
    let external_themers: Rc<Vec<Arc<dyn ExternalAppThemer>>> = Rc::new(vec![
        Arc::new(VscodeThemer::new()),
        Arc::new(ChromiumThemer::new()),
    ]);

    // Apply the correct accent for the current time on startup.
    apply_scheduled_accent(&app_settings);
    // Propagate the current appearance to external apps on startup.
    apply_external_theme(&external_themers);

    // === GSettings watchers ===
    setup_accent_schedule_watcher(app_settings.clone(), external_themers.clone());
    setup_wallpaper_watcher();
    setup_color_scheme_watcher(external_themers.clone());
    setup_external_theme_watcher(app_settings.clone(), external_themers.clone());

    // === Periodic tick (1 min) ===
    // A fallback ticker in case the daemon misses a schedule-change signal.
    // Cheap — only fires work if scheduled accent is enabled.
    {
        let app_settings = app_settings.clone();
        glib::timeout_add_seconds_local(60, move || {
            apply_scheduled_accent(&app_settings);
            glib::ControlFlow::Continue
        });
    }

    // Run the glib main loop forever.
    let main_loop = glib::MainLoop::new(None, false);
    tracing::info!("daemon ready — entering main loop");
    main_loop.run();
    Ok(())
}

/// Watch the Night Light schedule and our own scheduled-accent toggle.
/// When either changes, immediately re-evaluate the accent.
fn setup_accent_schedule_watcher(
    app_settings: Rc<gio::Settings>,
    themers: Rc<Vec<Arc<dyn ExternalAppThemer>>>,
) {
    // Our own key
    {
        let app_settings = app_settings.clone();
        let themers = themers.clone();
        app_settings.clone().connect_changed(Some("scheduled-accent-enabled"), move |_, _| {
            tracing::debug!("scheduled-accent-enabled changed");
            apply_scheduled_accent(&app_settings);
            apply_external_theme(&themers);
        });
    }
    for key in ["day-accent-color", "night-accent-color"] {
        let app_settings = app_settings.clone();
        let themers = themers.clone();
        app_settings.clone().connect_changed(Some(key), move |_, _| {
            tracing::debug!("{key} changed");
            apply_scheduled_accent(&app_settings);
            apply_external_theme(&themers);
        });
    }

    // Night Light schedule keys (may be auto-updated at sunrise/sunset by gnome-settings-daemon)
    let nl_settings = Rc::new(gio::Settings::new(NL_SCHEMA));
    for key in [
        "night-light-schedule-from",
        "night-light-schedule-to",
        "night-light-schedule-automatic",
    ] {
        let app_settings = app_settings.clone();
        let themers = themers.clone();
        let nl_settings_inner = nl_settings.clone();
        nl_settings.clone().connect_changed(Some(key), move |_, _| {
            tracing::debug!("night-light key '{key}' changed");
            apply_scheduled_accent(&app_settings);
            apply_external_theme(&themers);
            let _ = nl_settings_inner; // keep reference alive
        });
    }
}

/// Watch wallpaper changes. On every change we:
///   1. resolve the current image (handling slideshow `.xml`);
///   2. extract the top-N palette (always) and cache it to
///      `cached-wallpaper-palette` so the UI can paint swatches
///      immediately without re-running extraction;
///   3. if `use-wallpaper-accent` is enabled, write the accent —
///      using `wallpaper-accent-index` to pick the locked swatch when
///      the user previously clicked one, otherwise the dominant.
///
/// We also install a slideshow-interval tick: when `picture-uri`
/// points to a GNOME slideshow `.xml`, GNOME cycles images internally
/// *without* changing the GSetting, so `connect_changed` never fires.
/// The tick re-resolves the current image on the slideshow's own
/// cadence and reruns steps 2 and 3.
fn setup_wallpaper_watcher() {
    let bg_settings = Rc::new(gio::Settings::new(BG_SCHEMA));
    let app_settings = Rc::new(gio::Settings::new(APP_SCHEMA));

    // Track the current slideshow tick so we can replace it when the
    // wallpaper (or interval) changes.
    let slideshow_tick: Rc<RefCell<Option<glib::SourceId>>> =
        Rc::new(RefCell::new(None));

    // One code-path for "wallpaper is different now — re-extract and
    // maybe re-apply". Called from both the `picture-uri` watcher and
    // the slideshow tick.
    let reapply = {
        let bg = bg_settings.clone();
        let app = app_settings.clone();
        Rc::new(move |reason: &str| {
            let uri = bg.string("picture-uri").to_string();
            let Some(image) = resolve_image(&uri) else {
                tracing::warn!("could not resolve wallpaper image for {uri}");
                return;
            };
            tracing::debug!("wallpaper re-extract ({reason}): {}", image.display());

            let palette = match extract_palette(&image) {
                Ok(p) if !p.is_empty() => p,
                Ok(_) => {
                    tracing::warn!("no dominant colours in {}", image.display());
                    Vec::new()
                }
                Err(e) => {
                    tracing::warn!("extraction failed for {}: {e}", image.display());
                    return;
                }
            };

            if !palette.is_empty() {
                let encoded = encode_palette(&palette);
                let refs: Vec<&str> = encoded.iter().map(String::as_str).collect();
                let _ = app.set_strv("cached-wallpaper-palette", refs);
                tracing::info!(
                    "cached wallpaper palette: {} colours from {}",
                    palette.len(),
                    image.display(),
                );
            }

            let wants_reactive = app
                .settings_schema()
                .map(|s| s.has_key("use-wallpaper-accent"))
                .unwrap_or(false)
                && app.boolean("use-wallpaper-accent");
            if wants_reactive {
                apply_accent_from_palette(&app, &palette);
            }
        })
    };

    // --- picture-uri change (user picked a new wallpaper or slideshow XML) ---
    {
        let bg = bg_settings.clone();
        let app = app_settings.clone();
        let slideshow_tick = slideshow_tick.clone();
        let reapply = reapply.clone();
        bg_settings
            .clone()
            .connect_changed(Some("picture-uri"), move |_, _| {
                let uri = bg.string("picture-uri").to_string();
                tracing::info!("wallpaper changed: {uri}");
                reapply("picture-uri changed");
                reinstall_slideshow_tick(&uri, &app, &slideshow_tick, reapply.clone());
            });
    }

    // --- wallpaper-accent-index change: re-evaluate immediately so
    //     the user's click takes effect without having to also touch
    //     the wallpaper. ---
    {
        let reapply = reapply.clone();
        app_settings
            .clone()
            .connect_changed(Some("wallpaper-accent-index"), move |_, _| {
                reapply("wallpaper-accent-index changed");
            });
    }

    // --- slideshow-interval change: rebuild the tick so cycles line
    //     up with the fresh interval. ---
    {
        let bg = bg_settings.clone();
        let app = app_settings.clone();
        let slideshow_tick = slideshow_tick.clone();
        let reapply = reapply.clone();
        app_settings
            .clone()
            .connect_changed(Some("wallpaper-slideshow-interval"), move |_, _| {
                let uri = bg.string("picture-uri").to_string();
                reinstall_slideshow_tick(&uri, &app, &slideshow_tick, reapply.clone());
            });
    }

    // --- On startup, prime the palette + tick using the current URI. ---
    {
        let uri = bg_settings.string("picture-uri").to_string();
        reapply("startup");
        reinstall_slideshow_tick(
            &uri,
            &app_settings,
            &slideshow_tick,
            reapply.clone(),
        );
    }
}

/// Resolve the current wallpaper image, whether `raw` is a plain image
/// URI or a slideshow `.xml`.
fn resolve_image(raw: &str) -> Option<std::path::PathBuf> {
    match resolve_wallpaper_image(raw) {
        Ok(Some(p)) => Some(p),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("wallpaper resolve failed for {raw}: {e}");
            None
        }
    }
}

/// Write the accent color from `palette` to GNOME's `accent-color`,
/// respecting the `wallpaper-accent-index` lock if it's set to a valid
/// index. Silently no-ops on an empty palette.
fn apply_accent_from_palette(app: &gio::Settings, palette: &[PaletteEntry]) {
    let has_idx_key = app
        .settings_schema()
        .map(|s| s.has_key("wallpaper-accent-index"))
        .unwrap_or(false);
    let idx = if has_idx_key { app.int("wallpaper-accent-index") } else { -1 };

    let Some(chosen) = locked_accent(palette, idx) else {
        return;
    };
    let iface = gio::Settings::new(IFACE_SCHEMA);
    if iface.string("accent-color").as_str() != chosen {
        let _ = iface.set_string("accent-color", chosen);
        let source = if idx >= 0 { format!("locked[{idx}]") } else { "dominant".into() };
        tracing::info!("auto-accent from wallpaper ({source}) → {chosen}");
    }
}

/// Remove any previously-installed slideshow tick, and — if the
/// wallpaper URI points to a `.xml` — install a new one whose period
/// is the user's `wallpaper-slideshow-interval` (floored to 5 s so a
/// zero default can't wedge the daemon in a busy loop).
fn reinstall_slideshow_tick(
    uri: &str,
    app: &gio::Settings,
    slot: &Rc<RefCell<Option<glib::SourceId>>>,
    reapply: Rc<dyn Fn(&str)>,
) {
    if let Some(old) = slot.borrow_mut().take() {
        old.remove();
    }

    let is_slideshow = uri.strip_prefix("file://").unwrap_or(uri).ends_with(".xml");
    if !is_slideshow {
        return;
    }

    // Clamp to a safe minimum. GNOME's shortest stock slideshow is 1 s
    // but we don't need to re-extract colours at that cadence; we re-
    // sample at the configured hold time, which is the human-meaningful
    // unit users actually see on screen.
    let interval = app.uint("wallpaper-slideshow-interval").max(30);
    tracing::info!("slideshow tick installed: every {interval}s");

    let id = glib::timeout_add_seconds_local(interval, move || {
        reapply("slideshow tick");
        glib::ControlFlow::Continue
    });
    *slot.borrow_mut() = Some(id);
}

/// Watch dark/light color scheme changes and re-apply external-app theming
/// so Chromium/VS Code follow the mode switch.
fn setup_color_scheme_watcher(themers: Rc<Vec<Arc<dyn ExternalAppThemer>>>) {
    let iface = Rc::new(gio::Settings::new(IFACE_SCHEMA));
    iface.clone().connect_changed(Some("color-scheme"), move |s, _| {
        let scheme = s.string("color-scheme");
        tracing::info!("color-scheme changed to: {scheme}");
        apply_external_theme(&themers);
    });
}

/// Watch the GNOME accent color and our panel tint, re-propagating to
/// external apps whenever either changes. This catches both manual accent
/// picks in Settings and our own scheduled/wallpaper-driven accent writes.
fn setup_external_theme_watcher(
    app_settings: Rc<gio::Settings>,
    themers: Rc<Vec<Arc<dyn ExternalAppThemer>>>,
) {
    // GNOME accent-color (enum).
    {
        let iface = Rc::new(gio::Settings::new(IFACE_SCHEMA));
        let themers = themers.clone();
        iface.clone().connect_changed(Some("accent-color"), move |_, _| {
            tracing::debug!("accent-color changed — re-applying external themes");
            apply_external_theme(&themers);
        });
    }
    // Our panel tint hex.
    {
        let themers = themers.clone();
        app_settings.clone().connect_changed(Some("tb-panel-tint"), move |_, _| {
            tracing::debug!("tb-panel-tint changed — re-applying external themes");
            apply_external_theme(&themers);
        });
    }
}

/// Read the current accent + panel tint + color-scheme from GSettings and
/// fan out to every external-app themer. No-op on bad data (invalid hex,
/// unknown accent id).
fn apply_external_theme(themers: &[Arc<dyn ExternalAppThemer>]) {
    let iface = gio::Settings::new(IFACE_SCHEMA);
    let app = gio::Settings::new(APP_SCHEMA);

    let accent_name = iface.string("accent-color").to_string();
    let accent_hex = ACCENT_COLORS
        .iter()
        .find(|(id, _)| *id == accent_name)
        .map(|(_, hex)| *hex)
        .unwrap_or("#3584e4");

    let panel_tint = app.string("tb-panel-tint").to_string();
    let color_scheme = ColorScheme::from_gsettings(&iface.string("color-scheme"));

    let Ok(accent) = HexColor::new(accent_hex) else {
        tracing::warn!("external theming: invalid accent hex {accent_hex}");
        return;
    };
    let Ok(panel) = HexColor::new(&panel_tint) else {
        tracing::warn!("external theming: invalid panel tint {panel_tint}");
        return;
    };

    let spec = ExternalThemeSpec {
        accent,
        panel_tint: panel,
        color_scheme,
    };

    for t in themers {
        if let Err(e) = t.apply(&spec) {
            tracing::warn!("external themer '{}' failed: {e}", t.name());
        }
    }
}

/// Core scheduled-accent logic. Reads the Night Light schedule times and
/// sets the appropriate accent color based on current time.
/// Safe to call anytime — exits early if the feature is disabled.
fn apply_scheduled_accent(app_settings: &gio::Settings) {
    if !app_settings.boolean("scheduled-accent-enabled") {
        return;
    }

    let nl = gio::Settings::new(NL_SCHEMA);
    let from_h = nl.double("night-light-schedule-from");
    let to_h = nl.double("night-light-schedule-to");

    let now = {
        let dt = glib::DateTime::now_local().expect("local time");
        dt.hour() as f64 + dt.minute() as f64 / 60.0
    };

    // Night wraps past midnight (e.g. 20:00 → 6:00).
    let is_night = if from_h > to_h {
        now >= from_h || now < to_h
    } else {
        now >= from_h && now < to_h
    };

    let raw = if is_night {
        app_settings.string("night-accent-color").to_string()
    } else {
        app_settings.string("day-accent-color").to_string()
    };
    // `accent-color` is an enum — snap any custom hex to the nearest
    // named GNOME accent before writing.
    let target = nearest_accent_id(&raw);

    let iface = gio::Settings::new(IFACE_SCHEMA);
    let current = iface.string("accent-color").to_string();
    if current != target {
        let _ = iface.set_string("accent-color", &target);
        tracing::info!(
            "scheduled accent: {current} → {target} ({})",
            if is_night { "night" } else { "day" },
        );
    }
}

/// Map a value that may be either a GNOME accent id or a raw hex color to
/// the closest GNOME accent id. `accent-color` in `org.gnome.desktop.interface`
/// is an enum, so hex values would be rejected outright.
fn nearest_accent_id(value: &str) -> String {
    if ACCENT_COLORS.iter().any(|(id, _)| *id == value) {
        return value.to_owned();
    }
    let Ok(target) = HexColor::new(value).map(|c| c.to_rgb()) else {
        return "blue".to_owned();
    };
    let mut best = "blue";
    let mut best_dist = u32::MAX;
    for &(id, hex) in ACCENT_COLORS {
        let accent = HexColor::new(hex).map(|c| c.to_rgb()).unwrap_or((0, 0, 0));
        let dist = color::color_distance(target, accent);
        if dist < best_dist {
            best_dist = dist;
            best = id;
        }
    }
    best.to_owned()
}

