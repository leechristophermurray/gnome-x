// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Installed view — lists all extensions on the system with enable/disable toggles.
//! HIG pattern: boxed-list with AdwActionRow, AdwStatusPage for empty state.

use crate::components::detail_view::{DetailItem, DetailViewModel, DetailViewMsg, DetailViewOutput};
use crate::components::extension_row::{
    ExtensionRowInit, ExtensionRowModel, ExtensionRowOutput, RowMode,
};
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
    detail: Controller<DetailViewModel>,
    content_stack: gtk::Stack,
    has_extensions: bool,
    is_loading: bool,
}

impl InstalledModel {
    fn update_stack(&self) {
        let name = if self.is_loading {
            "loading"
        } else if self.has_extensions {
            "list"
        } else {
            "empty"
        };
        self.content_stack.set_visible_child_name(name);
    }
}

#[derive(Debug)]
pub enum InstalledMsg {
    Refresh,
    Loaded(Vec<Extension>),
    LoadFailed(String),
    ToggleExtension(ExtensionUuid, bool),
    ToggleComplete(ExtensionUuid, bool),
    ToggleFailed(String),
    ShowDetail {
        name: String,
        uuid: String,
        creator: String,
        description: String,
        screenshot_url: Option<String>,
        installed: bool,
    },
    DetailBack,
}

#[derive(Debug)]
pub enum InstalledOutput {
    Toast(String),
}

#[relm4::component(pub)]
impl SimpleComponent for InstalledModel {
    type Init = AppServices;
    type Input = InstalledMsg;
    type Output = InstalledOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
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
                    ExtensionRowOutput::ShowDetail {
                        name,
                        uuid,
                        creator,
                        description,
                        screenshot_url,
                        installed,
                    } => InstalledMsg::ShowDetail {
                        name,
                        uuid,
                        creator,
                        description,
                        screenshot_url,
                        installed,
                    },
                });

        let detail = DetailViewModel::builder()
            .launch(())
            .forward(sender.input_sender(), |output| match output {
                DetailViewOutput::Back => InstalledMsg::DetailBack,
                DetailViewOutput::InstallExtension(_) | DetailViewOutput::InstallContent(_, _) => {
                    InstalledMsg::Refresh
                }
            });

        // Build the content stack
        let content_stack = gtk::Stack::new();
        content_stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);

        let empty_page = adw::StatusPage::builder()
            .icon_name("view-grid-symbolic")
            .title("No Extensions Installed")
            .description("Extensions you install will appear here")
            .build();
        content_stack.add_named(&empty_page, Some("empty"));

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
        content_stack.add_named(&loading_box, Some("loading"));

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
        {
            let w: &gtk::ListBox = extensions.widget();
            w.set_selection_mode(gtk::SelectionMode::None);
            w.add_css_class("boxed-list");
            list_box_container.append(w);
        }
        list_clamp.set_child(Some(&list_box_container));
        list_scroll.set_child(Some(&list_clamp));
        content_stack.add_named(&list_scroll, Some("list"));

        // Detail page
        content_stack.add_named(detail.widget(), Some("detail"));

        content_stack.set_visible_child_name("loading");
        content_stack.set_vexpand(true);

        let model = InstalledModel {
            handle: services.handle,
            manage: services.manage,
            extensions,
            detail,
            content_stack: content_stack.clone(),
            has_extensions: false,
            is_loading: true,
        };

        let widgets = view_output!();

        root.append(&content_stack);

        sender.input(InstalledMsg::Refresh);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: InstalledMsg, sender: ComponentSender<Self>) {
        match msg {
            InstalledMsg::Refresh => {
                self.is_loading = true;
                self.update_stack();

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
                self.update_stack();

                let mut guard = self.extensions.guard();
                guard.clear();
                for ext in extensions {
                    guard.push_back(ExtensionRowInit {
                        name: ext.name,
                        uuid: ext.uuid.as_str().to_owned(),
                        creator: ext.creator,
                        description: ext.description,
                        screenshot_url: ext.screenshot_url,
                        state: ext.state,
                        mode: RowMode::Installed,
                    });
                }
            }
            InstalledMsg::LoadFailed(err) => {
                self.is_loading = false;
                self.has_extensions = false;
                self.update_stack();
                let _ = sender.output(InstalledOutput::Toast(format!(
                    "Failed to load extensions: {err}"
                )));
            }
            InstalledMsg::ToggleExtension(uuid, enabled) => {
                tracing::info!("toggling {uuid} to {enabled}");

                let manage = self.manage.clone();
                let input_sender = sender.input_sender().clone();
                let uuid_clone = uuid.clone();
                self.handle.spawn(async move {
                    match manage.toggle_extension(&uuid_clone, enabled).await {
                        Ok(()) => {
                            input_sender.emit(InstalledMsg::ToggleComplete(uuid_clone, enabled));
                        }
                        Err(e) => {
                            input_sender.emit(InstalledMsg::ToggleFailed(e.to_string()));
                        }
                    }
                });
            }
            InstalledMsg::ToggleComplete(uuid, enabled) => {
                let state = if enabled { "enabled" } else { "disabled" };
                tracing::info!("{uuid} {state}");
            }
            InstalledMsg::ToggleFailed(err) => {
                let _ = sender.output(InstalledOutput::Toast(format!("Toggle failed: {err}")));
            }
            InstalledMsg::ShowDetail {
                name,
                uuid,
                creator,
                description,
                screenshot_url,
                installed,
            } => {
                self.detail.emit(DetailViewMsg::Show(DetailItem::Extension {
                    name,
                    creator,
                    uuid,
                    description,
                    screenshot_url,
                    installed,
                }));
                self.content_stack.set_visible_child_name("detail");
            }
            InstalledMsg::DetailBack => {
                self.update_stack();
            }
        }
    }
}
