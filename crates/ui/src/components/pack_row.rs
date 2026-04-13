// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Factory component for a single Experience Pack row in a GtkListBox.

use adw::prelude::*;
use relm4::prelude::*;

pub struct PackRowModel {
    id: String,
    name: String,
    author: String,
    description: String,
}

pub struct PackRowInit {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
}

#[derive(Debug)]
pub enum PackRowMsg {
    Apply,
    Export,
    Delete,
}

#[derive(Debug)]
pub enum PackRowOutput {
    Apply(String),
    Export(String, String), // (id, suggested filename)
    Delete(String),
}

#[relm4::factory(pub)]
impl FactoryComponent for PackRowModel {
    type Init = PackRowInit;
    type Input = PackRowMsg;
    type Output = PackRowOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::ListBox;

    view! {
        adw::ActionRow {
            set_title: &self.name,
            set_subtitle: &format!("by {} \u{2014} {}", self.author, self.description),
            set_activatable: false,

            add_suffix = &gtk::Box {
                set_spacing: 8,
                set_valign: gtk::Align::Center,

                gtk::Button {
                    set_label: "Apply",
                    add_css_class: "suggested-action",
                    connect_clicked => PackRowMsg::Apply,
                },

                gtk::Button {
                    set_icon_name: "document-save-symbolic",
                    set_tooltip_text: Some("Export"),
                    add_css_class: "flat",
                    connect_clicked => PackRowMsg::Export,
                },

                gtk::Button {
                    set_icon_name: "user-trash-symbolic",
                    set_tooltip_text: Some("Delete"),
                    add_css_class: "flat",
                    connect_clicked => PackRowMsg::Delete,
                },
            },
        }
    }

    fn init_model(init: Self::Init, _index: &DynamicIndex, _sender: FactorySender<Self>) -> Self {
        Self {
            id: init.id,
            name: init.name,
            author: init.author,
            description: init.description,
        }
    }

    fn update(&mut self, msg: Self::Input, sender: FactorySender<Self>) {
        match msg {
            PackRowMsg::Apply => {
                let _ = sender.output(PackRowOutput::Apply(self.id.clone()));
            }
            PackRowMsg::Export => {
                let filename = format!("{}.gnomex-pack.tar.gz", self.id);
                let _ = sender.output(PackRowOutput::Export(self.id.clone(), filename));
            }
            PackRowMsg::Delete => {
                let _ = sender.output(PackRowOutput::Delete(self.id.clone()));
            }
        }
    }
}
