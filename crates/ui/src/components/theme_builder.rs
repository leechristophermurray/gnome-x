// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Theme Builder — visual customization of GTK4 and GNOME Shell appearance.
//! Generates CSS overrides for window radius, element radius, panel styling,
//! and accent-tinted surfaces (replaces Tint my Shell / Tint my GNOME).

use crate::services::AppServices;
use adw::prelude::*;
use gnomex_app::use_cases::ApplyThemeUseCase;
use gnomex_domain::{HexColor, Opacity, Radius, ThemeSpec, PanelSpec, DashSpec, TintSpec};
use relm4::prelude::*;
use std::sync::Arc;

const ACCENT_COLORS: &[(&str, &str, &str)] = &[
    ("blue", "Blue", "#3584e4"),
    ("teal", "Teal", "#2190a4"),
    ("green", "Green", "#3a944a"),
    ("yellow", "Yellow", "#c88800"),
    ("orange", "Orange", "#ed5b00"),
    ("red", "Red", "#e62d42"),
    ("pink", "Pink", "#d56199"),
    ("purple", "Purple", "#9141ac"),
    ("slate", "Slate", "#6f8396"),
];

/// Current theme builder state.
pub struct ThemeBuilderModel {
    apply_uc: Arc<ApplyThemeUseCase>,
    detected_version: String,
    wallpaper_swatches: gtk::Box,
    // Raw slider values (validated into ThemeSpec on Apply)
    window_radius: f64,
    element_radius: f64,
    panel_radius: f64,
    panel_opacity: f64,
    panel_tint_hex: String,
    tint_intensity: f64,
    dash_opacity: f64,
    overview_blur: bool,
}

#[derive(Debug)]
pub enum ThemeBuilderMsg {
    SetAccentColor(String),
    ExtractFromWallpaper,
    WallpaperColorsReady(Vec<(String, String)>), // Vec<(hex, closest_accent_id)>
    SetWindowRadius(f64),
    SetElementRadius(f64),
    SetPanelRadius(f64),
    SetPanelOpacity(f64),
    SetPanelTint(String),
    SetTintIntensity(f64),
    SetDashOpacity(f64),
    SetOverviewBlur(bool),
    Apply,
    Reset,
}

#[derive(Debug)]
pub enum ThemeBuilderOutput {
    Toast(String),
}

#[relm4::component(pub)]
impl SimpleComponent for ThemeBuilderModel {
    type Init = AppServices;
    type Input = ThemeBuilderMsg;
    type Output = ThemeBuilderOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
        }
    }

    fn init(
        services: AppServices,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let saved = load_saved_values();

        let scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let clamp = adw::Clamp::builder()
            .maximum_size(600)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        let outer = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .build();

        // === Accent Color section ===
        let accent_group = adw::PreferencesGroup::builder()
            .title("Accent Color")
            .description("System highlight color used across all apps")
            .build();

        let accent_row = adw::ActionRow::builder()
            .title("Color")
            .build();

        let color_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .valign(gtk::Align::Center)
            .build();

        let iface = gio::Settings::new("org.gnome.desktop.interface");
        let current_accent = iface.string("accent-color").to_string();

        for (i, &(id, label, hex)) in ACCENT_COLORS.iter().enumerate() {
            let btn = gtk::ToggleButton::builder()
                .width_request(32)
                .height_request(32)
                .tooltip_text(label)
                .active(id == current_accent)
                .build();

            let css_class = format!("accent-{id}");
            btn.add_css_class(&css_class);

            let css_provider = gtk::CssProvider::new();
            css_provider.load_from_string(&format!(
                ".{css_class} {{ \
                    background: {hex}; \
                    border-radius: 50%; \
                    min-width: 28px; \
                    min-height: 28px; \
                    padding: 0; \
                    border: 2px solid transparent; \
                }} \
                .{css_class}:checked {{ \
                    border-color: @theme_fg_color; \
                }}"
            ));
            gtk::style_context_add_provider_for_display(
                &gtk::gdk::Display::default().unwrap(),
                &css_provider,
                gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );

            if i > 0 {
                if let Some(first) = color_box.first_child() {
                    btn.set_group(first.downcast_ref::<gtk::ToggleButton>());
                }
            }

            let sender = sender.clone();
            let accent_id = id.to_owned();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    sender.input(ThemeBuilderMsg::SetAccentColor(accent_id.clone()));
                }
            });

            color_box.append(&btn);
        }

        accent_row.add_suffix(&color_box);
        accent_group.add(&accent_row);

        // "Material You" — extract colors from wallpaper
        let wallpaper_row = adw::ActionRow::builder()
            .title("From Wallpaper")
            .subtitle("Extract accent colors from your desktop wallpaper")
            .build();

        let wallpaper_action_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .valign(gtk::Align::Center)
            .build();

        let wallpaper_swatches = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .valign(gtk::Align::Center)
            .build();
        wallpaper_action_box.append(&wallpaper_swatches);

        let extract_btn = gtk::Button::builder()
            .label("Extract")
            .css_classes(["flat"])
            .build();
        {
            let sender = sender.clone();
            extract_btn.connect_clicked(move |_| {
                sender.input(ThemeBuilderMsg::ExtractFromWallpaper);
            });
        }
        wallpaper_action_box.append(&extract_btn);

        wallpaper_row.add_suffix(&wallpaper_action_box);
        accent_group.add(&wallpaper_row);

        outer.append(&accent_group);

        // === Radii section ===
        let radii_group = adw::PreferencesGroup::builder()
            .title("Border Radius")
            .description("Control the roundness of windows and UI elements")
            .build();

        let window_radius = build_spin_row(
            "Window Corners",
            "Corner radius for application windows",
            0.0, 48.0, saved.window_radius, 1.0,
        );
        {
            let sender = sender.clone();
            window_radius.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetWindowRadius(row.value()));
            });
        }
        radii_group.add(&window_radius);

        let element_radius = build_spin_row(
            "Element Radius",
            "Corner radius for buttons, entries, and cards",
            0.0, 24.0, saved.element_radius, 1.0,
        );
        {
            let sender = sender.clone();
            element_radius.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetElementRadius(row.value()));
            });
        }
        radii_group.add(&element_radius);

        outer.append(&radii_group);

        // === Panel section (Shell tinting) ===
        let panel_group = adw::PreferencesGroup::builder()
            .title("Shell Panel")
            .description("Customize the top panel appearance")
            .build();

        let panel_radius = build_spin_row(
            "Panel Corner Radius",
            "Roundness of the panel corners",
            0.0, 24.0, saved.panel_radius, 1.0,
        );
        {
            let sender = sender.clone();
            panel_radius.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetPanelRadius(row.value()));
            });
        }
        panel_group.add(&panel_radius);

        let panel_opacity = build_spin_row(
            "Panel Opacity",
            "0 = fully transparent, 100 = fully opaque",
            0.0, 100.0, saved.panel_opacity, 5.0,
        );
        {
            let sender = sender.clone();
            panel_opacity.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetPanelOpacity(row.value()));
            });
        }
        panel_group.add(&panel_opacity);

        // Panel tint color
        let tint_row = adw::ActionRow::builder()
            .title("Panel Tint Color")
            .subtitle("Background color blend for the panel")
            .build();

        let color_btn = gtk::ColorDialogButton::builder()
            .valign(gtk::Align::Center)
            .build();
        let dialog = gtk::ColorDialog::builder()
            .title("Panel Tint Color")
            .with_alpha(false)
            .build();
        color_btn.set_dialog(&dialog);
        // Set saved panel tint color
        if let Ok(rgba) = gtk::gdk::RGBA::parse(&saved.panel_tint_hex) {
            color_btn.set_rgba(&rgba);
        }

        {
            let sender = sender.clone();
            color_btn.connect_rgba_notify(move |btn| {
                let rgba = btn.rgba();
                let hex = format!(
                    "#{:02x}{:02x}{:02x}",
                    (rgba.red() * 255.0) as u8,
                    (rgba.green() * 255.0) as u8,
                    (rgba.blue() * 255.0) as u8,
                );
                sender.input(ThemeBuilderMsg::SetPanelTint(hex));
            });
        }
        tint_row.add_suffix(&color_btn);
        panel_group.add(&tint_row);

        outer.append(&panel_group);

        // === Accent Tinting (Tint my GNOME-style) ===
        let tint_group = adw::PreferencesGroup::builder()
            .title("Accent Tinting")
            .description("Blend the accent color into window backgrounds")
            .build();

        let tint_intensity = build_spin_row(
            "Tint Intensity",
            "How much accent color bleeds into surfaces (0\u{2013}20%)",
            0.0, 20.0, saved.tint_intensity, 1.0,
        );
        {
            let sender = sender.clone();
            tint_intensity.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetTintIntensity(row.value()));
            });
        }
        tint_group.add(&tint_intensity);

        outer.append(&tint_group);

        // === Dash / Overview ===
        let dash_group = adw::PreferencesGroup::builder()
            .title("Dash & Overview")
            .description("Customize the app launcher and activities view")
            .build();

        let dash_opacity = build_spin_row(
            "Dash Opacity",
            "Transparency of the dash background",
            0.0, 100.0, saved.dash_opacity, 5.0,
        );
        {
            let sender = sender.clone();
            dash_opacity.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetDashOpacity(row.value()));
            });
        }
        dash_group.add(&dash_opacity);

        let blur_switch = adw::SwitchRow::builder()
            .title("Overview Background Blur")
            .subtitle("Apply a blur effect to the overview background")
            .active(true)
            .build();
        {
            let sender = sender.clone();
            blur_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetOverviewBlur(row.is_active()));
            });
        }
        dash_group.add(&blur_switch);

        outer.append(&dash_group);

        // === Action buttons ===
        let action_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .halign(gtk::Align::End)
            .spacing(12)
            .margin_top(12)
            .build();

        let reset_btn = gtk::Button::builder()
            .label("Reset to Defaults")
            .css_classes(["flat"])
            .build();
        {
            let sender = sender.clone();
            reset_btn.connect_clicked(move |_| {
                sender.input(ThemeBuilderMsg::Reset);
            });
        }
        action_box.append(&reset_btn);

        let apply_btn = gtk::Button::builder()
            .label("Apply Theme")
            .css_classes(["suggested-action", "pill"])
            .build();
        {
            let sender = sender.clone();
            apply_btn.connect_clicked(move |_| {
                sender.input(ThemeBuilderMsg::Apply);
            });
        }
        action_box.append(&apply_btn);

        outer.append(&action_box);

        clamp.set_child(Some(&outer));
        scroll.set_child(Some(&clamp));
        root.append(&scroll);

        let model = ThemeBuilderModel {
            apply_uc: services.apply_theme,
            detected_version: services.detected_gnome_version,
            wallpaper_swatches: wallpaper_swatches.clone(),
            window_radius: saved.window_radius,
            element_radius: saved.element_radius,
            panel_radius: saved.panel_radius,
            panel_opacity: saved.panel_opacity,
            panel_tint_hex: saved.panel_tint_hex,
            tint_intensity: saved.tint_intensity,
            dash_opacity: saved.dash_opacity,
            overview_blur: true,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ThemeBuilderMsg, sender: ComponentSender<Self>) {
        match msg {
            ThemeBuilderMsg::SetAccentColor(color) => {
                let iface = gio::Settings::new("org.gnome.desktop.interface");
                match iface.set_string("accent-color", &color) {
                    Ok(_) => tracing::info!("accent color set to: {color}"),
                    Err(e) => tracing::warn!("failed to set accent color: {e}"),
                }
            }
            ThemeBuilderMsg::ExtractFromWallpaper => {
                let bg = gio::Settings::new("org.gnome.desktop.background");
                let uri = bg.string("picture-uri").to_string();
                let path = uri
                    .strip_prefix("file://")
                    .unwrap_or(&uri)
                    .to_owned();

                let sender = sender.input_sender().clone();
                // Run extraction off the main thread since it loads an image
                std::thread::spawn(move || {
                    match extract_wallpaper_colors(&path) {
                        Ok(colors) => sender.emit(ThemeBuilderMsg::WallpaperColorsReady(colors)),
                        Err(e) => {
                            tracing::warn!("wallpaper extraction failed: {e}");
                        }
                    }
                });
            }
            ThemeBuilderMsg::WallpaperColorsReady(colors) => {
                // Clear previous swatches
                while let Some(child) = self.wallpaper_swatches.first_child() {
                    self.wallpaper_swatches.remove(&child);
                }

                // Create clickable color swatches
                for (hex, accent_id) in &colors {
                    let btn = gtk::Button::builder()
                        .width_request(28)
                        .height_request(28)
                        .tooltip_text(&format!("{hex} \u{2192} {accent_id}"))
                        .build();

                    let css_class = format!(
                        "wallpaper-swatch-{}",
                        hex.trim_start_matches('#')
                    );
                    btn.add_css_class(&css_class);

                    let css_provider = gtk::CssProvider::new();
                    css_provider.load_from_string(&format!(
                        ".{css_class} {{ \
                            background: {hex}; \
                            border-radius: 50%; \
                            min-width: 24px; \
                            min-height: 24px; \
                            padding: 0; \
                            border: 2px solid rgba(255,255,255,0.3); \
                        }}"
                    ));
                    gtk::style_context_add_provider_for_display(
                        &gtk::gdk::Display::default().unwrap(),
                        &css_provider,
                        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                    );

                    let sender = sender.clone();
                    let aid = accent_id.clone();
                    btn.connect_clicked(move |_| {
                        sender.input(ThemeBuilderMsg::SetAccentColor(aid.clone()));
                    });

                    self.wallpaper_swatches.append(&btn);
                }

                if colors.is_empty() {
                    let _ = sender.output(ThemeBuilderOutput::Toast(
                        "No colors extracted".into(),
                    ));
                } else {
                    let _ = sender.output(ThemeBuilderOutput::Toast(format!(
                        "Extracted {} colors from wallpaper",
                        colors.len()
                    )));
                }
            }
            ThemeBuilderMsg::SetWindowRadius(v) => {
                self.window_radius = v;
            }
            ThemeBuilderMsg::SetElementRadius(v) => {
                self.element_radius = v;
            }
            ThemeBuilderMsg::SetPanelRadius(v) => self.panel_radius = v,
            ThemeBuilderMsg::SetPanelOpacity(v) => self.panel_opacity = v,
            ThemeBuilderMsg::SetPanelTint(hex) => self.panel_tint_hex = hex,
            ThemeBuilderMsg::SetTintIntensity(v) => {
                self.tint_intensity = v;
            }
            ThemeBuilderMsg::SetDashOpacity(v) => self.dash_opacity = v,
            ThemeBuilderMsg::SetOverviewBlur(v) => self.overview_blur = v,
            ThemeBuilderMsg::Apply => {
                match self.build_spec() {
                    Ok(spec) => match self.apply_uc.apply(&spec) {
                        Ok(()) => {
                            let ver = &self.detected_version;
                            let _ = sender.output(ThemeBuilderOutput::Toast(
                                format!("Theme applied ({ver}) \u{2014} restart apps to see changes"),
                            ));
                        }
                        Err(e) => {
                            let _ = sender
                                .output(ThemeBuilderOutput::Toast(format!("Error: {e}")));
                        }
                    },
                    Err(e) => {
                        let _ =
                            sender.output(ThemeBuilderOutput::Toast(format!("Invalid: {e}")));
                    }
                }
            }
            ThemeBuilderMsg::Reset => {
                self.window_radius = 12.0;
                self.element_radius = 6.0;
                self.panel_radius = 0.0;
                self.panel_opacity = 80.0;
                self.panel_tint_hex = "#1a1a1e".into();
                self.tint_intensity = 5.0;
                self.dash_opacity = 70.0;
                self.overview_blur = true;

                let _ = self.apply_uc.reset();
                let _ = sender
                    .output(ThemeBuilderOutput::Toast("Reset to defaults".into()));
            }
        }
    }
}

impl ThemeBuilderModel {
    /// Build a validated ThemeSpec from the current slider values.
    fn build_spec(&self) -> Result<ThemeSpec, gnomex_domain::DomainError> {
        let accent_hex = current_accent_hex();
        Ok(ThemeSpec {
            window_radius: Radius::new(self.window_radius)?,
            element_radius: Radius::new(self.element_radius)?,
            panel: PanelSpec {
                radius: Radius::new(self.panel_radius)?,
                opacity: Opacity::from_percent(self.panel_opacity)?,
                tint: HexColor::new(&self.panel_tint_hex)?,
            },
            dash: DashSpec {
                opacity: Opacity::from_percent(self.dash_opacity)?,
            },
            tint: TintSpec {
                accent_hex: HexColor::new(&accent_hex)?,
                intensity: Opacity::from_percent(self.tint_intensity)?,
            },
            overview_blur: self.overview_blur,
        })
    }
}

/// Read the current accent color hex from GSettings.
fn current_accent_hex() -> String {
    let iface = gio::Settings::new("org.gnome.desktop.interface");
    let name = iface.string("accent-color").to_string();
    ACCENT_COLORS
        .iter()
        .find(|(id, _, _)| *id == name)
        .map(|(_, _, hex)| hex.to_string())
        .unwrap_or_else(|| "#3584e4".into())
}

fn build_spin_row(
    title: &str,
    subtitle: &str,
    min: f64,
    max: f64,
    value: f64,
    step: f64,
) -> adw::SpinRow {
    let adjustment = gtk::Adjustment::new(value, min, max, step, step * 5.0, 0.0);
    adw::SpinRow::builder()
        .title(title)
        .subtitle(subtitle)
        .adjustment(&adjustment)
        .build()
}

/// Saved theme values, parsed from existing CSS files.
#[derive(Debug)]
struct SavedValues {
    window_radius: f64,
    element_radius: f64,
    panel_radius: f64,
    panel_opacity: f64,
    panel_tint_hex: String,
    tint_intensity: f64,
    dash_opacity: f64,
}

impl Default for SavedValues {
    fn default() -> Self {
        Self {
            window_radius: 12.0,
            element_radius: 6.0,
            panel_radius: 0.0,
            panel_opacity: 80.0,
            panel_tint_hex: "#1a1a1e".into(),
            tint_intensity: 5.0,
            dash_opacity: 70.0,
        }
    }
}

/// Parse saved values from existing CSS files on disk.
fn load_saved_values() -> SavedValues {
    let mut vals = SavedValues::default();
    let Some(home) = dirs::home_dir() else {
        return vals;
    };

    // Parse GTK CSS
    if let Ok(gtk_css) = std::fs::read_to_string(home.join(".config/gtk-4.0/gtk.css")) {
        if let Some(v) = extract_px(&gtk_css, "window.background { border-radius:") {
            vals.window_radius = v;
        }
        if let Some(v) = extract_px(&gtk_css, "button { border-radius:") {
            vals.element_radius = v;
        }
        // Tint intensity: parse from the mix() percentage in dark mode
        // Looking for: mix(#222226, @accent_bg_color, 0.05)
        if let Some(v) = extract_mix_pct(&gtk_css) {
            vals.tint_intensity = (v * 100.0).round();
        }
    }

    // Parse Shell CSS
    let shell_path = home
        .join(".local/share/themes/GNOME-X-Custom/gnome-shell")
        .join("gnome-shell.css");
    if let Ok(shell_css) = std::fs::read_to_string(shell_path) {
        // Panel opacity: alpha(#xxx, 0.8) → 80
        if let Some(v) = extract_alpha_opacity(&shell_css, "#panel") {
            vals.panel_opacity = (v * 100.0).round();
        }
        // Panel radius: border-radius: 0 0 Xpx Xpx
        if let Some(v) = extract_panel_radius(&shell_css) {
            vals.panel_radius = v;
        }
        // Panel tint: alpha(#xxxxxx, ...)
        if let Some(hex) = extract_alpha_color(&shell_css, "#panel") {
            vals.panel_tint_hex = hex;
        }
        // Dash opacity
        if let Some(v) = extract_alpha_opacity(&shell_css, "#dash") {
            vals.dash_opacity = (v * 100.0).round();
        }
    }

    vals
}

/// Extract a `Xpx` value after a pattern like `selector { border-radius: Xpx`.
fn extract_px(css: &str, pattern: &str) -> Option<f64> {
    let idx = css.find(pattern)?;
    let after = &css[idx + pattern.len()..];
    let trimmed = after.trim_start();
    let num_end = trimmed.find(|c: char| !c.is_ascii_digit() && c != '.')?;
    trimmed[..num_end].parse().ok()
}

/// Extract the mix() percentage from dark mode tint definition.
fn extract_mix_pct(css: &str) -> Option<f64> {
    // Find the dark mode section and look for a mix() with @accent_bg_color
    let dark_idx = css.find("prefers-color-scheme: dark")?;
    let dark_section = &css[dark_idx..];
    let mix_idx = dark_section.find("mix(#222226, @accent_bg_color,")?;
    let after = &dark_section[mix_idx..];
    let comma_idx = after.rfind(',')?;
    let rest = after[comma_idx + 1..].trim_start();
    let end = rest.find(')')?;
    rest[..end].trim().parse().ok()
}

/// Extract the opacity value from `alpha(#xxx, OPACITY)` in a selector block.
fn extract_alpha_opacity(css: &str, selector: &str) -> Option<f64> {
    let sel_idx = css.find(selector)?;
    let block = &css[sel_idx..];
    let alpha_idx = block.find("alpha(")?;
    let after = &block[alpha_idx..];
    let comma = after.find(',')?;
    let rest = &after[comma + 1..];
    let paren = rest.find(')')?;
    rest[..paren].trim().parse().ok()
}

/// Extract the hex color from `alpha(#xxxxxx, ...)` in a selector block.
fn extract_alpha_color(css: &str, selector: &str) -> Option<String> {
    let sel_idx = css.find(selector)?;
    let block = &css[sel_idx..];
    let alpha_idx = block.find("alpha(")?;
    let after = &block[alpha_idx + 6..];
    let comma = after.find(',')?;
    let hex = after[..comma].trim();
    if hex.starts_with('#') {
        Some(hex.to_owned())
    } else {
        None
    }
}

/// Extract panel border-radius from `border-radius: 0 0 Xpx Xpx`.
fn extract_panel_radius(css: &str) -> Option<f64> {
    let panel_idx = css.find("#panel")?;
    let block = &css[panel_idx..];
    let br_idx = block.find("border-radius:")?;
    let after = &block[br_idx + 14..];
    // Format: "0 0 Xpx Xpx" — find the third token
    let tokens: Vec<&str> = after.trim_start().split_whitespace().take(4).collect();
    if tokens.len() >= 3 {
        tokens[2].trim_end_matches("px").parse().ok()
    } else {
        None
    }
}

// --- Wallpaper color extraction (Material You style) ---

/// Extract dominant colors from a wallpaper image and map them to GNOME accent colors.
/// Returns up to 5 color swatches as (hex, closest_accent_id) pairs.
fn extract_wallpaper_colors(
    path: &str,
) -> Result<Vec<(String, String)>, String> {
    // Load and downscale image for fast processing
    let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, 64, 64, true)
        .map_err(|e| format!("failed to load wallpaper: {e}"))?;

    let pixels = pixbuf.pixel_bytes().ok_or("no pixel data")?;
    let channels = pixbuf.n_channels() as usize;
    let data = pixels.as_ref();

    // Collect pixel colors as (H, S, V) with enough saturation to be interesting
    let mut color_buckets: std::collections::HashMap<(u16, u8, u8), u32> = std::collections::HashMap::new();

    for chunk in data.chunks_exact(channels) {
        if channels < 3 {
            continue;
        }
        let (r, g, b) = (chunk[0], chunk[1], chunk[2]);
        let (h, s, v) = gnomex_domain::color::rgb_to_hsv(r, g, b);

        // Skip near-grey, very dark, or very bright pixels
        if s < 15 || v < 25 || v > 240 {
            continue;
        }

        // Bucket: hue in 10-degree bins, saturation in 3 bins, value in 3 bins
        let hue_bin = (h / 10) * 10;
        let sat_bin = (s / 85).min(2);
        let val_bin = (v / 85).min(2);
        *color_buckets.entry((hue_bin, sat_bin, val_bin)).or_default() += 1;
    }

    // Sort by frequency, take top clusters
    let mut buckets: Vec<_> = color_buckets.into_iter().collect();
    buckets.sort_by(|a, b| b.1.cmp(&a.1));

    let mut results = Vec::new();
    let mut used_accents = std::collections::HashSet::new();

    for ((hue_bin, sat_bin, val_bin), _count) in buckets.iter().take(15) {
        // Reconstruct a representative color from the bucket center
        let h = *hue_bin + 5;
        let s = (*sat_bin as u16) * 85 + 42;
        let v = (*val_bin as u16) * 85 + 42;
        let (r, g, b) = gnomex_domain::color::hsv_to_rgb(h, s.min(255) as u8, v.min(255) as u8);
        let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);

        // Map to closest GNOME accent color
        let accent_id = closest_accent(r, g, b);

        // Only include each accent once
        if used_accents.contains(&accent_id) {
            continue;
        }
        used_accents.insert(accent_id.clone());
        results.push((hex, accent_id));

        if results.len() >= 5 {
            break;
        }
    }

    Ok(results)
}

/// Find the closest GNOME accent color for an RGB value.
fn closest_accent(r: u8, g: u8, b: u8) -> String {
    use gnomex_domain::color::color_distance;

    let mut best_id = "blue";
    let mut best_dist = u32::MAX;

    for &(id, _, hex) in ACCENT_COLORS {
        let accent = gnomex_domain::HexColor::new(hex)
            .map(|c| c.to_rgb())
            .unwrap_or((0, 0, 0));
        let dist = color_distance((r, g, b), accent);
        if dist < best_dist {
            best_dist = dist;
            best_id = id;
        }
    }

    best_id.to_owned()
}

// Color utilities delegate to gnomex_domain::color

/// Convenience re-export of dirs for home directory.
mod dirs {
    pub fn home_dir() -> Option<std::path::PathBuf> {
        std::env::var("HOME").ok().map(std::path::PathBuf::from)
    }
}
