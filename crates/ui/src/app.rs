// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Main application component — owns the AdwViewSwitcher navigation and
//! top-level page switching between Explore, Installed, Packs, and Settings.

use crate::components::explore::ExploreModel;
use crate::components::installed::InstalledModel;
use crate::components::packs::PacksModel;
use crate::components::settings::SettingsModel;
use adw::prelude::*;
use relm4::prelude::*;

pub struct AppModel {
    explore: Controller<ExploreModel>,
    installed: Controller<InstalledModel>,
    packs: Controller<PacksModel>,
    settings: Controller<SettingsModel>,
}

#[derive(Debug)]
pub enum AppMsg {}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = ();
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("Experience"),
            set_default_size: (900, 640),

            #[wrap(Some)]
            set_content = &adw::ToolbarView {
                add_top_bar = &adw::HeaderBar {
                    #[wrap(Some)]
                    set_title_widget = &adw::ViewSwitcher {
                        set_stack: Some(&view_stack),
                        set_policy: adw::ViewSwitcherPolicy::Wide,
                    },
                },

                add_bottom_bar = &adw::ViewSwitcherBar {
                    set_stack: Some(&view_stack),
                },

                #[wrap(Some)]
                #[name = "view_stack"]
                set_content = &adw::ViewStack {},
            },
        }
    }

    fn init(_: (), root: Self::Root, _sender: ComponentSender<Self>) -> ComponentParts<Self> {
        let explore = ExploreModel::builder()
            .launch(())
            .detach();

        let installed = InstalledModel::builder()
            .launch(())
            .detach();

        let packs = PacksModel::builder()
            .launch(())
            .detach();

        let settings = SettingsModel::builder()
            .launch(())
            .detach();

        let model = AppModel {
            explore,
            installed,
            packs,
            settings,
        };

        let widgets = view_output!();

        // Add pages to ViewStack programmatically (Relm4 view! macro doesn't
        // support add_titled_with_icon directly)
        let stack = &widgets.view_stack;
        stack.add_titled_with_icon(
            model.explore.widget(),
            Some("explore"),
            "Explore",
            "system-search-symbolic",
        );
        stack.add_titled_with_icon(
            model.installed.widget(),
            Some("installed"),
            "Installed",
            "view-grid-symbolic",
        );
        stack.add_titled_with_icon(
            model.packs.widget(),
            Some("packs"),
            "Packs",
            "package-x-generic-symbolic",
        );
        stack.add_titled_with_icon(
            model.settings.widget(),
            Some("settings"),
            "Settings",
            "emblem-system-symbolic",
        );

        ComponentParts { model, widgets }
    }
}
