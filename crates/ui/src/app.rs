// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Main application component — owns the AdwViewSwitcher navigation and
//! top-level page switching.

use crate::components::explore::{ExploreModel, ExploreOutput};
use crate::components::installed::{InstalledModel, InstalledOutput};
use crate::components::packs::{PacksModel, PacksOutput};
use crate::components::settings::SettingsModel;
use crate::components::shell_tweaks::ShellTweaksModel;
use crate::components::theme_builder::{ThemeBuilderModel, ThemeBuilderOutput};
use crate::services::AppServices;
use adw::prelude::*;
use relm4::prelude::*;

pub struct AppModel {
    #[allow(dead_code)]
    explore: Controller<ExploreModel>,
    #[allow(dead_code)]
    installed: Controller<InstalledModel>,
    #[allow(dead_code)]
    theme_builder: Controller<ThemeBuilderModel>,
    #[allow(dead_code)]
    packs: Controller<PacksModel>,
    #[allow(dead_code)]
    settings: Controller<SettingsModel>,
    #[allow(dead_code)]
    shell_tweaks: Controller<ShellTweaksModel>,
    toast_overlay: adw::ToastOverlay,
}

#[derive(Debug)]
pub enum AppMsg {
    Toast(String),
}

#[relm4::component(pub)]
impl SimpleComponent for AppModel {
    type Init = AppServices;
    type Input = AppMsg;
    type Output = ();

    view! {
        adw::ApplicationWindow {
            set_title: Some("Experience"),
            set_icon_name: Some("io.github.gnomex.GnomeX"),
            set_default_size: (900, 640),
        }
    }

    fn init(
        services: AppServices,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let explore = ExploreModel::builder()
            .launch(services.clone())
            .forward(sender.input_sender(), |msg| match msg {
                ExploreOutput::Toast(s) => AppMsg::Toast(s),
            });

        let installed = InstalledModel::builder()
            .launch(services.clone())
            .forward(sender.input_sender(), |msg| match msg {
                InstalledOutput::Toast(s) => AppMsg::Toast(s),
            });

        let theme_builder = ThemeBuilderModel::builder()
            .launch(services.clone())
            .forward(sender.input_sender(), |msg| match msg {
                ThemeBuilderOutput::Toast(s) => AppMsg::Toast(s),
            });

        let packs = PacksModel::builder()
            .launch(services.clone())
            .forward(sender.input_sender(), |msg| match msg {
                PacksOutput::Toast(s) => AppMsg::Toast(s),
            });

        let settings = SettingsModel::builder().launch(()).detach();

        let shell_tweaks = ShellTweaksModel::builder()
            .launch(services.clone())
            .detach();

        // Build the layout with ToastOverlay wrapping ToolbarView
        let toast_overlay = adw::ToastOverlay::new();

        let toolbar_view = adw::ToolbarView::new();

        let view_stack = adw::ViewStack::new();

        let header = adw::HeaderBar::new();
        let switcher = adw::ViewSwitcher::new();
        switcher.set_stack(Some(&view_stack));
        switcher.set_policy(adw::ViewSwitcherPolicy::Wide);
        header.set_title_widget(Some(&switcher));
        toolbar_view.add_top_bar(&header);

        let bottom_bar = adw::ViewSwitcherBar::new();
        bottom_bar.set_stack(Some(&view_stack));
        toolbar_view.add_bottom_bar(&bottom_bar);

        toolbar_view.set_content(Some(&view_stack));

        toast_overlay.set_child(Some(&toolbar_view));
        root.set_content(Some(&toast_overlay));

        view_stack.add_titled_with_icon(
            explore.widget(),
            Some("explore"),
            "Explore",
            "system-search-symbolic",
        );
        view_stack.add_titled_with_icon(
            installed.widget(),
            Some("installed"),
            "Installed",
            "view-grid-symbolic",
        );
        view_stack.add_titled_with_icon(
            theme_builder.widget(),
            Some("theme-builder"),
            "Theme",
            "applications-graphics-symbolic",
        );
        view_stack.add_titled_with_icon(
            packs.widget(),
            Some("packs"),
            "Packs",
            "package-x-generic-symbolic",
        );
        view_stack.add_titled_with_icon(
            shell_tweaks.widget(),
            Some("shell"),
            "Shell",
            "preferences-desktop-symbolic",
        );
        view_stack.add_titled_with_icon(
            settings.widget(),
            Some("settings"),
            "Settings",
            "emblem-system-symbolic",
        );

        let model = AppModel {
            explore,
            installed,
            theme_builder,
            packs,
            settings,
            shell_tweaks,
            toast_overlay: toast_overlay.clone(),
        };

        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: AppMsg, _sender: ComponentSender<Self>) {
        match msg {
            AppMsg::Toast(text) => {
                let toast = adw::Toast::new(&text);
                toast.set_timeout(3);
                self.toast_overlay.add_toast(toast);
            }
        }
    }
}
