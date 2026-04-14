// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Settings view — app preferences using AdwPreferencesPage.
//! HIG pattern: PreferencesGroup with SwitchRow, ActionRow, and custom accent picker.

use adw::prelude::*;
use relm4::prelude::*;

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

pub struct SettingsModel {
    version_validation_disabled: bool,
    #[allow(dead_code)]
    accent_buttons: Vec<gtk::ToggleButton>,
}

#[derive(Debug)]
pub enum SettingsMsg {
    ToggleVersionValidation(bool),
    SetAccentColor(String),
}

#[derive(Debug)]
pub enum SettingsOutput {}

#[relm4::component(pub)]
impl SimpleComponent for SettingsModel {
    type Init = ();
    type Input = SettingsMsg;
    type Output = SettingsOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_vexpand: true,
        }
    }

    fn init(_: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let clamp = adw::Clamp::builder()
            .maximum_size(600)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        let outer_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .build();

        // --- Appearance section ---
        let appearance_group = adw::PreferencesGroup::builder()
            .title("Appearance")
            .description("Customize the look of your desktop")
            .build();

        // Accent color picker
        let accent_row = adw::ActionRow::builder()
            .title("Accent Color")
            .subtitle("Choose the highlight color used across apps")
            .build();

        let color_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .valign(gtk::Align::Center)
            .build();

        // Read current accent color from GSettings
        let iface = gio::Settings::new("org.gnome.desktop.interface");
        let current_accent = iface.string("accent-color").to_string();

        let mut accent_buttons = Vec::new();
        for (i, &(id, label, hex)) in ACCENT_COLORS.iter().enumerate() {
            let btn = gtk::ToggleButton::builder()
                .width_request(32)
                .height_request(32)
                .tooltip_text(label)
                .active(id == current_accent)
                .build();

            // Color the button with a unique CSS class
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

            // Radio group behavior
            if i > 0 {
                if let Some(first) = color_box.first_child() {
                    btn.set_group(first.downcast_ref::<gtk::ToggleButton>());
                }
            }

            let sender = sender.clone();
            let accent_id = id.to_owned();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    sender.input(SettingsMsg::SetAccentColor(accent_id.clone()));
                }
            });

            color_box.append(&btn);
            accent_buttons.push(btn);
        }

        accent_row.add_suffix(&color_box);
        appearance_group.add(&accent_row);
        outer_box.append(&appearance_group);

        // --- Extensions section ---
        let ext_group = adw::PreferencesGroup::builder()
            .title("Extensions")
            .description("Configure extension behavior")
            .build();

        let version_switch = adw::SwitchRow::builder()
            .title("Disable Version Validation")
            .subtitle("Allow installing extensions not marked compatible with the current shell version")
            .build();
        {
            let sender = sender.clone();
            version_switch.connect_active_notify(move |row| {
                sender.input(SettingsMsg::ToggleVersionValidation(row.is_active()));
            });
        }
        ext_group.add(&version_switch);
        outer_box.append(&ext_group);

        // --- About section ---
        let about_group = adw::PreferencesGroup::builder()
            .title("About")
            .build();

        let version_row = adw::ActionRow::builder()
            .title("Version")
            .subtitle(env!("CARGO_PKG_VERSION"))
            .build();
        about_group.add(&version_row);

        let license_row = adw::ActionRow::builder()
            .title("License")
            .subtitle("Apache License 2.0")
            .build();
        about_group.add(&license_row);
        outer_box.append(&about_group);

        clamp.set_child(Some(&outer_box));
        scroll.set_child(Some(&clamp));
        root.append(&scroll);

        let model = SettingsModel {
            version_validation_disabled: false,
            accent_buttons,
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: SettingsMsg, _sender: ComponentSender<Self>) {
        match msg {
            SettingsMsg::ToggleVersionValidation(active) => {
                self.version_validation_disabled = active;
                tracing::info!("version validation disabled: {active}");
            }
            SettingsMsg::SetAccentColor(color) => {
                let iface = gio::Settings::new("org.gnome.desktop.interface");
                match iface.set_string("accent-color", &color) {
                    Ok(_) => tracing::info!("accent color set to: {color}"),
                    Err(e) => tracing::warn!("failed to set accent color: {e}"),
                }
            }
        }
    }
}
