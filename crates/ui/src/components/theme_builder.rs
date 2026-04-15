// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Theme Builder — visual customization of GTK4 and GNOME Shell appearance.
//! Generates CSS overrides for window radius, element radius, panel styling,
//! and accent-tinted surfaces (replaces Tint my Shell / Tint my GNOME).

use crate::services::AppServices;
use adw::prelude::*;
use gnomex_app::use_cases::ApplyThemeUseCase;
use gnomex_domain::theme_capability::{self, ThemeControlId};
use gnomex_domain::{DashSpec, HexColor, Opacity, PanelSpec, Radius, ThemeSpec, TintSpec};
use relm4::prelude::*;
use std::collections::HashMap;
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
    compat_rows: HashMap<ThemeControlId, (adw::SpinRow, String)>, // (row, original_subtitle)
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
    SetButtonLayout(String),
    SetTitlebarDoubleClick(String),
    SetTitlebarFont(String),
    SetNightLight(bool),
    SetNightLightTemp(u32),
    SetNightLightAuto(bool),
    SetOverAmplification(bool),
    SetEventSounds(bool),
    SetColorScheme(String),
    SetFont(String),
    SetMonoFont(String),
    SetTextScale(f64),
    SetFontHinting(String),
    SetCursorSize(i32),
    SetAnimations(bool),
    SetHotCorners(bool),
    RefreshCompat,
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

        // === Detected version banner ===
        let version_banner = adw::ActionRow::builder()
            .title("Desktop Environment")
            .subtitle(&format!(
                "{} \u{2014} theme engine adapter active",
                services.detected_gnome_version,
            ))
            .css_classes(["property"])
            .build();
        version_banner.add_prefix(
            &gtk::Image::from_icon_name("computer-symbolic"),
        );
        outer.append(&version_banner);

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

        // === Color Scheme ===
        let scheme_group = adw::PreferencesGroup::builder()
            .title("Color Scheme")
            .description("Control light and dark appearance")
            .build();

        let iface = gio::Settings::new("org.gnome.desktop.interface");
        let current_scheme = iface.string("color-scheme").to_string();

        let scheme_row = adw::ComboRow::builder()
            .title("Appearance")
            .subtitle("Set the system color scheme for all apps")
            .build();
        let scheme_list = gtk::StringList::new(&["Default", "Prefer Light", "Prefer Dark"]);
        scheme_row.set_model(Some(&scheme_list));
        scheme_row.set_selected(match current_scheme.as_str() {
            "prefer-light" => 1,
            "prefer-dark" => 2,
            _ => 0,
        });
        {
            let sender = sender.clone();
            scheme_row.connect_selected_notify(move |row| {
                let scheme = match row.selected() {
                    1 => "prefer-light",
                    2 => "prefer-dark",
                    _ => "default",
                };
                sender.input(ThemeBuilderMsg::SetColorScheme(scheme.into()));
            });
        }
        scheme_group.add(&scheme_row);
        outer.append(&scheme_group);

        // === Typography ===
        let font_group = adw::PreferencesGroup::builder()
            .title("Typography")
            .description("Fonts and text rendering")
            .build();

        let current_font = iface.string("font-name").to_string();
        let font_row = adw::ActionRow::builder()
            .title("Interface Font")
            .subtitle(&current_font)
            .activatable(true)
            .build();
        let font_btn = gtk::FontDialogButton::builder()
            .valign(gtk::Align::Center)
            .build();
        let font_dialog = gtk::FontDialog::builder()
            .title("Interface Font")
            .build();
        font_btn.set_dialog(&font_dialog);
        if let Some(desc) = gtk::pango::FontDescription::from_string(&current_font).into() {
            font_btn.set_font_desc(&desc);
        }
        {
            let sender = sender.clone();
            font_btn.connect_font_desc_notify(move |btn| {
                if let Some(desc) = btn.font_desc() {
                    sender.input(ThemeBuilderMsg::SetFont(desc.to_string()));
                }
            });
        }
        font_row.add_suffix(&font_btn);
        font_group.add(&font_row);

        let current_mono = iface.string("monospace-font-name").to_string();
        let mono_row = adw::ActionRow::builder()
            .title("Monospace Font")
            .subtitle(&current_mono)
            .activatable(true)
            .build();
        let mono_btn = gtk::FontDialogButton::builder()
            .valign(gtk::Align::Center)
            .build();
        let mono_dialog = gtk::FontDialog::builder()
            .title("Monospace Font")
            .build();
        mono_btn.set_dialog(&mono_dialog);
        if let Some(desc) = gtk::pango::FontDescription::from_string(&current_mono).into() {
            mono_btn.set_font_desc(&desc);
        }
        {
            let sender = sender.clone();
            mono_btn.connect_font_desc_notify(move |btn| {
                if let Some(desc) = btn.font_desc() {
                    sender.input(ThemeBuilderMsg::SetMonoFont(desc.to_string()));
                }
            });
        }
        mono_row.add_suffix(&mono_btn);
        font_group.add(&mono_row);

        let current_scale = iface.double("text-scaling-factor");
        let scale_row = build_spin_row(
            "Text Scaling",
            "Scale factor for all text (1.0 = 100%)",
            0.5, 3.0, current_scale, 0.1,
        );
        {
            let sender = sender.clone();
            scale_row.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetTextScale(row.value()));
            });
        }
        font_group.add(&scale_row);

        let current_hinting = iface.string("font-hinting").to_string();
        let hinting_row = adw::ComboRow::builder()
            .title("Font Hinting")
            .subtitle("Controls sharpness of text at small sizes")
            .build();
        let hinting_list = gtk::StringList::new(&["None", "Slight", "Medium", "Full"]);
        hinting_row.set_model(Some(&hinting_list));
        hinting_row.set_selected(match current_hinting.as_str() {
            "none" => 0,
            "slight" => 1,
            "medium" => 2,
            _ => 3,
        });
        {
            let sender = sender.clone();
            hinting_row.connect_selected_notify(move |row| {
                let hint = match row.selected() {
                    0 => "none",
                    1 => "slight",
                    2 => "medium",
                    _ => "full",
                };
                sender.input(ThemeBuilderMsg::SetFontHinting(hint.into()));
            });
        }
        font_group.add(&hinting_row);
        outer.append(&font_group);

        // === Cursor ===
        let cursor_group = adw::PreferencesGroup::builder()
            .title("Cursor")
            .description("Pointer appearance")
            .build();

        let current_cursor_size = iface.int("cursor-size") as f64;
        let cursor_size_row = build_spin_row(
            "Cursor Size",
            "Size of the mouse pointer in pixels",
            16.0, 96.0, current_cursor_size, 8.0,
        );
        {
            let sender = sender.clone();
            cursor_size_row.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetCursorSize(row.value() as i32));
            });
        }
        cursor_group.add(&cursor_size_row);
        outer.append(&cursor_group);

        // === Desktop Behavior ===
        let behavior_group = adw::PreferencesGroup::builder()
            .title("Desktop Behavior")
            .description("System-wide interaction settings")
            .build();

        let animations_switch = adw::SwitchRow::builder()
            .title("Animations")
            .subtitle("Enable or disable UI transitions and animations")
            .active(iface.boolean("enable-animations"))
            .build();
        {
            let sender = sender.clone();
            animations_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetAnimations(row.is_active()));
            });
        }
        behavior_group.add(&animations_switch);

        let hot_corners_switch = adw::SwitchRow::builder()
            .title("Hot Corners")
            .subtitle("Trigger Activities by pushing the pointer to the top-left corner")
            .active(iface.boolean("enable-hot-corners"))
            .build();
        {
            let sender = sender.clone();
            hot_corners_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetHotCorners(row.is_active()));
            });
        }
        behavior_group.add(&hot_corners_switch);
        outer.append(&behavior_group);

        // === Window Manager ===
        let wm_group = adw::PreferencesGroup::builder()
            .title("Window Titlebar")
            .description("Titlebar buttons and actions")
            .build();

        let wm = gio::Settings::new("org.gnome.desktop.wm.preferences");

        // Titlebar button layout
        let current_layout = wm.string("button-layout").to_string();
        let layout_row = adw::ComboRow::builder()
            .title("Button Layout")
            .subtitle("Position of window control buttons")
            .build();
        let layout_options = gtk::StringList::new(&[
            "Close only (right)",
            "Minimize, Close (right)",
            "Minimize, Maximize, Close (right)",
            "Close (left)",
            "Close, Minimize (left)",
            "Close, Minimize, Maximize (left)",
        ]);
        layout_row.set_model(Some(&layout_options));
        let layout_values = [
            "appmenu:close",
            "appmenu:minimize,close",
            "appmenu:minimize,maximize,close",
            "close:appmenu",
            "close,minimize:appmenu",
            "close,minimize,maximize:appmenu",
        ];
        let selected_layout = layout_values
            .iter()
            .position(|&v| v == current_layout)
            .unwrap_or(2) as u32;
        layout_row.set_selected(selected_layout);
        {
            let sender = sender.clone();
            layout_row.connect_selected_notify(move |row| {
                let idx = row.selected() as usize;
                if let Some(&layout) = layout_values.get(idx) {
                    sender.input(ThemeBuilderMsg::SetButtonLayout(layout.into()));
                }
            });
        }
        wm_group.add(&layout_row);

        // Double-click titlebar action
        let current_dbl = wm.string("action-double-click-titlebar").to_string();
        let dbl_row = adw::ComboRow::builder()
            .title("Double-Click Titlebar")
            .subtitle("Action when double-clicking a window titlebar")
            .build();
        let dbl_options = gtk::StringList::new(&[
            "Toggle Maximize",
            "Minimize",
            "Lower Window",
            "Menu",
            "None",
        ]);
        dbl_row.set_model(Some(&dbl_options));
        let dbl_values = [
            "toggle-maximize",
            "minimize",
            "lower",
            "menu",
            "none",
        ];
        let selected_dbl = dbl_values
            .iter()
            .position(|&v| v == current_dbl)
            .unwrap_or(0) as u32;
        dbl_row.set_selected(selected_dbl);
        {
            let sender = sender.clone();
            dbl_row.connect_selected_notify(move |row| {
                let idx = row.selected() as usize;
                if let Some(&action) = dbl_values.get(idx) {
                    sender.input(ThemeBuilderMsg::SetTitlebarDoubleClick(action.into()));
                }
            });
        }
        wm_group.add(&dbl_row);

        // Titlebar font
        let current_tb_font = wm.string("titlebar-font").to_string();
        let tb_font_row = adw::ActionRow::builder()
            .title("Titlebar Font")
            .subtitle(&current_tb_font)
            .build();
        let tb_font_btn = gtk::FontDialogButton::builder()
            .valign(gtk::Align::Center)
            .build();
        let tb_font_dialog = gtk::FontDialog::builder()
            .title("Titlebar Font")
            .build();
        tb_font_btn.set_dialog(&tb_font_dialog);
        if let Some(desc) = gtk::pango::FontDescription::from_string(&current_tb_font).into() {
            tb_font_btn.set_font_desc(&desc);
        }
        {
            let sender = sender.clone();
            tb_font_btn.connect_font_desc_notify(move |btn| {
                if let Some(desc) = btn.font_desc() {
                    sender.input(ThemeBuilderMsg::SetTitlebarFont(desc.to_string()));
                }
            });
        }
        tb_font_row.add_suffix(&tb_font_btn);
        wm_group.add(&tb_font_row);

        outer.append(&wm_group);

        // === Night Light ===
        let nl_group = adw::PreferencesGroup::builder()
            .title("Night Light")
            .description("Reduce blue light in the evening")
            .build();

        let color_settings = gio::Settings::new("org.gnome.settings-daemon.plugins.color");

        let nl_switch = adw::SwitchRow::builder()
            .title("Night Light")
            .subtitle("Warm the screen color to reduce eye strain at night")
            .active(color_settings.boolean("night-light-enabled"))
            .build();
        {
            let sender = sender.clone();
            nl_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetNightLight(row.is_active()));
            });
        }
        nl_group.add(&nl_switch);

        let current_temp = color_settings.uint("night-light-temperature") as f64;
        let temp_row = build_spin_row(
            "Color Temperature",
            "Lower values are warmer (1000\u{2013}6500 K)",
            1000.0, 6500.0, current_temp, 100.0,
        );
        {
            let sender = sender.clone();
            temp_row.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetNightLightTemp(row.value() as u32));
            });
        }
        nl_group.add(&temp_row);

        let nl_auto = adw::SwitchRow::builder()
            .title("Automatic Schedule")
            .subtitle("Use sunrise and sunset times based on location")
            .active(color_settings.boolean("night-light-schedule-automatic"))
            .build();
        {
            let sender = sender.clone();
            nl_auto.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetNightLightAuto(row.is_active()));
            });
        }
        nl_group.add(&nl_auto);

        outer.append(&nl_group);

        // === Sound ===
        let sound_group = adw::PreferencesGroup::builder()
            .title("Sound")
            .description("Audio feedback settings")
            .build();

        let sound_settings = gio::Settings::new("org.gnome.desktop.sound");

        let overamp_switch = adw::SwitchRow::builder()
            .title("Allow Volume Above 100%")
            .subtitle("Enable amplification beyond the default maximum")
            .active(sound_settings.boolean("allow-volume-above-100-percent"))
            .build();
        {
            let sender = sender.clone();
            overamp_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetOverAmplification(row.is_active()));
            });
        }
        sound_group.add(&overamp_switch);

        let event_sounds_switch = adw::SwitchRow::builder()
            .title("Event Sounds")
            .subtitle("Play sounds for system events like alerts and notifications")
            .active(sound_settings.boolean("event-sounds"))
            .build();
        {
            let sender = sender.clone();
            event_sounds_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetEventSounds(row.is_active()));
            });
        }
        sound_group.add(&event_sounds_switch);

        outer.append(&sound_group);

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
            compat_rows: HashMap::from([
                (ThemeControlId::WindowRadius, (window_radius, "Corner radius for application windows".into())),
                (ThemeControlId::ElementRadius, (element_radius, "Corner radius for buttons, entries, and cards".into())),
                (ThemeControlId::PanelRadius, (panel_radius, "Roundness of the panel corners".into())),
                (ThemeControlId::AccentTint, (tint_intensity, "How much accent color bleeds into surfaces (0\u{2013}20%)".into())),
                (ThemeControlId::DashOpacity, (dash_opacity, "Transparency of the dash background".into())),
            ]),
        };
        let widgets = view_output!();

        // Initial compatibility check
        sender.input(ThemeBuilderMsg::RefreshCompat);

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
                sender.input(ThemeBuilderMsg::RefreshCompat);
            }
            ThemeBuilderMsg::SetElementRadius(v) => {
                self.element_radius = v;
                sender.input(ThemeBuilderMsg::RefreshCompat);
            }
            ThemeBuilderMsg::SetPanelRadius(v) => {
                self.panel_radius = v;
                sender.input(ThemeBuilderMsg::RefreshCompat);
            }
            ThemeBuilderMsg::SetPanelOpacity(v) => self.panel_opacity = v,
            ThemeBuilderMsg::SetPanelTint(hex) => self.panel_tint_hex = hex,
            ThemeBuilderMsg::SetTintIntensity(v) => {
                self.tint_intensity = v;
            }
            ThemeBuilderMsg::SetDashOpacity(v) => self.dash_opacity = v,
            ThemeBuilderMsg::SetOverviewBlur(v) => self.overview_blur = v,
            ThemeBuilderMsg::SetButtonLayout(layout) => {
                let wm = gio::Settings::new("org.gnome.desktop.wm.preferences");
                let _ = wm.set_string("button-layout", &layout);
                tracing::info!("button layout: {layout}");
            }
            ThemeBuilderMsg::SetTitlebarDoubleClick(action) => {
                let wm = gio::Settings::new("org.gnome.desktop.wm.preferences");
                let _ = wm.set_string("action-double-click-titlebar", &action);
                tracing::info!("titlebar double-click: {action}");
            }
            ThemeBuilderMsg::SetTitlebarFont(font) => {
                let wm = gio::Settings::new("org.gnome.desktop.wm.preferences");
                let _ = wm.set_string("titlebar-font", &font);
                tracing::info!("titlebar font: {font}");
            }
            ThemeBuilderMsg::SetNightLight(enabled) => {
                let cs = gio::Settings::new("org.gnome.settings-daemon.plugins.color");
                let _ = cs.set_boolean("night-light-enabled", enabled);
                tracing::info!("night light: {enabled}");
            }
            ThemeBuilderMsg::SetNightLightTemp(temp) => {
                let cs = gio::Settings::new("org.gnome.settings-daemon.plugins.color");
                let _ = cs.set_uint("night-light-temperature", temp);
                tracing::info!("night light temp: {temp}K");
            }
            ThemeBuilderMsg::SetNightLightAuto(auto) => {
                let cs = gio::Settings::new("org.gnome.settings-daemon.plugins.color");
                let _ = cs.set_boolean("night-light-schedule-automatic", auto);
                tracing::info!("night light auto schedule: {auto}");
            }
            ThemeBuilderMsg::SetOverAmplification(enabled) => {
                let sound = gio::Settings::new("org.gnome.desktop.sound");
                let _ = sound.set_boolean("allow-volume-above-100-percent", enabled);
                tracing::info!("over-amplification: {enabled}");
            }
            ThemeBuilderMsg::SetEventSounds(enabled) => {
                let sound = gio::Settings::new("org.gnome.desktop.sound");
                let _ = sound.set_boolean("event-sounds", enabled);
                tracing::info!("event sounds: {enabled}");
            }
            ThemeBuilderMsg::SetColorScheme(scheme) => {
                let iface = gio::Settings::new("org.gnome.desktop.interface");
                let _ = iface.set_string("color-scheme", &scheme);
                tracing::info!("color scheme set to: {scheme}");
            }
            ThemeBuilderMsg::SetFont(font) => {
                let iface = gio::Settings::new("org.gnome.desktop.interface");
                let _ = iface.set_string("font-name", &font);
                tracing::info!("font set to: {font}");
            }
            ThemeBuilderMsg::SetMonoFont(font) => {
                let iface = gio::Settings::new("org.gnome.desktop.interface");
                let _ = iface.set_string("monospace-font-name", &font);
                tracing::info!("monospace font set to: {font}");
            }
            ThemeBuilderMsg::SetTextScale(scale) => {
                let iface = gio::Settings::new("org.gnome.desktop.interface");
                let _ = iface.set_double("text-scaling-factor", scale);
                tracing::info!("text scale set to: {scale}");
            }
            ThemeBuilderMsg::SetFontHinting(hinting) => {
                let iface = gio::Settings::new("org.gnome.desktop.interface");
                let _ = iface.set_string("font-hinting", &hinting);
                tracing::info!("font hinting set to: {hinting}");
            }
            ThemeBuilderMsg::SetCursorSize(size) => {
                let iface = gio::Settings::new("org.gnome.desktop.interface");
                let _ = iface.set_int("cursor-size", size);
                tracing::info!("cursor size set to: {size}");
            }
            ThemeBuilderMsg::SetAnimations(enabled) => {
                let iface = gio::Settings::new("org.gnome.desktop.interface");
                let _ = iface.set_boolean("enable-animations", enabled);
                tracing::info!("animations: {enabled}");
            }
            ThemeBuilderMsg::SetHotCorners(enabled) => {
                let iface = gio::Settings::new("org.gnome.desktop.interface");
                let _ = iface.set_boolean("enable-hot-corners", enabled);
                tracing::info!("hot corners: {enabled}");
            }
            ThemeBuilderMsg::RefreshCompat => {
                if let Ok(spec) = self.build_spec() {
                    let report = theme_capability::compatibility_report(&spec);

                    // Reset all subtitles to originals
                    for (row, original) in self.compat_rows.values() {
                        row.set_subtitle(original);
                    }

                    // Append warnings to the subtitle of affected controls
                    for (control, warnings) in &report {
                        if let Some((row, original)) = self.compat_rows.get(control) {
                            let warning_text = if warnings.len() == 1 {
                                warnings[0].clone()
                            } else {
                                format!(
                                    "{} GNOME versions affected",
                                    warnings.len()
                                )
                            };
                            row.set_subtitle(&format!(
                                "{original}\n<span foreground=\"#e5a50a\">\u{26a0} {warning_text}</span>"
                            ));
                            row.set_tooltip_text(Some(&warnings.join("\n")));
                        }
                    }
                }
            }
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
