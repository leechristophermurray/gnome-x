// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Installed view — lists all extensions on the system with enable/disable toggles.
//! HIG pattern: boxed-list with AdwActionRow, AdwStatusPage for empty state.

use crate::components::extension_row::{ExtensionRowInit, ExtensionRowModel, ExtensionRowOutput};
use adw::prelude::*;
use gnomex_domain::{Extension, ExtensionUuid};
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;

pub struct InstalledModel {
    extensions: FactoryVecDeque<ExtensionRowModel>,
    has_extensions: bool,
    is_loading: bool,
}

#[derive(Debug)]
pub enum InstalledMsg {
    Refresh,
    Loaded(Vec<Extension>),
    ToggleExtension(ExtensionUuid, bool),
}

#[derive(Debug)]
pub enum InstalledOutput {}

#[relm4::component(pub)]
impl SimpleComponent for InstalledModel {
    type Init = ();
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

    fn init(_: (), root: Self::Root, sender: ComponentSender<Self>) -> ComponentParts<Self> {
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
                // In production, calls ManageUseCase::list_installed_extensions
                let input_sender = sender.input_sender().clone();
                relm4::spawn(async move {
                    tracing::info!("loading installed extensions via D-Bus");
                    input_sender.emit(InstalledMsg::Loaded(vec![]));
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
            InstalledMsg::ToggleExtension(uuid, enabled) => {
                tracing::info!("toggle {uuid} \u{2192} {enabled}");
            }
        }
    }
}
