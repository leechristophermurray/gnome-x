// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! End-to-end test for the Material-palette branch of
//! `ApplyThemeUseCase`: feed a stub palette, flip the toggle, assert
//! the CSS generator receives a spec whose `widget_colors` were
//! derived from the palette permutation (not whatever
//! `widget_colors` the caller originally supplied).

use gnomex_app::ports::{
    AppearanceSettings, IconThemeRecolorer, RecolorOutcome, ThemeCss, ThemeCssGenerator,
    ThemeWriter, WallpaperPaletteProvider,
};
use gnomex_app::use_cases::ApplyThemeUseCase;
use gnomex_app::AppError;
use gnomex_domain::{
    HexColor, MaterialPaletteSpec, Permutation, ThemeSpec, WidgetColorOverrides,
};
use std::sync::{Arc, Mutex};

/// Capturing generator — clones every spec handed to `generate`
/// into a vec so the test can assert on exactly what the use case
/// routed through.
#[derive(Default)]
struct CapturingCssGen {
    captured: Mutex<Vec<ThemeSpec>>,
}

impl ThemeCssGenerator for CapturingCssGen {
    fn version_label(&self) -> &str {
        "TEST"
    }
    fn generate(&self, spec: &ThemeSpec) -> Result<ThemeCss, AppError> {
        self.captured.lock().unwrap().push(spec.clone());
        Ok(ThemeCss {
            gtk_css: "/* stub */".into(),
            gtk3_css: "/* stub */".into(),
            shell_css: "/* stub */".into(),
        })
    }
}

#[derive(Default)]
struct NoopWriter;
impl ThemeWriter for NoopWriter {
    fn write_gtk_css(&self, _: &str, _: &str) -> Result<(), AppError> {
        Ok(())
    }
    fn write_shell_css(&self, _: &str, _: &str) -> Result<(), AppError> {
        Ok(())
    }
    fn clear_overrides(&self) -> Result<(), AppError> {
        Ok(())
    }
}

#[derive(Default)]
struct NoopAppearance {
    accent_writes: Mutex<Vec<String>>,
    color_scheme: Mutex<String>,
}
impl AppearanceSettings for NoopAppearance {
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
        Ok(String::new())
    }
    fn set_wallpaper(&self, _: &str) -> Result<(), AppError> {
        Ok(())
    }
    fn get_accent_color(&self) -> Result<String, AppError> {
        Ok(String::new())
    }
    fn set_accent_color(&self, id: &str) -> Result<(), AppError> {
        self.accent_writes.lock().unwrap().push(id.to_owned());
        Ok(())
    }
    fn get_color_scheme(&self) -> Result<String, AppError> {
        Ok(self.color_scheme.lock().unwrap().clone())
    }
}

/// Captures every accent id the use case routes through the
/// recolorer so tests can assert MD3 kicked it.
#[derive(Default)]
struct SpyRecolorer {
    calls: Mutex<Vec<String>>,
}
impl IconThemeRecolorer for SpyRecolorer {
    fn recolor(&self, accent_id: &str) -> Result<RecolorOutcome, AppError> {
        self.calls.lock().unwrap().push(accent_id.to_owned());
        Ok(RecolorOutcome::NativelyTracks("Test".into()))
    }
}

struct StubPalette(Option<[HexColor; 3]>);
impl WallpaperPaletteProvider for StubPalette {
    fn top3(&self) -> Option<[HexColor; 3]> {
        self.0.clone()
    }
}

fn rgb_hex() -> [HexColor; 3] {
    [
        HexColor::new("#ff0000").unwrap(), // palette[0] — red
        HexColor::new("#00ff00").unwrap(), // palette[1] — green
        HexColor::new("#0000ff").unwrap(), // palette[2] — blue
    ]
}

fn spec_with_user_overrides() -> ThemeSpec {
    // The user had widget_color overrides set before enabling MD3;
    // after enabling MD3 those must NOT appear in the CSS generator input.
    let mut s = ThemeSpec::defaults();
    s.widget_colors = WidgetColorOverrides {
        button_bg_light: Some(HexColor::new("#aaaaaa").unwrap()),
        button_bg_dark: Some(HexColor::new("#222222").unwrap()),
        ..Default::default()
    };
    s
}

#[test]
fn md3_disabled_passes_user_widget_colors_through() {
    let generator = Arc::new(CapturingCssGen::default());
    let writer = Arc::new(NoopWriter);
    let appearance = Arc::new(NoopAppearance::default());
    let palette = Arc::new(StubPalette(Some(rgb_hex())));
    let uc = ApplyThemeUseCase::new(generator.clone(), writer, appearance)
        .with_palette_provider(palette);

    // MD3 off → derive_material_spec never fires, user's
    // widget_colors survive.
    let s = spec_with_user_overrides(); // MD3 disabled by default
    uc.apply(&s).unwrap();
    let captured = &generator.captured.lock().unwrap()[0];
    assert_eq!(captured.widget_colors.button_bg_light.as_ref().unwrap().as_str(), "#aaaaaa");
}

#[test]
fn md3_enabled_overwrites_widget_colors_from_palette() {
    let generator = Arc::new(CapturingCssGen::default());
    let writer = Arc::new(NoopWriter);
    let appearance = Arc::new(NoopAppearance::default());
    let palette = Arc::new(StubPalette(Some(rgb_hex())));
    let uc = ApplyThemeUseCase::new(generator.clone(), writer, appearance)
        .with_palette_provider(palette);

    // MD3 ON with non-zero tint (so the derivation is observable).
    // Day: Bg0Pri1Sec2 → bg=red, primary=green, secondary=blue.
    // So button_bg_light must be a blend toward green (the primary),
    // NOT the user's #aaaaaa.
    let mut s = spec_with_user_overrides();
    s.material_palette = MaterialPaletteSpec {
        enabled: true,
        day_permutation: Permutation::Bg0Pri1Sec2,
        night_permutation: Permutation::Bg0Pri1Sec2,
    };
    s.tint.intensity = gnomex_domain::Opacity::from_fraction(1.0).unwrap();
    uc.apply(&s).unwrap();

    let captured = &generator.captured.lock().unwrap()[0];
    // The user's #aaaaaa must be gone — MD3 overrode it.
    let btn_light = captured.widget_colors.button_bg_light.as_ref().unwrap();
    assert_ne!(btn_light.as_str(), "#aaaaaa", "user override leaked past MD3 derivation");
    // Primary at full intensity × 0.8 blend toward green → the green
    // channel must dominate red + blue.
    let (r, g, b) = btn_light.to_rgb();
    assert!(g > r, "green should dominate red, got r={r} g={g} b={b}");
    assert!(g > b, "green should dominate blue, got r={r} g={g} b={b}");
}

#[test]
fn md3_enabled_but_palette_empty_falls_back_to_user_overrides() {
    let generator = Arc::new(CapturingCssGen::default());
    let writer = Arc::new(NoopWriter);
    let appearance = Arc::new(NoopAppearance::default());
    // Palette provider returns None — simulates first-run before
    // the daemon has extracted anything.
    let palette = Arc::new(StubPalette(None));
    let uc = ApplyThemeUseCase::new(generator.clone(), writer, appearance)
        .with_palette_provider(palette);

    let mut s = spec_with_user_overrides();
    s.material_palette.enabled = true;
    uc.apply(&s).unwrap();

    // No palette → MD3 can't derive → we pass the user's
    // widget_colors through unchanged so theme apply still works.
    let captured = &generator.captured.lock().unwrap()[0];
    assert_eq!(
        captured.widget_colors.button_bg_light.as_ref().unwrap().as_str(),
        "#aaaaaa",
        "MD3 fallback must preserve user overrides when palette is empty",
    );
}

#[test]
fn md3_enabled_with_no_provider_is_noop() {
    // The use case was constructed WITHOUT `with_palette_provider`.
    // MD3 mode silently degrades to the user's static overrides.
    let generator = Arc::new(CapturingCssGen::default());
    let writer = Arc::new(NoopWriter);
    let appearance = Arc::new(NoopAppearance::default());
    let uc = ApplyThemeUseCase::new(generator.clone(), writer, appearance);

    let mut s = spec_with_user_overrides();
    s.material_palette.enabled = true;
    uc.apply(&s).unwrap();

    let captured = &generator.captured.lock().unwrap()[0];
    assert_eq!(
        captured.widget_colors.button_bg_light.as_ref().unwrap().as_str(),
        "#aaaaaa",
    );
}

#[test]
fn md3_applies_day_and_night_permutations_independently() {
    let generator = Arc::new(CapturingCssGen::default());
    let writer = Arc::new(NoopWriter);
    let appearance = Arc::new(NoopAppearance::default());
    let palette = Arc::new(StubPalette(Some(rgb_hex())));
    let uc = ApplyThemeUseCase::new(generator.clone(), writer, appearance)
        .with_palette_provider(palette);

    // Day pri=green, night pri=blue — the derived overrides must
    // differ between light/dark widgets.
    let mut s = spec_with_user_overrides();
    s.material_palette = MaterialPaletteSpec {
        enabled: true,
        day_permutation: Permutation::Bg0Pri1Sec2, // pri=palette[1]=green
        night_permutation: Permutation::Bg0Pri2Sec1, // pri=palette[2]=blue
    };
    s.tint.intensity = gnomex_domain::Opacity::from_fraction(1.0).unwrap();
    uc.apply(&s).unwrap();

    let captured = &generator.captured.lock().unwrap()[0];
    let (_, lg, _) = captured.widget_colors.button_bg_light.as_ref().unwrap().to_rgb();
    let (_, _, db) = captured.widget_colors.button_bg_dark.as_ref().unwrap().to_rgb();
    assert!(lg > 50, "light button should lean green, got g={lg}");
    assert!(db > 50, "dark button should lean blue, got b={db}");
}

#[test]
fn md3_writes_nearest_gnome_accent_to_settings_and_kicks_recolorer() {
    // The complaint from real testing: MD3 paints widget CSS but
    // folder icons don't follow because `accent-color` never
    // changes. The use case must propagate the MD3 primary to
    // both AppearanceSettings.set_accent_color AND the icon
    // recolorer, so Adwaita + Papirus both see fresh input.
    let generator = Arc::new(CapturingCssGen::default());
    let writer = Arc::new(NoopWriter);
    let appearance = Arc::new(NoopAppearance::default());
    let palette = Arc::new(StubPalette(Some(rgb_hex())));
    let recolorer = Arc::new(SpyRecolorer::default());
    let uc = ApplyThemeUseCase::new(generator.clone(), writer, appearance.clone())
        .with_palette_provider(palette)
        .with_icon_recolorer(recolorer.clone());

    let mut s = spec_with_user_overrides();
    s.material_palette = MaterialPaletteSpec {
        enabled: true,
        // Day primary = palette[1] = #00ff00 → nearest GNOME accent is
        // "green".
        day_permutation: Permutation::Bg0Pri1Sec2,
        night_permutation: Permutation::Bg0Pri1Sec2,
    };
    s.tint.intensity = gnomex_domain::Opacity::from_fraction(1.0).unwrap();
    uc.apply(&s).unwrap();

    let writes = appearance.accent_writes.lock().unwrap();
    assert_eq!(*writes, vec!["green".to_string()]);

    let calls = recolorer.calls.lock().unwrap();
    assert_eq!(*calls, vec!["green".to_string()]);
}

#[test]
fn md3_propagates_night_pri_ignored_because_accent_enum_is_scheme_agnostic() {
    // The accent enum is one id for the whole system; we pick day
    // pri so accent stays stable across the scheme flip. A night-only
    // permutation change should NOT retrigger an accent-color write.
    let generator = Arc::new(CapturingCssGen::default());
    let writer = Arc::new(NoopWriter);
    let appearance = Arc::new(NoopAppearance::default());
    let palette = Arc::new(StubPalette(Some(rgb_hex())));
    let uc = ApplyThemeUseCase::new(generator.clone(), writer, appearance.clone())
        .with_palette_provider(palette);

    let mut s = spec_with_user_overrides();
    s.material_palette = MaterialPaletteSpec {
        enabled: true,
        day_permutation: Permutation::Bg0Pri1Sec2,   // day pri = green
        night_permutation: Permutation::Bg0Pri2Sec1, // night pri = blue
    };
    s.tint.intensity = gnomex_domain::Opacity::from_fraction(1.0).unwrap();
    uc.apply(&s).unwrap();

    let writes = appearance.accent_writes.lock().unwrap();
    assert_eq!(*writes, vec!["green".to_string()], "day pri should drive accent");
}

#[test]
fn md3_sets_shell_tint_override_to_background_role() {
    // The user's panel / calendar popup / OSD should pick up the
    // MD3 muted background, not the stock accent. We assert that
    // the CSS generator receives a spec whose `shell_tint_override`
    // is populated with the wallpaper-palette background colour
    // appropriate to the active colour scheme.
    let generator = Arc::new(CapturingCssGen::default());
    let writer = Arc::new(NoopWriter);
    let appearance = Arc::new(NoopAppearance::default());
    *appearance.color_scheme.lock().unwrap() = "prefer-dark".into();
    let palette = Arc::new(StubPalette(Some(rgb_hex())));
    let uc = ApplyThemeUseCase::new(generator.clone(), writer, appearance)
        .with_palette_provider(palette);

    // Night permutation bg = palette[0] = #ff0000. Dark scheme, so
    // MD3 should pick night.background = red.
    let mut s = spec_with_user_overrides();
    s.material_palette = MaterialPaletteSpec {
        enabled: true,
        day_permutation: Permutation::Bg2Pri0Sec1,   // day bg = palette[2] = blue
        night_permutation: Permutation::Bg0Pri1Sec2, // night bg = palette[0] = red
    };
    s.tint.intensity = gnomex_domain::Opacity::from_fraction(0.5).unwrap();
    uc.apply(&s).unwrap();

    let captured = &generator.captured.lock().unwrap()[0];
    let tint = captured
        .shell_tint_override
        .as_ref()
        .expect("MD3 must populate shell_tint_override");
    // Dark scheme should use the NIGHT role background, which under
    // Bg0Pri1Sec2 is palette[0] = #ff0000.
    assert_eq!(tint.as_str(), "#ff0000");
}

#[test]
fn md3_shell_tint_follows_light_scheme_to_day_background() {
    let generator = Arc::new(CapturingCssGen::default());
    let writer = Arc::new(NoopWriter);
    let appearance = Arc::new(NoopAppearance::default());
    *appearance.color_scheme.lock().unwrap() = "prefer-light".into();
    let palette = Arc::new(StubPalette(Some(rgb_hex())));
    let uc = ApplyThemeUseCase::new(generator.clone(), writer, appearance)
        .with_palette_provider(palette);

    let mut s = spec_with_user_overrides();
    s.material_palette = MaterialPaletteSpec {
        enabled: true,
        day_permutation: Permutation::Bg2Pri0Sec1,   // day bg = palette[2] = blue
        night_permutation: Permutation::Bg0Pri1Sec2, // night bg = palette[0] = red
    };
    s.tint.intensity = gnomex_domain::Opacity::from_fraction(0.5).unwrap();
    uc.apply(&s).unwrap();

    let captured = &generator.captured.lock().unwrap()[0];
    let tint = captured.shell_tint_override.as_ref().unwrap();
    // Light scheme → day.background = palette[2] = #0000ff.
    assert_eq!(tint.as_str(), "#0000ff");
}

#[test]
fn md3_disabled_leaves_shell_tint_override_none() {
    // Non-MD3 Apply must not populate the override — the classic
    // accent-tinted shell CSS path should run unchanged.
    let generator = Arc::new(CapturingCssGen::default());
    let writer = Arc::new(NoopWriter);
    let appearance = Arc::new(NoopAppearance::default());
    let palette = Arc::new(StubPalette(Some(rgb_hex())));
    let uc = ApplyThemeUseCase::new(generator.clone(), writer, appearance)
        .with_palette_provider(palette);

    // MD3 off (default).
    let s = spec_with_user_overrides();
    uc.apply(&s).unwrap();
    let captured = &generator.captured.lock().unwrap()[0];
    assert!(captured.shell_tint_override.is_none());
}
