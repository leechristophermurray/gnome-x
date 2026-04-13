// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Factory component for a single extension row in a GtkListBox.
//! Used by both Explore (with Install button) and Installed (with toggle switch).

use adw::prelude::*;
use gnomex_domain::{ExtensionState, ExtensionUuid};
use relm4::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowMode {
    Explore,
    Installed,
}

pub struct ExtensionRowModel {
    name: String,
    uuid: String,
    creator: String,
    description: String,
    screenshot_url: Option<String>,
    state: ExtensionState,
    mode: RowMode,
}

pub struct ExtensionRowInit {
    pub name: String,
    pub uuid: String,
    pub creator: String,
    pub description: String,
    pub screenshot_url: Option<String>,
    pub state: ExtensionState,
    pub mode: RowMode,
}

#[derive(Debug)]
pub enum ExtensionRowMsg {
    ToggleEnabled(bool),
    Activated,
}

#[derive(Debug)]
pub enum ExtensionRowOutput {
    Install(ExtensionUuid),
    Toggle(ExtensionUuid, bool),
    ShowDetail {
        name: String,
        uuid: String,
        creator: String,
        description: String,
        screenshot_url: Option<String>,
        installed: bool,
    },
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
            set_activatable: true,
            connect_activated => ExtensionRowMsg::Activated,

            add_suffix = &gtk::Box {
                set_spacing: 8,
                set_valign: gtk::Align::Center,

                gtk::Button {
                    set_label: "Install",
                    add_css_class: "suggested-action",
                    #[watch]
                    set_visible: self.state == ExtensionState::Available,
                    connect_clicked => ExtensionRowMsg::ToggleEnabled(true),
                },

                gtk::Label {
                    set_label: "Installed",
                    add_css_class: "dim-label",
                    #[watch]
                    set_visible: self.mode == RowMode::Explore && self.state != ExtensionState::Available,
                },

                gtk::Switch {
                    #[watch]
                    set_visible: self.mode == RowMode::Installed && self.state != ExtensionState::Available,
                    #[watch]
                    set_active: self.state == ExtensionState::Enabled,
                    set_valign: gtk::Align::Center,
                    connect_state_set[sender] => move |_, active| {
                        sender.input(ExtensionRowMsg::ToggleEnabled(active));
                        glib::Propagation::Proceed
                    },
                },

                gtk::Image {
                    set_icon_name: Some("go-next-symbolic"),
                    add_css_class: "dim-label",
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
            screenshot_url: init.screenshot_url,
            state: init.state,
            mode: init.mode,
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
            ExtensionRowMsg::Activated => {
                let _ = sender.output(ExtensionRowOutput::ShowDetail {
                    name: self.name.clone(),
                    uuid: self.uuid.clone(),
                    creator: self.creator.clone(),
                    description: self.description.clone(),
                    screenshot_url: self.screenshot_url.clone(),
                    installed: self.state != ExtensionState::Available,
                });
            }
        }
    }
}
