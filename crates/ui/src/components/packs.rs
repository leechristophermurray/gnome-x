// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Packs view — experience pack management (Phase 3 placeholder).
//! Shows an AdwStatusPage indicating this feature is coming soon.

use adw::prelude::*;
use relm4::prelude::*;

pub struct PacksModel;

#[derive(Debug)]
pub enum PacksMsg {}

#[derive(Debug)]
pub enum PacksOutput {}

#[relm4::component(pub)]
impl SimpleComponent for PacksModel {
    type Init = ();
    type Input = PacksMsg;
    type Output = PacksOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_vexpand: true,

            adw::StatusPage {
                set_icon_name: Some("package-x-generic-symbolic"),
                set_title: "Experience Packs",
                set_description: Some("Snapshot and share complete desktop configurations.\nComing in a future update."),
                set_vexpand: true,
            },
        }
    }

    fn init(_: (), root: Self::Root, _sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let model = PacksModel;
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }
}
