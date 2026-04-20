// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Theme Builder — visual customization of GTK4 and GNOME Shell appearance.
//! Generates CSS overrides for window radius, element radius, panel styling,
//! and accent-tinted surfaces (replaces Tint my Shell / Tint my GNOME).

use crate::components::color_picker;
use crate::services::AppServices;
use adw::prelude::*;
use gnomex_app::use_cases::ApplyThemeUseCase;
use gnomex_domain::theme_capability::{self, ThemeControlId};
use gnomex_domain::{
    DashSpec, HexColor, LayerSeparationSpec, Opacity, PanelSpec, Radius, SidebarSpec, ThemeSpec,
    TintSpec,
};
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
    floating_dock: Arc<dyn gnomex_app::ports::FloatingDockController>,
    blur_my_shell: Arc<dyn gnomex_app::ports::BlurMyShellController>,
    detected_version: String,
    wallpaper_swatches: Vec<gtk::Box>, // one per extract row (single, day, night)
    color_pickers: Vec<gtk::Box>,     // picker widgets to deselect on wallpaper pick
    // Raw slider values (validated into ThemeSpec on Apply)
    window_radius: f64,
    element_radius: f64,
    panel_radius: f64,
    panel_opacity: f64,
    panel_tint_hex: String,
    tint_intensity: f64,
    dash_opacity: f64,
    overview_blur: bool,
    // Headerbar / CSD
    headerbar_height: f64,
    headerbar_shadow: f64,
    circular_buttons: bool,
    show_window_shadow: bool,
    inset_border: f64,
    // Visual insets
    card_border_width: f64,
    separator_opacity: f64,
    focus_ring_width: f64,
    combo_inset: bool,
    // Sidebar (Nautilus/Files/Disks left-nav)
    sidebar_opacity: f64,
    sidebar_fg_override: String,
    // Layer separation (headerbar / sidebar / content boundaries)
    layer_headerbar_bottom: f64,
    layer_sidebar_divider: f64,
    layer_content_contrast: f64,
    // Notification
    notification_radius: f64,
    notification_opacity: f64,
    // Scheduled accent
    scheduled_accent_enabled: bool,
    day_accent: String,
    night_accent: String,
    // Accent UI rows for dynamic visibility
    single_color_row: adw::ActionRow,
    single_wallpaper_row: adw::ActionRow,
    day_accent_row: adw::ActionRow,
    day_wp_row: adw::ActionRow,
    night_accent_row: adw::ActionRow,
    night_wp_row: adw::ActionRow,
    sched_info_row: adw::ActionRow,
    // Shared tinting — accent color drives panel/shell colors
    shared_tinting: bool,
    // Panel scheduling
    use_accent_for_panel: bool,
    scheduled_panel_enabled: bool,
    day_panel_tint: String,
    night_panel_tint: String,
    // Widget refs for dynamic visibility
    panel_tint_row: adw::ActionRow,
    panel_use_accent_row: adw::SwitchRow,
    panel_sched_row: adw::SwitchRow,
    day_panel_row: adw::ActionRow,
    night_panel_row: adw::ActionRow,
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
    SetFloatingDock(bool),
    SetSharedTinting(bool),
    RefreshVisibility,
    SetUseAccentForPanel(bool),
    SetScheduledAccent(bool),
    SetDayAccent(String),
    SetNightAccent(String),
    SetScheduledPanel(bool),
    SetDayPanelTint(String),
    SetNightPanelTint(String),
    SetSunriseSunset(bool),
    TickAccentSchedule,
    // Headerbar / CSD
    SetHeaderbarHeight(f64),
    SetHeaderbarShadow(f64),
    SetCircularButtons(bool),
    SetShowWindowShadow(bool),
    SetInsetBorder(f64),
    // Visual Insets
    SetCardBorderWidth(f64),
    SetSeparatorOpacity(f64),
    SetFocusRingWidth(f64),
    SetComboInset(bool),
    // Sidebar
    SetSidebarOpacity(f64),
    SetSidebarFgOverride(String),
    // Layer separation
    SetLayerHeaderbarBottom(f64),
    SetLayerSidebarDivider(f64),
    SetLayerContentContrast(f64),
    // Notifications
    SetNotificationRadius(f64),
    SetNotificationOpacity(f64),
    // Window Behavior
    SetEdgeTiling(bool),
    SetCenterNewWindows(bool),
    SetAttachModals(bool),
    SetAutoMaximize(bool),
    SetDraggableBorderWidth(i32),
    SetDynamicWorkspaces(bool),
    // WM
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
        // All theme builder values now read from GSettings (tb-* keys)

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
        // version_banner appended in final ordering below

        // === Unified Accent Color section ===
        // Dynamically shows single or day/night pickers based on schedule toggle.
        let accent_group = adw::PreferencesGroup::builder()
            .title("Accent Color")
            .description("System highlight color used across all apps")
            .build();

        let iface = gio::Settings::new("org.gnome.desktop.interface");
        let current_accent = iface.string("accent-color").to_string();

        let app_settings = gio::Settings::new("io.github.gnomex.GnomeX");
        let saved_day = app_settings.string("day-accent-color").to_string();
        let saved_night = app_settings.string("night-accent-color").to_string();
        let saved_sched_enabled = app_settings.boolean("scheduled-accent-enabled");
        let initial_day = if saved_day.is_empty() { "blue".into() } else { saved_day };
        let initial_night = if saved_night.is_empty() { "slate".into() } else { saved_night };

        // Schedule toggle
        let sched_switch = adw::SwitchRow::builder()
            .title("Schedule Accent Color")
            .subtitle("Automatically switch between day and night colors")
            .active(saved_sched_enabled)
            .build();
        {
            let sender = sender.clone();
            sched_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetScheduledAccent(row.is_active()));
            });
        }
        accent_group.add(&sched_switch);

        // --- Single color picker (shown when scheduler is OFF) ---
        let single_color_row = adw::ActionRow::builder()
            .title("Color")
            .build();
        let single_picker = {
            let sender = sender.clone();
            color_picker::build_color_picker(
                &color_picker::accent_id_to_hex(&current_accent),
                move |id, _hex| {
                    sender.input(ThemeBuilderMsg::SetAccentColor(id));
                },
            )
        };
        single_color_row.add_suffix(&single_picker);
        accent_group.add(&single_color_row);

        let single_wallpaper_row = adw::ActionRow::builder()
            .title("From Wallpaper")
            .subtitle("Extract accent colors from your desktop wallpaper")
            .build();
        let wallpaper_swatches = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .valign(gtk::Align::Center)
            .build();
        let single_wp_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .valign(gtk::Align::Center)
            .build();
        single_wp_box.append(&wallpaper_swatches);
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
        single_wp_box.append(&extract_btn);
        single_wallpaper_row.add_suffix(&single_wp_box);
        accent_group.add(&single_wallpaper_row);

        // --- Day color picker (shown when scheduler is ON) ---
        let day_accent_row = adw::ActionRow::builder()
            .title("Day Color")
            .subtitle("Active during daytime hours")
            .build();
        let day_picker = {
            let sender = sender.clone();
            color_picker::build_color_picker(
                &color_picker::accent_id_to_hex(&initial_day),
                move |id, _hex| {
                    sender.input(ThemeBuilderMsg::SetDayAccent(id));
                },
            )
        };
        day_accent_row.add_suffix(&day_picker);
        accent_group.add(&day_accent_row);

        let day_wp_row = adw::ActionRow::builder()
            .title("Day \u{2014} From Wallpaper")
            .subtitle("Extract day accent from wallpaper")
            .build();
        let day_wp_swatches = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .valign(gtk::Align::Center)
            .build();
        let day_wp_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .valign(gtk::Align::Center)
            .build();
        day_wp_box.append(&day_wp_swatches);
        let day_extract_btn = gtk::Button::builder()
            .label("Extract")
            .css_classes(["flat"])
            .build();
        {
            let sender = sender.clone();
            day_extract_btn.connect_clicked(move |_| {
                sender.input(ThemeBuilderMsg::ExtractFromWallpaper);
            });
        }
        day_wp_box.append(&day_extract_btn);
        day_wp_row.add_suffix(&day_wp_box);
        accent_group.add(&day_wp_row);

        // --- Night color picker (shown when scheduler is ON) ---
        let night_accent_row = adw::ActionRow::builder()
            .title("Night Color")
            .subtitle("Active during evening/night hours")
            .build();
        let night_picker = {
            let sender = sender.clone();
            color_picker::build_color_picker(
                &color_picker::accent_id_to_hex(&initial_night),
                move |id, _hex| {
                    sender.input(ThemeBuilderMsg::SetNightAccent(id));
                },
            )
        };
        night_accent_row.add_suffix(&night_picker);
        accent_group.add(&night_accent_row);

        let night_wp_row = adw::ActionRow::builder()
            .title("Night \u{2014} From Wallpaper")
            .subtitle("Extract night accent from wallpaper")
            .build();
        let night_wp_swatches = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(4)
            .valign(gtk::Align::Center)
            .build();
        let night_wp_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(8)
            .valign(gtk::Align::Center)
            .build();
        night_wp_box.append(&night_wp_swatches);
        let night_extract_btn = gtk::Button::builder()
            .label("Extract")
            .css_classes(["flat"])
            .build();
        {
            let sender = sender.clone();
            night_extract_btn.connect_clicked(move |_| {
                sender.input(ThemeBuilderMsg::ExtractFromWallpaper);
            });
        }
        night_wp_box.append(&night_extract_btn);
        night_wp_row.add_suffix(&night_wp_box);
        accent_group.add(&night_wp_row);

        // Sunrise/sunset toggle
        let nl_settings = gio::Settings::new("org.gnome.settings-daemon.plugins.color");
        let is_auto = nl_settings.boolean("night-light-schedule-automatic");

        let sunrise_switch = adw::SwitchRow::builder()
            .title("Use Sunrise / Sunset")
            .subtitle("Automatically calculate schedule from your location")
            .active(is_auto)
            .build();
        {
            let sender = sender.clone();
            sunrise_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetSunriseSunset(row.is_active()));
            });
        }
        accent_group.add(&sunrise_switch);

        // Schedule info
        let from_h = nl_settings.double("night-light-schedule-from");
        let to_h = nl_settings.double("night-light-schedule-to");
        let schedule_label = if is_auto {
            format!(
                "Sunset: ~{}:00 \u{2192} Sunrise: ~{}:00 (location-based)",
                from_h as u32,
                to_h as u32,
            )
        } else {
            format!(
                "Night: {}:00 \u{2192} {}:00 (fixed schedule)",
                from_h as u32,
                to_h as u32,
            )
        };
        let sched_info = adw::ActionRow::builder()
            .title("Schedule")
            .subtitle(&schedule_label)
            .build();
        sched_info.add_prefix(&gtk::Image::from_icon_name("preferences-system-time-symbolic"));
        accent_group.add(&sched_info);

        // appended in final ordering

        // === Radii section ===
        let radii_group = adw::PreferencesGroup::builder()
            .title("Border Radius")
            .description("Control the roundness of windows and UI elements")
            .build();

        let window_radius = build_spin_row(
            "Window Corners",
            "Corner radius for application windows",
            0.0, 48.0, app_settings.double("tb-window-radius"), 1.0,
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
            0.0, 24.0, app_settings.double("tb-element-radius"), 1.0,
        );
        {
            let sender = sender.clone();
            element_radius.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetElementRadius(row.value()));
            });
        }
        radii_group.add(&element_radius);

        // appended in final ordering

        // === Panel section (Shell tinting) ===
        let panel_group = adw::PreferencesGroup::builder()
            .title("Shell Panel")
            .description("Customize the top panel appearance")
            .build();

        let panel_radius = build_spin_row(
            "Panel Corner Radius",
            "Roundness of the panel corners",
            0.0, 24.0, app_settings.double("tb-panel-radius"), 1.0,
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
            0.0, 100.0, app_settings.double("tb-panel-opacity"), 5.0,
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
        if let Ok(rgba) = gtk::gdk::RGBA::parse(&app_settings.string("tb-panel-tint").to_string()) {
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

        // "Use accent color as panel tint" toggle
        let use_accent_switch = adw::SwitchRow::builder()
            .title("Use Accent Color as Panel Tint")
            .subtitle("Automatically derive the panel color from your accent color")
            .active(false)
            .build();
        {
            let sender = sender.clone();
            use_accent_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetUseAccentForPanel(row.is_active()));
            });
        }
        panel_group.add(&use_accent_switch);

        // Panel color scheduling (day/night panel tint)
        let panel_sched_switch = adw::SwitchRow::builder()
            .title("Schedule Panel Color")
            .subtitle("Switch panel tint between day and night automatically")
            .active(app_settings.boolean("scheduled-panel-enabled"))
            .build();
        {
            let sender = sender.clone();
            panel_sched_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetScheduledPanel(row.is_active()));
            });
        }
        panel_group.add(&panel_sched_switch);

        let day_panel_row = adw::ActionRow::builder()
            .title("Day Panel Tint")
            .subtitle("Panel color during daytime")
            .build();
        let day_panel_btn = gtk::ColorDialogButton::builder()
            .valign(gtk::Align::Center)
            .build();
        let day_panel_dialog = gtk::ColorDialog::builder()
            .title("Day Panel Tint")
            .with_alpha(false)
            .build();
        day_panel_btn.set_dialog(&day_panel_dialog);
        let saved_day_panel = app_settings.string("day-panel-tint").to_string();
        if !saved_day_panel.is_empty() {
            if let Ok(rgba) = gtk::gdk::RGBA::parse(&saved_day_panel) {
                day_panel_btn.set_rgba(&rgba);
            }
        }
        {
            let sender = sender.clone();
            day_panel_btn.connect_rgba_notify(move |btn| {
                let rgba = btn.rgba();
                let hex = format!(
                    "#{:02x}{:02x}{:02x}",
                    (rgba.red() * 255.0) as u8,
                    (rgba.green() * 255.0) as u8,
                    (rgba.blue() * 255.0) as u8,
                );
                sender.input(ThemeBuilderMsg::SetDayPanelTint(hex));
            });
        }
        day_panel_row.add_suffix(&day_panel_btn);
        panel_group.add(&day_panel_row);

        let night_panel_row = adw::ActionRow::builder()
            .title("Night Panel Tint")
            .subtitle("Panel color during nighttime")
            .build();
        let night_panel_btn = gtk::ColorDialogButton::builder()
            .valign(gtk::Align::Center)
            .build();
        let night_panel_dialog = gtk::ColorDialog::builder()
            .title("Night Panel Tint")
            .with_alpha(false)
            .build();
        night_panel_btn.set_dialog(&night_panel_dialog);
        let saved_night_panel = app_settings.string("night-panel-tint").to_string();
        if !saved_night_panel.is_empty() {
            if let Ok(rgba) = gtk::gdk::RGBA::parse(&saved_night_panel) {
                night_panel_btn.set_rgba(&rgba);
            }
        }
        {
            let sender = sender.clone();
            night_panel_btn.connect_rgba_notify(move |btn| {
                let rgba = btn.rgba();
                let hex = format!(
                    "#{:02x}{:02x}{:02x}",
                    (rgba.red() * 255.0) as u8,
                    (rgba.green() * 255.0) as u8,
                    (rgba.blue() * 255.0) as u8,
                );
                sender.input(ThemeBuilderMsg::SetNightPanelTint(hex));
            });
        }
        night_panel_row.add_suffix(&night_panel_btn);
        panel_group.add(&night_panel_row);

        // === Accent Tinting (Tint my GNOME-style) ===
        let tint_group = adw::PreferencesGroup::builder()
            .title("Accent Tinting")
            .description("Blend the accent color into window backgrounds")
            .build();

        let tint_intensity = build_spin_row(
            "Tint Intensity",
            "How much accent color bleeds into surfaces (0\u{2013}20%)",
            0.0, 20.0, app_settings.double("tb-tint-intensity"), 1.0,
        );
        {
            let sender = sender.clone();
            tint_intensity.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetTintIntensity(row.value()));
            });
        }
        tint_group.add(&tint_intensity);

        // === Shared Tinting toggle ===
        let shared_group = adw::PreferencesGroup::builder()
            .title("Shared Tinting")
            .description("Use the accent color tint for the shell panel and dash as well")
            .build();

        let shared_tinting_switch = adw::SwitchRow::builder()
            .title("Apply Accent Tint to Shell")
            .subtitle("When enabled, the panel and dash inherit the accent tint \u{2014} independent shell color pickers are hidden")
            .active(app_settings.boolean("shared-tinting-enabled"))
            .build();
        {
            let sender = sender.clone();
            shared_tinting_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetSharedTinting(row.is_active()));
            });
        }
        shared_group.add(&shared_tinting_switch);

        // appended in final ordering

        // === Dash / Overview ===
        let dash_group = adw::PreferencesGroup::builder()
            .title("Dash &amp; Overview")
            .description("Customize the app launcher and activities view")
            .build();

        let dash_opacity = build_spin_row(
            "Dash Opacity",
            "Transparency of the dash background",
            0.0, 100.0, app_settings.double("tb-dash-opacity"), 5.0,
        );
        {
            let sender = sender.clone();
            dash_opacity.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetDashOpacity(row.value()));
            });
        }
        dash_group.add(&dash_opacity);

        let bms_available = gio::SettingsSchemaSource::default()
            .and_then(|src| src.lookup("org.gnome.shell.extensions.blur-my-shell.overview", true))
            .is_some();
        let blur_subtitle = if bms_available {
            "Blur the wallpaper and dim the overview"
        } else {
            "Dim the overview — install Blur My Shell for real wallpaper blur"
        };
        let blur_switch = adw::SwitchRow::builder()
            .title("Overview Background Blur")
            .subtitle(blur_subtitle)
            .active(app_settings.boolean("tb-overview-blur"))
            .build();
        {
            let sender = sender.clone();
            blur_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetOverviewBlur(row.is_active()));
            });
        }
        dash_group.add(&blur_switch);

        // === Floating Dock — requires the Dash to Dock extension ===
        let dtd_available = gio::SettingsSchemaSource::default()
            .and_then(|src| src.lookup("org.gnome.shell.extensions.dash-to-dock", true))
            .is_some();
        let floating_subtitle = if dtd_available {
            "Turn the dash into an always-visible floating dock"
        } else {
            "Requires the Dash to Dock extension — install it first"
        };
        let floating_dock = adw::SwitchRow::builder()
            .title("Floating Dock")
            .subtitle(floating_subtitle)
            .active(app_settings.boolean("tb-floating-dock-enabled"))
            .sensitive(dtd_available)
            .build();
        {
            let sender = sender.clone();
            floating_dock.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetFloatingDock(row.is_active()));
            });
        }
        dash_group.add(&floating_dock);

        // appended in final ordering

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
        // appended in final ordering

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
        // appended in final ordering

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
        // appended in final ordering

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
        // appended in final ordering

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

        // appended in final ordering

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

        // appended in final ordering

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

        // appended in final ordering

        // === Notifications ===
        let notif_group = adw::PreferencesGroup::builder()
            .title("Notifications &amp; Calendar")
            .description("Shell notification banner and calendar styling")
            .build();

        let notif_radius = build_spin_row(
            "Notification Radius",
            "Corner radius of notification banners",
            0.0, 24.0, 12.0, 1.0,
        );
        {
            let sender = sender.clone();
            notif_radius.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetNotificationRadius(row.value()));
            });
        }
        notif_group.add(&notif_radius);

        let notif_opacity = build_spin_row(
            "Notification Opacity",
            "Background opacity of notification banners (0\u{2013}100%)",
            0.0, 100.0, 95.0, 5.0,
        );
        {
            let sender = sender.clone();
            notif_opacity.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetNotificationOpacity(row.value() / 100.0));
            });
        }
        notif_group.add(&notif_opacity);

        // appended in final ordering

        // === Headerbar & CSD ===
        let csd_group = adw::PreferencesGroup::builder()
            .title("Headerbar &amp; CSD")
            .description("Client-side decoration and titlebar styling")
            .build();

        let hb_height = build_spin_row(
            "Headerbar Height",
            "Minimum height of the titlebar (default 47, compact ~30)",
            24.0, 60.0, 47.0, 1.0,
        );
        {
            let sender = sender.clone();
            hb_height.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetHeaderbarHeight(row.value()));
            });
        }
        csd_group.add(&hb_height);

        let circular_btn_switch = adw::SwitchRow::builder()
            .title("Circular Window Controls")
            .subtitle("Make close/minimize/maximize buttons round like macOS")
            .active(app_settings.boolean("tb-circular-buttons"))
            .build();
        {
            let sender = sender.clone();
            circular_btn_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetCircularButtons(row.is_active()));
            });
        }
        csd_group.add(&circular_btn_switch);

        let shadow_switch = adw::SwitchRow::builder()
            .title("Window Drop Shadow")
            .subtitle("Show the soft shadow around CSD windows")
            .active(app_settings.boolean("tb-show-window-shadow"))
            .build();
        {
            let sender = sender.clone();
            shadow_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetShowWindowShadow(row.is_active()));
            });
        }
        csd_group.add(&shadow_switch);

        let inset_border_row = build_spin_row(
            "Window Inset Border",
            "Visible border inside the window frame (0 = none)",
            0.0, 4.0, 0.0, 1.0,
        );
        {
            let sender = sender.clone();
            inset_border_row.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetInsetBorder(row.value()));
            });
        }
        csd_group.add(&inset_border_row);

        // appended in final ordering

        // === Visual Insets & Borders ===
        let inset_group = adw::PreferencesGroup::builder()
            .title("Visual Insets &amp; Borders")
            .description("Fine-tune borders, shadows, and focus indicators")
            .build();

        let card_border = build_spin_row(
            "Card Border Width",
            "Visible border around cards and list rows (0 = none)",
            0.0, 4.0, 1.0, 0.5,
        );
        {
            let sender = sender.clone();
            card_border.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetCardBorderWidth(row.value()));
            });
        }
        inset_group.add(&card_border);

        let hbar_shadow = build_spin_row(
            "Headerbar Shadow",
            "Drop shadow intensity below the headerbar (0 = flat)",
            0.0, 1.0, 1.0, 0.1,
        );
        {
            let sender = sender.clone();
            hbar_shadow.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetHeaderbarShadow(row.value()));
            });
        }
        inset_group.add(&hbar_shadow);

        let separator_opacity = build_spin_row(
            "Separator Opacity",
            "Visibility of divider lines between list rows (0\u{2013}100%)",
            0.0, 100.0, 100.0, 10.0,
        );
        {
            let sender = sender.clone();
            separator_opacity.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetSeparatorOpacity(row.value()));
            });
        }
        inset_group.add(&separator_opacity);

        let focus_ring = build_spin_row(
            "Focus Ring Width",
            "Keyboard focus outline thickness",
            0.0, 4.0, 2.0, 0.5,
        );
        {
            let sender = sender.clone();
            focus_ring.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetFocusRingWidth(row.value()));
            });
        }
        inset_group.add(&focus_ring);

        let combo_inset_switch = adw::SwitchRow::builder()
            .title("Combo Button Inset")
            .subtitle("Show a visible inset/border on dropdown combo buttons")
            .active(true)
            .build();
        {
            let sender = sender.clone();
            combo_inset_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetComboInset(row.is_active()));
            });
        }
        inset_group.add(&combo_inset_switch);

        // === Sidebar (Nautilus / Files / Disks left-nav) ===
        let sidebar_group = adw::PreferencesGroup::builder()
            .title("Sidebar")
            .description(
                "Controls the left navigation sidebar rendered by Nautilus, \
                 Files, Disks, Settings, and any AdwOverlaySplitView app.",
            )
            .build();

        let sidebar_opacity_row = build_spin_row(
            "Sidebar Opacity",
            "Background opacity of the left-nav sidebar (100% = opaque)",
            0.0,
            100.0,
            app_settings.double("tb-sidebar-opacity") * 100.0,
            5.0,
        );
        {
            let sender = sender.clone();
            sidebar_opacity_row.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetSidebarOpacity(row.value() / 100.0));
            });
        }
        sidebar_group.add(&sidebar_opacity_row);

        let sidebar_fg_row = adw::ActionRow::builder()
            .title("Sidebar Text Color")
            .subtitle("Override the sidebar foreground colour. Leave default to follow the active colour scheme.")
            .build();
        let sidebar_fg_btn = gtk::ColorDialogButton::builder().build();
        let sidebar_fg_dialog = gtk::ColorDialog::builder()
            .title("Sidebar Text Color")
            .with_alpha(false)
            .build();
        sidebar_fg_btn.set_dialog(&sidebar_fg_dialog);
        let saved_sidebar_fg = app_settings.string("tb-sidebar-fg-override").to_string();
        if let Ok(rgba) = gtk::gdk::RGBA::parse(&saved_sidebar_fg) {
            sidebar_fg_btn.set_rgba(&rgba);
        }
        let sidebar_fg_reset = gtk::Button::builder()
            .icon_name("edit-clear-symbolic")
            .valign(gtk::Align::Center)
            .css_classes(["flat"])
            .tooltip_text("Reset to default (follow colour scheme)")
            .build();
        {
            let sender = sender.clone();
            sidebar_fg_btn.connect_rgba_notify(move |btn| {
                let rgba = btn.rgba();
                let hex = format!(
                    "#{:02x}{:02x}{:02x}",
                    (rgba.red() * 255.0) as u8,
                    (rgba.green() * 255.0) as u8,
                    (rgba.blue() * 255.0) as u8,
                );
                sender.input(ThemeBuilderMsg::SetSidebarFgOverride(hex));
            });
        }
        {
            let sender = sender.clone();
            sidebar_fg_reset.connect_clicked(move |_| {
                sender.input(ThemeBuilderMsg::SetSidebarFgOverride(String::new()));
            });
        }
        sidebar_fg_row.add_suffix(&sidebar_fg_btn);
        sidebar_fg_row.add_suffix(&sidebar_fg_reset);
        sidebar_group.add(&sidebar_fg_row);

        // === Layer Separation (headerbar / sidebar / content) ===
        let layer_group = adw::PreferencesGroup::builder()
            .title("Layer Separation")
            .description(
                "Make the boundaries between headerbar, sidebar, and content \
                 visible. Libadwaita defaults blend all three; dial these up \
                 if you prefer a more traditional desktop silhouette.",
            )
            .build();

        let layer_hb_row = build_spin_row(
            "Headerbar Bottom Border",
            "Line under the headerbar separating it from content (0 = flush)",
            0.0,
            4.0,
            app_settings.double("tb-layer-headerbar-bottom"),
            1.0,
        );
        {
            let sender = sender.clone();
            layer_hb_row.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetLayerHeaderbarBottom(row.value()));
            });
        }
        layer_group.add(&layer_hb_row);

        let layer_sb_row = build_spin_row(
            "Sidebar Divider",
            "Vertical rule between sidebar and content column (0 = blended)",
            0.0,
            4.0,
            app_settings.double("tb-layer-sidebar-divider"),
            1.0,
        );
        {
            let sender = sender.clone();
            layer_sb_row.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetLayerSidebarDivider(row.value()));
            });
        }
        layer_group.add(&layer_sb_row);

        let layer_cc_row = build_spin_row(
            "Content Contrast",
            "Extra contrast applied to the content backdrop (0\u{2013}100%)",
            0.0,
            100.0,
            app_settings.double("tb-layer-content-contrast") * 100.0,
            5.0,
        );
        {
            let sender = sender.clone();
            layer_cc_row.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetLayerContentContrast(row.value() / 100.0));
            });
        }
        layer_group.add(&layer_cc_row);

        // appended in final ordering

        // === Window Behavior (Mutter) ===
        let window_group = adw::PreferencesGroup::builder()
            .title("Window Behavior")
            .description("Window management and client-side decorations")
            .build();

        let mutter = gio::Settings::new("org.gnome.mutter");

        let edge_tiling_switch = adw::SwitchRow::builder()
            .title("Edge Tiling")
            .subtitle("Tile windows by dragging them to screen edges")
            .active(mutter.boolean("edge-tiling"))
            .build();
        {
            let sender = sender.clone();
            edge_tiling_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetEdgeTiling(row.is_active()));
            });
        }
        window_group.add(&edge_tiling_switch);

        let center_switch = adw::SwitchRow::builder()
            .title("Center New Windows")
            .subtitle("Open new windows in the center of the screen")
            .active(mutter.boolean("center-new-windows"))
            .build();
        {
            let sender = sender.clone();
            center_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetCenterNewWindows(row.is_active()));
            });
        }
        window_group.add(&center_switch);

        let attach_switch = adw::SwitchRow::builder()
            .title("Attach Modal Dialogs")
            .subtitle("Attach dialog windows to their parent window")
            .active(mutter.boolean("attach-modal-dialogs"))
            .build();
        {
            let sender = sender.clone();
            attach_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetAttachModals(row.is_active()));
            });
        }
        window_group.add(&attach_switch);

        let auto_max_switch = adw::SwitchRow::builder()
            .title("Auto Maximize")
            .subtitle("Automatically maximize windows that are nearly full-screen")
            .active(mutter.boolean("auto-maximize"))
            .build();
        {
            let sender = sender.clone();
            auto_max_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetAutoMaximize(row.is_active()));
            });
        }
        window_group.add(&auto_max_switch);

        let dynamic_ws_switch = adw::SwitchRow::builder()
            .title("Dynamic Workspaces")
            .subtitle("Automatically add and remove workspaces as needed")
            .active(mutter.boolean("dynamic-workspaces"))
            .build();
        {
            let sender = sender.clone();
            dynamic_ws_switch.connect_active_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetDynamicWorkspaces(row.is_active()));
            });
        }
        window_group.add(&dynamic_ws_switch);

        let current_border = mutter.int("draggable-border-width") as f64;
        let border_width = build_spin_row(
            "Draggable Border Width",
            "Invisible resize handle width at window edges (px)",
            1.0, 48.0, current_border, 1.0,
        );
        {
            let sender = sender.clone();
            border_width.connect_value_notify(move |row| {
                sender.input(ThemeBuilderMsg::SetDraggableBorderWidth(row.value() as i32));
            });
        }
        window_group.add(&border_width);

        // appended in final ordering

        // === Action buttons (will be placed in bottom bar) ===

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
        // reset_btn placed in bottom_bar below

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
        // apply_btn placed in bottom_bar below

        // === Category-based layout with sidebar ===
        // Build per-category content pages
        fn build_category_page(groups: &[&impl IsA<gtk::Widget>]) -> gtk::ScrolledWindow {
            let scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
            let clamp = adw::Clamp::builder()
                .maximum_size(600)
                .margin_top(24)
                .margin_bottom(24)
                .margin_start(24)
                .margin_end(24)
                .build();
            let page_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(24)
                .build();
            for group in groups {
                page_box.append(*group);
            }
            clamp.set_child(Some(&page_box));
            scroll.set_child(Some(&clamp));
            scroll
        }

        let color_page = build_category_page(
            &[&accent_group, &tint_group, &shared_group, &panel_group, &dash_group, &notif_group, &scheme_group],
        );
        let windows_page = build_category_page(
            &[&radii_group, &csd_group, &inset_group, &sidebar_group, &layer_group, &window_group, &wm_group],
        );
        let typography_page = build_category_page(
            &[&font_group, &cursor_group],
        );
        let behavior_page = build_category_page(
            &[&behavior_group, &nl_group],
        );
        let sound_page = build_category_page(
            &[&sound_group],
        );

        // Stack for category content
        let category_stack = gtk::Stack::new();
        category_stack.set_transition_type(gtk::StackTransitionType::Crossfade);
        category_stack.add_named(&color_page, Some("color"));
        category_stack.add_named(&windows_page, Some("windows"));
        category_stack.add_named(&typography_page, Some("typography"));
        category_stack.add_named(&behavior_page, Some("behavior"));
        category_stack.add_named(&sound_page, Some("sound"));

        // Sidebar
        let sidebar_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::Single)
            .css_classes(["navigation-sidebar"])
            .build();

        let categories = [
            ("color", "Color", "applications-graphics-symbolic"),
            ("windows", "Windows", "preferences-desktop-display-symbolic"),
            ("typography", "Typography", "font-x-generic-symbolic"),
            ("behavior", "Behavior", "preferences-other-symbolic"),
            ("sound", "Sound", "audio-speakers-symbolic"),
        ];

        for (id, label, icon) in &categories {
            let row = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(12)
                .margin_top(8)
                .margin_bottom(8)
                .margin_start(12)
                .margin_end(12)
                .build();
            row.append(&gtk::Image::from_icon_name(icon));
            row.append(&gtk::Label::new(Some(label)));

            let list_row = gtk::ListBoxRow::builder()
                .child(&row)
                .name(*id)
                .build();
            sidebar_list.append(&list_row);
        }

        // Select first row
        if let Some(first) = sidebar_list.row_at_index(0) {
            sidebar_list.select_row(Some(&first));
        }

        {
            let stack = category_stack.clone();
            sidebar_list.connect_row_selected(move |_, row| {
                if let Some(row) = row {
                    let name = row.widget_name();
                    if !name.is_empty() {
                        stack.set_visible_child_name(&name);
                    }
                }
            });
        }

        // Compose the split layout
        let sidebar_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .width_request(180)
            .build();
        sidebar_box.append(&version_banner);
        sidebar_box.append(&sidebar_list);

        category_stack.set_hexpand(true);

        let split = gtk::Paned::builder()
            .orientation(gtk::Orientation::Horizontal)
            .vexpand(true)
            .start_child(&sidebar_box)
            .end_child(&category_stack)
            .position(180)
            .shrink_start_child(false)
            .shrink_end_child(false)
            .build();

        root.append(&split);

        // Persistent bottom action bar — visible across all categories
        let bottom_bar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .halign(gtk::Align::End)
            .spacing(12)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(24)
            .margin_end(24)
            .css_classes(["toolbar"])
            .build();
        bottom_bar.append(&reset_btn);
        bottom_bar.append(&apply_btn);
        root.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
        root.append(&bottom_bar);

        let model = ThemeBuilderModel {
            apply_uc: services.apply_theme,
            floating_dock: services.floating_dock,
            blur_my_shell: services.blur_my_shell,
            detected_version: services.detected_gnome_version,
            wallpaper_swatches: vec![
                wallpaper_swatches.clone(),
                day_wp_swatches.clone(),
                night_wp_swatches.clone(),
            ],
            color_pickers: vec![
                single_picker.clone(),
                day_picker.clone(),
                night_picker.clone(),
            ],
            window_radius: app_settings.double("tb-window-radius"),
            element_radius: app_settings.double("tb-element-radius"),
            panel_radius: app_settings.double("tb-panel-radius"),
            panel_opacity: app_settings.double("tb-panel-opacity"),
            panel_tint_hex: app_settings.string("tb-panel-tint").to_string(),
            tint_intensity: app_settings.double("tb-tint-intensity"),
            dash_opacity: app_settings.double("tb-dash-opacity"),
            overview_blur: app_settings.boolean("tb-overview-blur"),
            headerbar_height: app_settings.double("tb-headerbar-height"),
            headerbar_shadow: app_settings.double("tb-headerbar-shadow"),
            circular_buttons: app_settings.boolean("tb-circular-buttons"),
            show_window_shadow: app_settings.boolean("tb-show-window-shadow"),
            inset_border: app_settings.double("tb-inset-border"),
            card_border_width: app_settings.double("tb-card-border-width"),
            separator_opacity: app_settings.double("tb-separator-opacity"),
            focus_ring_width: app_settings.double("tb-focus-ring-width"),
            combo_inset: app_settings.boolean("tb-combo-inset"),
            sidebar_opacity: app_settings.double("tb-sidebar-opacity"),
            sidebar_fg_override: app_settings.string("tb-sidebar-fg-override").to_string(),
            layer_headerbar_bottom: app_settings.double("tb-layer-headerbar-bottom"),
            layer_sidebar_divider: app_settings.double("tb-layer-sidebar-divider"),
            layer_content_contrast: app_settings.double("tb-layer-content-contrast"),
            notification_radius: app_settings.double("tb-notification-radius"),
            notification_opacity: app_settings.double("tb-notification-opacity"),
            scheduled_accent_enabled: saved_sched_enabled,
            day_accent: initial_day,
            night_accent: initial_night,
            single_color_row: single_color_row.clone(),
            single_wallpaper_row: single_wallpaper_row.clone(),
            day_accent_row: day_accent_row.clone(),
            day_wp_row: day_wp_row.clone(),
            night_accent_row: night_accent_row.clone(),
            night_wp_row: night_wp_row.clone(),
            sched_info_row: sched_info.clone(),
            shared_tinting: app_settings.boolean("shared-tinting-enabled"),
            use_accent_for_panel: app_settings.boolean("shared-tinting-enabled"),
            scheduled_panel_enabled: app_settings.boolean("scheduled-panel-enabled"),
            day_panel_tint: if saved_day_panel.is_empty() { "#1a1a1e".into() } else { saved_day_panel },
            night_panel_tint: if saved_night_panel.is_empty() { "#1a1a1e".into() } else { saved_night_panel },
            panel_tint_row: tint_row.clone(),
            panel_use_accent_row: use_accent_switch.clone(),
            panel_sched_row: panel_sched_switch.clone(),
            day_panel_row: day_panel_row.clone(),
            night_panel_row: night_panel_row.clone(),
            compat_rows: HashMap::from([
                (ThemeControlId::WindowRadius, (window_radius, "Corner radius for application windows".into())),
                (ThemeControlId::ElementRadius, (element_radius, "Corner radius for buttons, entries, and cards".into())),
                (ThemeControlId::PanelRadius, (panel_radius, "Roundness of the panel corners".into())),
                (ThemeControlId::AccentTint, (tint_intensity, "How much accent color bleeds into surfaces (0\u{2013}20%)".into())),
                (ThemeControlId::DashOpacity, (dash_opacity, "Transparency of the dash background".into())),
            ]),
        };
        let widgets = view_output!();

        // Initial state
        sender.input(ThemeBuilderMsg::RefreshVisibility);
        sender.input(ThemeBuilderMsg::RefreshCompat);

        // Apply scheduled accent immediately on startup
        sender.input(ThemeBuilderMsg::TickAccentSchedule);

        // Check accent schedule every 60 seconds
        {
            let sender = sender.clone();
            glib::timeout_add_seconds_local(60, move || {
                sender.input(ThemeBuilderMsg::TickAccentSchedule);
                glib::ControlFlow::Continue
            });
        }

        // --- Cached wallpaper palette ------------------------------------
        // Paint the last-known swatches right away so the user doesn't
        // have to click Extract every time the panel opens. Then watch
        // the GSetting so when the daemon re-extracts on wallpaper
        // change, the swatches update live.
        let cached = load_cached_palette(&app_settings);
        if !cached.is_empty() {
            sender.input(ThemeBuilderMsg::WallpaperColorsReady(cached));
        }
        {
            let sender = sender.clone();
            app_settings
                .clone()
                .connect_changed(Some("cached-wallpaper-palette"), move |s, _| {
                    let palette = load_cached_palette(s);
                    if !palette.is_empty() {
                        sender.input(ThemeBuilderMsg::WallpaperColorsReady(palette));
                    }
                });
        }

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
                // Clear all swatch containers
                for container in &self.wallpaper_swatches {
                    while let Some(child) = container.first_child() {
                        container.remove(&child);
                    }
                }

                // Register CSS for each color once
                for (hex, _) in &colors {
                    let css_class = format!(
                        "wallpaper-swatch-{}",
                        hex.trim_start_matches('#')
                    );
                    let css_provider = gtk::CssProvider::new();
                    css_provider.load_from_string(&format!(
                        // Match widget request (28x28) and CSS
                        // minimums so a squeezing parent can't
                        // produce a 24x28 ellipse — see bug #1.
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
                }

                // Populate each swatch container with toggle buttons for visual feedback
                let pickers = self.color_pickers.clone();
                for container in &self.wallpaper_swatches {
                    for (i, (hex, accent_id)) in colors.iter().enumerate() {
                        let btn = gtk::ToggleButton::builder()
                            .width_request(28)
                            .height_request(28)
                            .halign(gtk::Align::Center)
                            .valign(gtk::Align::Center)
                            .hexpand(false)
                            .vexpand(false)
                            .tooltip_text(&format!("{hex} \u{2192} {accent_id}"))
                            .build();
                        btn.add_css_class(&format!(
                            "wallpaper-swatch-{}",
                            hex.trim_start_matches('#')
                        ));

                        // Link wallpaper swatches in a radio group within each container
                        if i > 0 {
                            if let Some(first) = container.first_child() {
                                btn.set_group(first.downcast_ref::<gtk::ToggleButton>());
                            }
                        }

                        let sender = sender.clone();
                        let aid = accent_id.clone();
                        let pickers = pickers.clone();
                        btn.connect_toggled(move |b| {
                            if b.is_active() {
                                // Deselect all color picker toggles
                                for picker in &pickers {
                                    color_picker::deselect_all(picker);
                                }
                                sender.input(ThemeBuilderMsg::SetAccentColor(aid.clone()));
                            }
                        });

                        container.append(&btn);
                    }
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
            ThemeBuilderMsg::SetOverviewBlur(v) => {
                self.overview_blur = v;
                // Persist immediately so the value round-trips through
                // Experience Packs even before the user hits Apply. The
                // GNOME X Integration extension listens to this key and
                // drives the native blur; the BMS fallback below only
                // kicks in when that extension isn't enabled.
                let app = gio::Settings::new("io.github.gnomex.GnomeX");
                let _ = app.set_boolean("tb-overview-blur", v);
                if !gnomex_extension_enabled() && self.blur_my_shell.is_available() {
                    if let Err(e) = self.blur_my_shell.apply(v) {
                        tracing::warn!("blur my shell apply failed: {e}");
                    }
                }
            }
            ThemeBuilderMsg::SetFloatingDock(enabled) => {
                // Persist the toggle even if neither our extension nor
                // Dash to Dock is present — the value round-trips through
                // Experience Packs and reactivates once one is available.
                let app = gio::Settings::new("io.github.gnomex.GnomeX");
                let _ = app.set_boolean("tb-floating-dock-enabled", enabled);
                if !gnomex_extension_enabled() {
                    if let Err(e) = self.floating_dock.apply(enabled) {
                        tracing::warn!("floating dock apply failed: {e}");
                    } else {
                        tracing::info!(
                            "floating dock: {}",
                            if enabled { "enabled" } else { "disabled" }
                        );
                    }
                }
            }
            ThemeBuilderMsg::SetSharedTinting(enabled) => {
                self.shared_tinting = enabled;
                let app = gio::Settings::new("io.github.gnomex.GnomeX");
                let _ = app.set_boolean("shared-tinting-enabled", enabled);
                if enabled {
                    let accent_hex = current_accent_hex();
                    self.panel_tint_hex = accent_hex;
                }
                sender.input(ThemeBuilderMsg::RefreshVisibility);
            }
            ThemeBuilderMsg::RefreshVisibility => {
                // Accent color: single vs day/night based on scheduler
                let sched_on = self.scheduled_accent_enabled;
                self.single_color_row.set_visible(!sched_on);
                self.single_wallpaper_row.set_visible(!sched_on);
                self.day_accent_row.set_visible(sched_on);
                self.day_wp_row.set_visible(sched_on);
                self.night_accent_row.set_visible(sched_on);
                self.night_wp_row.set_visible(sched_on);
                self.sched_info_row.set_visible(sched_on);

                // When shared tinting is on, hide independent panel color controls
                let hide_panel_colors = self.shared_tinting;
                self.panel_tint_row.set_visible(!hide_panel_colors);
                self.panel_use_accent_row.set_visible(!hide_panel_colors);
                self.panel_sched_row.set_visible(!hide_panel_colors);
                self.day_panel_row.set_visible(!hide_panel_colors && self.scheduled_panel_enabled);
                self.night_panel_row.set_visible(!hide_panel_colors && self.scheduled_panel_enabled);
            }
            ThemeBuilderMsg::SetUseAccentForPanel(enabled) => {
                self.use_accent_for_panel = enabled;
                if enabled {
                    // Derive panel tint from current accent color
                    let accent_hex = current_accent_hex();
                    self.panel_tint_hex = accent_hex.clone();
                    sender.input(ThemeBuilderMsg::SetPanelTint(accent_hex));
                }
            }
            ThemeBuilderMsg::SetScheduledPanel(enabled) => {
                self.scheduled_panel_enabled = enabled;
                let app = gio::Settings::new("io.github.gnomex.GnomeX");
                let _ = app.set_boolean("scheduled-panel-enabled", enabled);
                sender.input(ThemeBuilderMsg::RefreshVisibility);
                if enabled {
                    sender.input(ThemeBuilderMsg::TickAccentSchedule);
                }
            }
            ThemeBuilderMsg::SetDayPanelTint(hex) => {
                self.day_panel_tint = hex.clone();
                let app = gio::Settings::new("io.github.gnomex.GnomeX");
                let _ = app.set_string("day-panel-tint", &hex);
                if self.scheduled_panel_enabled {
                    sender.input(ThemeBuilderMsg::TickAccentSchedule);
                }
            }
            ThemeBuilderMsg::SetNightPanelTint(hex) => {
                self.night_panel_tint = hex.clone();
                let app = gio::Settings::new("io.github.gnomex.GnomeX");
                let _ = app.set_string("night-panel-tint", &hex);
                if self.scheduled_panel_enabled {
                    sender.input(ThemeBuilderMsg::TickAccentSchedule);
                }
            }
            ThemeBuilderMsg::SetScheduledAccent(enabled) => {
                self.scheduled_accent_enabled = enabled;
                let app = gio::Settings::new("io.github.gnomex.GnomeX");
                let _ = app.set_boolean("scheduled-accent-enabled", enabled);
                sender.input(ThemeBuilderMsg::RefreshVisibility);
                if enabled {
                    sender.input(ThemeBuilderMsg::TickAccentSchedule);
                }
                tracing::info!("scheduled accent: {enabled}");
            }
            ThemeBuilderMsg::SetDayAccent(id) => {
                self.day_accent = id.clone();
                let app = gio::Settings::new("io.github.gnomex.GnomeX");
                let _ = app.set_string("day-accent-color", &id);
                if self.scheduled_accent_enabled {
                    sender.input(ThemeBuilderMsg::TickAccentSchedule);
                }
            }
            ThemeBuilderMsg::SetNightAccent(id) => {
                self.night_accent = id.clone();
                let app = gio::Settings::new("io.github.gnomex.GnomeX");
                let _ = app.set_string("night-accent-color", &id);
                if self.scheduled_accent_enabled {
                    sender.input(ThemeBuilderMsg::TickAccentSchedule);
                }
            }
            ThemeBuilderMsg::SetSunriseSunset(enabled) => {
                // This sets the Night Light schedule mode, which our tick handler
                // reads from. When automatic, gnome-settings-daemon computes
                // sunrise/sunset from the device's geolocation.
                let nl = gio::Settings::new("org.gnome.settings-daemon.plugins.color");
                let _ = nl.set_boolean("night-light-schedule-automatic", enabled);
                let app = gio::Settings::new("io.github.gnomex.GnomeX");
                let _ = app.set_boolean("use-sunrise-sunset", enabled);
                tracing::info!("sunrise/sunset scheduling: {enabled}");
            }
            ThemeBuilderMsg::TickAccentSchedule => {
                let nl = gio::Settings::new("org.gnome.settings-daemon.plugins.color");
                let from_h = nl.double("night-light-schedule-from");
                let to_h = nl.double("night-light-schedule-to");

                let now = {
                    let dt = glib::DateTime::now_local().unwrap();
                    dt.hour() as f64 + dt.minute() as f64 / 60.0
                };

                let is_night = if from_h > to_h {
                    now >= from_h || now < to_h
                } else {
                    now >= from_h && now < to_h
                };

                // Accent color scheduling
                if self.scheduled_accent_enabled {
                    let raw = if is_night {
                        &self.night_accent
                    } else {
                        &self.day_accent
                    };
                    // `accent-color` is an enum — snap any custom hex to the
                    // nearest named GNOME accent before writing.
                    let target = color_picker::nearest_accent_id(raw);
                    let iface = gio::Settings::new("org.gnome.desktop.interface");
                    let current = iface.string("accent-color").to_string();
                    if current != target {
                        let _ = iface.set_string("accent-color", &target);
                        tracing::info!(
                            "scheduled accent: {current} \u{2192} {target} ({})",
                            if is_night { "night" } else { "day" }
                        );
                    }

                    // If "use accent for panel" is on, also update panel tint
                    if self.use_accent_for_panel {
                        let accent_hex = current_accent_hex();
                        self.panel_tint_hex = accent_hex;
                    }
                }

                // Panel tint scheduling
                if self.scheduled_panel_enabled {
                    let target_tint = if is_night {
                        &self.night_panel_tint
                    } else {
                        &self.day_panel_tint
                    };
                    if self.panel_tint_hex != *target_tint {
                        self.panel_tint_hex = target_tint.clone();
                        tracing::info!(
                            "scheduled panel tint: \u{2192} {target_tint} ({})",
                            if is_night { "night" } else { "day" }
                        );
                    }
                }
            }
            // --- Headerbar / CSD ---
            ThemeBuilderMsg::SetHeaderbarHeight(v) => self.headerbar_height = v,
            ThemeBuilderMsg::SetHeaderbarShadow(v) => self.headerbar_shadow = v,
            ThemeBuilderMsg::SetCircularButtons(v) => self.circular_buttons = v,
            ThemeBuilderMsg::SetShowWindowShadow(v) => self.show_window_shadow = v,
            ThemeBuilderMsg::SetInsetBorder(v) => self.inset_border = v,
            // --- Visual Insets ---
            ThemeBuilderMsg::SetCardBorderWidth(v) => self.card_border_width = v,
            ThemeBuilderMsg::SetSeparatorOpacity(v) => self.separator_opacity = v,
            ThemeBuilderMsg::SetFocusRingWidth(v) => self.focus_ring_width = v,
            ThemeBuilderMsg::SetComboInset(v) => self.combo_inset = v,
            // --- Sidebar ---
            ThemeBuilderMsg::SetSidebarOpacity(v) => self.sidebar_opacity = v,
            ThemeBuilderMsg::SetSidebarFgOverride(v) => self.sidebar_fg_override = v,
            // --- Layer separation ---
            ThemeBuilderMsg::SetLayerHeaderbarBottom(v) => self.layer_headerbar_bottom = v,
            ThemeBuilderMsg::SetLayerSidebarDivider(v) => self.layer_sidebar_divider = v,
            ThemeBuilderMsg::SetLayerContentContrast(v) => self.layer_content_contrast = v,
            // --- Notifications ---
            ThemeBuilderMsg::SetNotificationRadius(v) => self.notification_radius = v,
            ThemeBuilderMsg::SetNotificationOpacity(v) => self.notification_opacity = v,

            // --- Window Behavior (Mutter GSettings, live) ---
            ThemeBuilderMsg::SetEdgeTiling(enabled) => {
                let m = gio::Settings::new("org.gnome.mutter");
                let _ = m.set_boolean("edge-tiling", enabled);
            }
            ThemeBuilderMsg::SetCenterNewWindows(enabled) => {
                let m = gio::Settings::new("org.gnome.mutter");
                let _ = m.set_boolean("center-new-windows", enabled);
            }
            ThemeBuilderMsg::SetAttachModals(enabled) => {
                let m = gio::Settings::new("org.gnome.mutter");
                let _ = m.set_boolean("attach-modal-dialogs", enabled);
            }
            ThemeBuilderMsg::SetAutoMaximize(enabled) => {
                let m = gio::Settings::new("org.gnome.mutter");
                let _ = m.set_boolean("auto-maximize", enabled);
            }
            ThemeBuilderMsg::SetDynamicWorkspaces(enabled) => {
                let m = gio::Settings::new("org.gnome.mutter");
                let _ = m.set_boolean("dynamic-workspaces", enabled);
            }
            ThemeBuilderMsg::SetDraggableBorderWidth(width) => {
                let m = gio::Settings::new("org.gnome.mutter");
                let _ = m.set_int("draggable-border-width", width);
            }
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
                // Persist all theme builder values to GSettings
                let app = gio::Settings::new("io.github.gnomex.GnomeX");
                let _ = app.set_double("tb-window-radius", self.window_radius);
                let _ = app.set_double("tb-element-radius", self.element_radius);
                let _ = app.set_double("tb-panel-radius", self.panel_radius);
                let _ = app.set_double("tb-panel-opacity", self.panel_opacity);
                let _ = app.set_string("tb-panel-tint", &self.panel_tint_hex);
                let _ = app.set_double("tb-tint-intensity", self.tint_intensity);
                let _ = app.set_double("tb-dash-opacity", self.dash_opacity);
                let _ = app.set_boolean("tb-overview-blur", self.overview_blur);
                let _ = app.set_double("tb-headerbar-height", self.headerbar_height);
                let _ = app.set_double("tb-headerbar-shadow", self.headerbar_shadow);
                let _ = app.set_boolean("tb-circular-buttons", self.circular_buttons);
                let _ = app.set_boolean("tb-show-window-shadow", self.show_window_shadow);
                let _ = app.set_double("tb-inset-border", self.inset_border);
                let _ = app.set_double("tb-card-border-width", self.card_border_width);
                let _ = app.set_double("tb-separator-opacity", self.separator_opacity);
                let _ = app.set_double("tb-focus-ring-width", self.focus_ring_width);
                let _ = app.set_boolean("tb-combo-inset", self.combo_inset);
                let _ = app.set_double("tb-sidebar-opacity", self.sidebar_opacity);
                let _ = app.set_string("tb-sidebar-fg-override", &self.sidebar_fg_override);
                let _ = app.set_double("tb-layer-headerbar-bottom", self.layer_headerbar_bottom);
                let _ = app.set_double("tb-layer-sidebar-divider", self.layer_sidebar_divider);
                let _ = app.set_double("tb-layer-content-contrast", self.layer_content_contrast);
                let _ = app.set_double("tb-notification-radius", self.notification_radius);
                let _ = app.set_double("tb-notification-opacity", self.notification_opacity);

                // Generate and write CSS
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
                self.sidebar_opacity = 1.0;
                self.sidebar_fg_override = String::new();
                self.layer_headerbar_bottom = 0.0;
                self.layer_sidebar_divider = 0.0;
                self.layer_content_contrast = 0.0;

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
        use gnomex_domain::{HeaderbarSpec, WindowFrameSpec, InsetSpec};
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
            headerbar: HeaderbarSpec {
                min_height: Radius::new(self.headerbar_height)?,
                shadow_intensity: Opacity::from_fraction(self.headerbar_shadow)?,
                circular_buttons: self.circular_buttons,
            },
            window_frame: WindowFrameSpec {
                show_shadow: self.show_window_shadow,
                inset_border: Radius::new(self.inset_border)?,
            },
            insets: InsetSpec {
                card_border_width: Radius::new(self.card_border_width)?,
                separator_opacity: Opacity::from_percent(self.separator_opacity)?,
                focus_ring_width: Radius::new(self.focus_ring_width)?,
                combo_inset: self.combo_inset,
            },
            foreground: gnomex_domain::ForegroundSpec::default(),
            layers: LayerSeparationSpec {
                headerbar_bottom: Radius::new(self.layer_headerbar_bottom)?,
                sidebar_divider: Radius::new(self.layer_sidebar_divider)?,
                content_contrast: Opacity::from_fraction(self.layer_content_contrast)?,
            },
            sidebar: SidebarSpec {
                opacity: Opacity::from_fraction(self.sidebar_opacity)?,
                fg_override: {
                    let trimmed = self.sidebar_fg_override.trim();
                    if trimmed.is_empty() {
                        None
                    } else {
                        Some(HexColor::new(trimmed)?)
                    }
                },
            },
            status_colors: gnomex_domain::StatusColorSpec::default(),
            notifications: gnomex_domain::NotificationSpec {
                radius: Radius::new(self.notification_radius)?,
                opacity: Opacity::from_fraction(self.notification_opacity)?,
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

/// Check whether the bundled `io.github.gnomex@io.github.gnomex`
/// GNOME Shell extension is currently enabled. When it is, we skip
/// the BMS / DtD fallback paths because the extension itself listens
/// to our GSettings keys and drives the effects natively.
fn gnomex_extension_enabled() -> bool {
    const UUID: &str = "io.github.gnomex@io.github.gnomex";
    let has_schema = gio::SettingsSchemaSource::default()
        .and_then(|src| src.lookup("org.gnome.shell", true))
        .is_some();
    if !has_schema {
        return false;
    }
    let s = gio::Settings::new("org.gnome.shell");
    s.strv("enabled-extensions")
        .iter()
        .any(|v| v.as_str() == UUID)
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

// --- Wallpaper color extraction (Material You style) ---

/// Extract dominant colors from a wallpaper image and map them to GNOME accent colors.
fn extract_wallpaper_colors(path: &str) -> Result<Vec<(String, String)>, String> {
    use gnomex_domain::color::{rgb_to_hsv, hsv_to_rgb};

    let pixbuf = gtk::gdk_pixbuf::Pixbuf::from_file_at_scale(path, 64, 64, true)
        .map_err(|e| format!("failed to load wallpaper: {e}"))?;

    let pixels = pixbuf.pixel_bytes().ok_or("no pixel data")?;
    let channels = pixbuf.n_channels() as usize;
    let data = pixels.as_ref();

    let mut color_buckets: std::collections::HashMap<(u16, u8, u8), u32> =
        std::collections::HashMap::new();

    for chunk in data.chunks_exact(channels) {
        if channels < 3 { continue; }
        let (r, g, b) = (chunk[0], chunk[1], chunk[2]);
        let (h, s, v) = rgb_to_hsv(r, g, b);
        if s < 15 || v < 25 || v > 240 { continue; }
        let hue_bin = (h / 10) * 10;
        let sat_bin = (s / 85).min(2);
        let val_bin = (v / 85).min(2);
        *color_buckets.entry((hue_bin, sat_bin, val_bin)).or_default() += 1;
    }

    let mut buckets: Vec<_> = color_buckets.into_iter().collect();
    buckets.sort_by(|a, b| b.1.cmp(&a.1));

    let mut results = Vec::new();
    let mut used_accents = std::collections::HashSet::new();

    for ((hue_bin, sat_bin, val_bin), _) in buckets.iter().take(15) {
        let h = *hue_bin + 5;
        let s = (*sat_bin as u16) * 85 + 42;
        let v = (*val_bin as u16) * 85 + 42;
        let (r, g, b) = hsv_to_rgb(h, s.min(255) as u8, v.min(255) as u8);
        let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
        let accent_id = closest_accent(r, g, b);
        if used_accents.contains(&accent_id) { continue; }
        used_accents.insert(accent_id.clone());
        results.push((hex, accent_id));
        if results.len() >= 5 { break; }
    }

    Ok(results)
}

/// Parse the `cached-wallpaper-palette` GSetting strv back into the
/// `Vec<(hex, accent_id)>` shape the UI renders. Silently skips
/// malformed entries so a corrupted cache can't break the page.
fn load_cached_palette(settings: &gio::Settings) -> Vec<(String, String)> {
    let has_key = settings
        .settings_schema()
        .map(|s| s.has_key("cached-wallpaper-palette"))
        .unwrap_or(false);
    if !has_key {
        return Vec::new();
    }
    settings
        .strv("cached-wallpaper-palette")
        .iter()
        .filter_map(|entry| {
            let s = entry.as_str();
            let (hex, accent) = s.split_once(':')?;
            if hex.is_empty() || accent.is_empty() {
                return None;
            }
            Some((hex.to_owned(), accent.to_owned()))
        })
        .collect()
}

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
