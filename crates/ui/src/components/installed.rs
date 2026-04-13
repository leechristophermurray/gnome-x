// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Installed view — lists all extensions on the system with enable/disable toggles.
//! HIG pattern: boxed-list with AdwActionRow, AdwStatusPage for empty state.

use crate::components::extension_row::{ExtensionRowInit, ExtensionRowModel, ExtensionRowOutput};
use crate::services::AppServices;
use adw::prelude::*;
use gnomex_app::use_cases::ManageUseCase;
use gnomex_domain::{Extension, ExtensionUuid};
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;
use std::sync::Arc;
use tokio::runtime::Handle;

pub struct InstalledModel {
    handle: Handle,
    manage: Arc<ManageUseCase>,
    extensions: FactoryVecDeque<ExtensionRowModel>,
    has_extensions: bool,
    is_loading: bool,
}

#[derive(Debug)]
pub enum InstalledMsg {
    Refresh,
    Loaded(Vec<Extension>),
    LoadFailed(String),
    ToggleExtension(ExtensionUuid, bool),
    ToggleComplete(ExtensionUuid, bool),
    ToggleFailed(String),
}

#[derive(Debug)]
pub enum InstalledOutput {}

#[relm4::component(pub)]
impl SimpleComponent for InstalledModel {
    type Init = AppServices;
    type Input = InstalledMsg;
    type Output = InstalledOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            #[name = "content_stack"]
            gtk::Stack {
                #[watch]
                set_visible_child_name: if model.is_loading {
                    "loading"
                } else if model.has_extensions {
                    "list"
                } else {
                    "empty"
                },
            },
        }
    }

    fn init(
        services: AppServices,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let extensions =
            FactoryVecDeque::builder()
                .launch(gtk::ListBox::default())
                .forward(sender.input_sender(), |output| match output {
                    ExtensionRowOutput::Toggle(uuid, active) => {
                        InstalledMsg::ToggleExtension(uuid, active)
                    }
                    ExtensionRowOutput::Install(_) => unreachable!(),
                });

        let model = InstalledModel {
            handle: services.handle,
            manage: services.manage,
            extensions,
            has_extensions: false,
            is_loading: false,
        };

        let widgets = view_output!();

        // Build stack pages programmatically
        let empty_page = adw::StatusPage::builder()
            .icon_name("view-grid-symbolic")
            .title("No Extensions Installed")
            .description("Extensions you install will appear here")
            .build();
        widgets.content_stack.add_named(&empty_page, Some("empty"));

        let loading_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::Center)
            .halign(gtk::Align::Center)
            .spacing(12)
            .build();
        let spinner = gtk::Spinner::builder()
            .spinning(true)
            .width_request(32)
            .height_request(32)
            .build();
        loading_box.append(&spinner);
        let loading_label = gtk::Label::builder()
            .label("Loading extensions\u{2026}")
            .css_classes(["dim-label"])
            .build();
        loading_box.append(&loading_label);
        widgets
            .content_stack
            .add_named(&loading_box, Some("loading"));

        let list_scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let list_clamp = adw::Clamp::builder()
            .maximum_size(600)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        let list_box_container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(12)
            .build();
        let title = gtk::Label::builder()
            .label("Installed Extensions")
            .halign(gtk::Align::Start)
            .css_classes(["title-2"])
            .build();
        list_box_container.append(&title);
        let ext_list = model.extensions.widget().clone();
        ext_list.set_selection_mode(gtk::SelectionMode::None);
        ext_list.add_css_class("boxed-list");
        list_box_container.append(&ext_list);
        list_clamp.set_child(Some(&list_box_container));
        list_scroll.set_child(Some(&list_clamp));
        widgets.content_stack.add_named(&list_scroll, Some("list"));

        // Trigger initial load
        sender.input(InstalledMsg::Refresh);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: InstalledMsg, sender: ComponentSender<Self>) {
        match msg {
            InstalledMsg::Refresh => {
                self.is_loading = true;

                let manage = self.manage.clone();
                let input_sender = sender.input_sender().clone();
                self.handle.spawn(async move {
                    tracing::info!("loading installed extensions via D-Bus");
                    match manage.list_installed_extensions().await {
                        Ok(extensions) => {
                            tracing::info!("found {} installed extensions", extensions.len());
                            input_sender.emit(InstalledMsg::Loaded(extensions));
                        }
                        Err(e) => {
                            tracing::error!("failed to list extensions: {e}");
                            input_sender.emit(InstalledMsg::LoadFailed(e.to_string()));
                        }
                    }
                });
            }
            InstalledMsg::Loaded(extensions) => {
                self.is_loading = false;
                self.has_extensions = !extensions.is_empty();

                let mut guard = self.extensions.guard();
                guard.clear();
                for ext in extensions {
                    guard.push_back(ExtensionRowInit {
                        name: ext.name,
                        uuid: ext.uuid.as_str().to_owned(),
                        creator: ext.creator,
                        description: ext.description,
                        state: ext.state,
                    });
                }
            }
            InstalledMsg::LoadFailed(err) => {
                self.is_loading = false;
                self.has_extensions = false;
                tracing::warn!("load failed: {err}");
            }
            InstalledMsg::ToggleExtension(uuid, enabled) => {
                tracing::info!("toggling {uuid} to {enabled}");

                let manage = self.manage.clone();
                let input_sender = sender.input_sender().clone();
                let uuid_clone = uuid.clone();
                self.handle.spawn(async move {
                    match manage.toggle_extension(&uuid_clone, enabled).await {
                        Ok(()) => {
                            tracing::info!("{uuid_clone} toggled to {enabled}");
                            input_sender
                                .emit(InstalledMsg::ToggleComplete(uuid_clone, enabled));
                        }
                        Err(e) => {
                            tracing::error!("toggle failed: {e}");
                            input_sender.emit(InstalledMsg::ToggleFailed(e.to_string()));
                        }
                    }
                });
            }
            InstalledMsg::ToggleComplete(uuid, enabled) => {
                let state = if enabled { "enabled" } else { "disabled" };
                tracing::info!("{uuid} {state}");
                // TODO: show AdwToast confirmation
            }
            InstalledMsg::ToggleFailed(err) => {
                tracing::warn!("toggle error: {err}");
                // TODO: show AdwToast error, refresh list to restore state
            }
        }
    }
}
