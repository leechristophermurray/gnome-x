// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! End-to-end test for the Material-palette branch of
//! `ApplyThemeUseCase`: feed a stub palette, flip the toggle, assert
//! the CSS generator receives a spec whose `widget_colors` were
//! derived from the palette permutation (not whatever
//! `widget_colors` the caller originally supplied).

use gnomex_app::ports::{
    AppearanceSettings, ThemeCss, ThemeCssGenerator, ThemeWriter, WallpaperPaletteProvider,
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
struct NoopAppearance;
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
    let appearance = Arc::new(NoopAppearance);
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
    let appearance = Arc::new(NoopAppearance);
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
    let appearance = Arc::new(NoopAppearance);
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
    let appearance = Arc::new(NoopAppearance);
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
    let appearance = Arc::new(NoopAppearance);
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
