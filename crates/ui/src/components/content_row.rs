// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Factory component for a single content row (theme/icon/cursor) in a GtkListBox.

use adw::prelude::*;
use gnomex_domain::{ContentId, ContentState};
use relm4::prelude::*;

pub struct ContentRowModel {
    name: String,
    creator: String,
    id: u64,
    file_id: u64,
    state: ContentState,
}

pub struct ContentRowInit {
    pub name: String,
    pub creator: String,
    pub id: u64,
    pub file_id: u64,
    pub state: ContentState,
}

#[derive(Debug)]
pub enum ContentRowMsg {
    Action,
}

#[derive(Debug)]
pub enum ContentRowOutput {
    Install(ContentId, u64, String),
    Apply(String),
}

#[relm4::factory(pub)]
impl FactoryComponent for ContentRowModel {
    type Init = ContentRowInit;
    type Input = ContentRowMsg;
    type Output = ContentRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        adw::ActionRow {
            set_title: &self.name,
            set_subtitle: &format!("by {}", self.creator),

            add_suffix = &gtk::Box {
                set_spacing: 8,
                set_valign: gtk::Align::Center,

                gtk::Button {
                    set_label: "Install",
                    add_css_class: "suggested-action",
                    #[watch]
                    set_visible: self.state == ContentState::Available,
                    connect_clicked => ContentRowMsg::Action,
                },

                gtk::Button {
                    set_label: "Apply",
                    #[watch]
                    set_visible: self.state == ContentState::Installed,
                    connect_clicked => ContentRowMsg::Action,
                },

                gtk::Label {
                    set_label: "Active",
                    add_css_class: "dim-label",
                    #[watch]
                    set_visible: self.state == ContentState::Active,
                },
            },
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            name: init.name,
            creator: init.creator,
            id: init.id,
            file_id: init.file_id,
            state: init.state,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            ContentRowMsg::Action => match self.state {
                ContentState::Available => {
                    let _ = sender.output(ContentRowOutput::Install(
                        ContentId(self.id),
                        self.file_id,
                        self.name.clone(),
                    ));
                }
                ContentState::Installed => {
                    let _ = sender.output(ContentRowOutput::Apply(self.name.clone()));
                    self.state = ContentState::Active;
                }
                ContentState::Active => {}
            },
        }
    }
}
