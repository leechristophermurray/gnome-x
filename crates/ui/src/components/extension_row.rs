// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Factory component for a single extension row in a GtkListBox.
//! Used by both Explore (with Install button) and Installed (with toggle switch).

use adw::prelude::*;
use gnomex_domain::{ExtensionState, ExtensionUuid};
use relm4::prelude::*;

pub struct ExtensionRowModel {
    name: String,
    uuid: String,
    creator: String,
    #[allow(dead_code)]
    description: String,
    state: ExtensionState,
}

pub struct ExtensionRowInit {
    pub name: String,
    pub uuid: String,
    pub creator: String,
    pub description: String,
    pub state: ExtensionState,
}

#[derive(Debug)]
pub enum ExtensionRowMsg {
    ToggleEnabled(bool),
}

#[derive(Debug)]
pub enum ExtensionRowOutput {
    Install(ExtensionUuid),
    Toggle(ExtensionUuid, bool),
}

#[relm4::factory(pub)]
impl FactoryComponent for ExtensionRowModel {
    type Init = ExtensionRowInit;
    type Input = ExtensionRowMsg;
    type Output = ExtensionRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        adw::ActionRow {
            set_title: &self.name,
            set_subtitle: &format!("by {} \u{2014} {}", self.creator, self.uuid),

            add_suffix = &gtk::Box {
                set_spacing: 8,
                set_valign: gtk::Align::Center,

                gtk::Button {
                    set_label: "Install",
                    add_css_class: "suggested-action",
                    add_css_class: "pill",
                    #[watch]
                    set_visible: self.state == ExtensionState::Available,
                    connect_clicked => ExtensionRowMsg::ToggleEnabled(true),
                },

                gtk::Switch {
                    #[watch]
                    set_visible: self.state != ExtensionState::Available,
                    #[watch]
                    set_active: self.state == ExtensionState::Enabled,
                    set_valign: gtk::Align::Center,
                    connect_state_set[sender] => move |_, active| {
                        sender.input(ExtensionRowMsg::ToggleEnabled(active));
                        glib::Propagation::Proceed
                    },
                },
            },
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            name: init.name,
            uuid: init.uuid,
            creator: init.creator,
            description: init.description,
            state: init.state,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            ExtensionRowMsg::ToggleEnabled(active) => {
                if let Ok(uuid) = ExtensionUuid::new(&self.uuid) {
                    if self.state == ExtensionState::Available {
                        let _ = sender.output(ExtensionRowOutput::Install(uuid));
                    } else {
                        let _ = sender.output(ExtensionRowOutput::Toggle(uuid, active));
                        self.state = if active {
                            ExtensionState::Enabled
                        } else {
                            ExtensionState::Disabled
                        };
                    }
                }
            }
        }
    }
}
