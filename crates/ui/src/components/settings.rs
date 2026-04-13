// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Settings view — app preferences using AdwPreferencesPage.
//! HIG pattern: PreferencesGroup with SwitchRow and ActionRow.

use adw::prelude::*;
use relm4::prelude::*;

pub struct SettingsModel {
    version_validation_disabled: bool,
}

#[derive(Debug)]
pub enum SettingsMsg {
    ToggleVersionValidation(bool),
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
        let model = SettingsModel {
            version_validation_disabled: false,
        };
        let widgets = view_output!();

        // Build preferences UI programmatically (PreferencesGroup doesn't
        // implement RelmContainerExt, so view! macro can't nest into it)
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

        // Extensions section
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

        // About section
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

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: SettingsMsg, _sender: ComponentSender<Self>) {
        match msg {
            SettingsMsg::ToggleVersionValidation(active) => {
                self.version_validation_disabled = active;
                tracing::info!("version validation disabled: {active}");
            }
        }
    }
}
