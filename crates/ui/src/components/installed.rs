// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Installed view — lists extensions, themes, icons, and cursors on the system.
//! Extensions get enable/disable toggles. Themes/icons/cursors are listed by name.

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
    themes_list: gtk::ListBox,
    icons_list: gtk::ListBox,
    cursors_list: gtk::ListBox,
    has_content: bool,
    is_loading: bool,
}

impl InstalledModel {
    fn update_stack(&self) {
        let name = if self.is_loading {
            "loading"
        } else if self.has_content {
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
    ContentLoaded {
        themes: Vec<String>,
        icons: Vec<String>,
        cursors: Vec<String>,
    },
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

        // Content stack
        let content_stack = gtk::Stack::new();
        content_stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);

        let empty_page = adw::StatusPage::builder()
            .icon_name("view-grid-symbolic")
            .title("Nothing Installed")
            .description("Extensions, themes, and icons you install will appear here")
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
            .label("Loading\u{2026}")
            .css_classes(["dim-label"])
            .build();
        loading_box.append(&loading_label);
        content_stack.add_named(&loading_box, Some("loading"));

        // Main list scroll
        let list_scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let list_clamp = adw::Clamp::builder()
            .maximum_size(600)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        let list_container = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .build();

        // Extensions section
        let ext_label = gtk::Label::builder()
            .label("Extensions")
            .halign(gtk::Align::Start)
            .css_classes(["title-2"])
            .build();
        list_container.append(&ext_label);
        {
            let w: &gtk::ListBox = extensions.widget();
            w.set_selection_mode(gtk::SelectionMode::None);
            w.add_css_class("boxed-list");
            list_container.append(w);
        }

        // Themes section
        let themes_label = gtk::Label::builder()
            .label("Themes")
            .halign(gtk::Align::Start)
            .css_classes(["title-2"])
            .build();
        let themes_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        list_container.append(&themes_label);
        list_container.append(&themes_list);

        // Icons section
        let icons_label = gtk::Label::builder()
            .label("Icon Packs")
            .halign(gtk::Align::Start)
            .css_classes(["title-2"])
            .build();
        let icons_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        list_container.append(&icons_label);
        list_container.append(&icons_list);

        // Cursors section
        let cursors_label = gtk::Label::builder()
            .label("Cursors")
            .halign(gtk::Align::Start)
            .css_classes(["title-2"])
            .build();
        let cursors_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        list_container.append(&cursors_label);
        list_container.append(&cursors_list);

        list_clamp.set_child(Some(&list_container));
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
            themes_list: themes_list.clone(),
            icons_list: icons_list.clone(),
            cursors_list: cursors_list.clone(),
            has_content: false,
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

                // Load extensions (async — needs D-Bus + EGO enrichment)
                let manage = self.manage.clone();
                let input_sender = sender.input_sender().clone();
                self.handle.spawn(async move {
                    match manage.list_installed_extensions().await {
                        Ok(extensions) => {
                            tracing::info!("found {} installed extensions", extensions.len());
                            input_sender.emit(InstalledMsg::Loaded(extensions));
                        }
                        Err(e) => {
                            input_sender.emit(InstalledMsg::LoadFailed(e.to_string()));
                        }
                    }
                });

                // Load themes/icons/cursors (sync — filesystem only)
                let themes = self.manage.list_installed_themes().unwrap_or_default();
                let icons = self.manage.list_installed_icons().unwrap_or_default();
                let cursors = self.manage.list_installed_cursors().unwrap_or_default();
                sender.input_sender().emit(InstalledMsg::ContentLoaded {
                    themes,
                    icons,
                    cursors,
                });
            }
            InstalledMsg::Loaded(extensions) => {
                self.is_loading = false;
                self.has_content = !extensions.is_empty()
                    || self.themes_list.first_child().is_some()
                    || self.icons_list.first_child().is_some();
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
            InstalledMsg::ContentLoaded {
                themes,
                icons,
                cursors,
            } => {
                populate_name_list(&self.themes_list, &themes);
                populate_name_list(&self.icons_list, &icons);
                populate_name_list(&self.cursors_list, &cursors);

                // Update has_content if we got content items even before extensions load
                if !themes.is_empty() || !icons.is_empty() || !cursors.is_empty() {
                    self.has_content = true;
                }
            }
            InstalledMsg::LoadFailed(err) => {
                self.is_loading = false;
                self.update_stack();
                let _ = sender.output(InstalledOutput::Toast(format!(
                    "Failed to load extensions: {err}"
                )));
            }
            InstalledMsg::ToggleExtension(uuid, enabled) => {
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
            InstalledMsg::ToggleComplete(_uuid, _enabled) => {}
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

/// Populate a ListBox with simple name-only rows.
fn populate_name_list(list: &gtk::ListBox, names: &[String]) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for name in names {
        let row = adw::ActionRow::builder()
            .title(name)
            .build();
        list.append(&row);
    }
}
