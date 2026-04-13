// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Explore view — search and browse extensions from extensions.gnome.org.
//! Follows HIG: SearchEntry at top, AdwStatusPage for empty/loading states,
//! GtkListBox with .boxed-list for results.

use crate::components::extension_row::{ExtensionRowInit, ExtensionRowModel, ExtensionRowOutput};
use crate::services::AppServices;
use adw::prelude::*;
use gnomex_app::use_cases::BrowseUseCase;
use gnomex_domain::{Extension, ExtensionUuid};
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;
use std::sync::Arc;
use tokio::runtime::Handle;

pub struct ExploreModel {
    handle: Handle,
    browse: Arc<BrowseUseCase>,
    search_query: String,
    results: FactoryVecDeque<ExtensionRowModel>,
    is_loading: bool,
    has_results: bool,
}

#[derive(Debug)]
pub enum ExploreMsg {
    SearchChanged(String),
    SearchActivated,
    LoadResults(Vec<Extension>),
    SearchFailed(String),
    InstallExtension(ExtensionUuid),
    InstallComplete(ExtensionUuid),
    InstallFailed(String),
}

#[derive(Debug)]
pub enum ExploreOutput {}

#[relm4::component(pub)]
impl SimpleComponent for ExploreModel {
    type Init = AppServices;
    type Input = ExploreMsg;
    type Output = ExploreOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,

            gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                set_margin_top: 24,
                set_margin_bottom: 12,
                set_margin_start: 12,
                set_margin_end: 12,
                set_halign: gtk::Align::Center,
                set_width_request: 600,

                gtk::SearchEntry {
                    set_placeholder_text: Some("Search extensions\u{2026}"),
                    set_hexpand: true,
                    connect_search_changed[sender] => move |entry| {
                        sender.input(ExploreMsg::SearchChanged(entry.text().to_string()));
                    },
                    connect_activate[sender] => move |_| {
                        sender.input(ExploreMsg::SearchActivated);
                    },
                },
            },

            #[name = "content_stack"]
            gtk::Stack {
                #[watch]
                set_visible_child_name: if model.is_loading {
                    "loading"
                } else if model.has_results {
                    "results"
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
        let results =
            FactoryVecDeque::builder()
                .launch(gtk::ListBox::default())
                .forward(sender.input_sender(), |output| match output {
                    ExtensionRowOutput::Install(uuid) => ExploreMsg::InstallExtension(uuid),
                    ExtensionRowOutput::Toggle(_, _) => unreachable!(),
                });

        let model = ExploreModel {
            handle: services.handle,
            browse: services.browse,
            search_query: String::new(),
            results,
            is_loading: false,
            has_results: false,
        };

        let widgets = view_output!();

        // Build stack pages programmatically
        let empty_page = adw::StatusPage::builder()
            .icon_name("system-search-symbolic")
            .title("Explore Extensions")
            .description("Search for GNOME Shell extensions to install")
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
            .label("Searching\u{2026}")
            .css_classes(["dim-label"])
            .build();
        loading_box.append(&loading_label);
        widgets
            .content_stack
            .add_named(&loading_box, Some("loading"));

        let results_scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let results_clamp = adw::Clamp::builder()
            .maximum_size(600)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        let results_list = model.results.widget().clone();
        results_list.set_selection_mode(gtk::SelectionMode::None);
        results_list.add_css_class("boxed-list");
        results_clamp.set_child(Some(&results_list));
        results_scroll.set_child(Some(&results_clamp));
        widgets
            .content_stack
            .add_named(&results_scroll, Some("results"));

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ExploreMsg, sender: ComponentSender<Self>) {
        match msg {
            ExploreMsg::SearchChanged(query) => {
                self.search_query = query;
            }
            ExploreMsg::SearchActivated => {
                if self.search_query.trim().is_empty() {
                    return;
                }
                self.is_loading = true;
                self.has_results = false;

                let browse = self.browse.clone();
                let query = self.search_query.clone();
                let input_sender = sender.input_sender().clone();
                self.handle.spawn(async move {
                    tracing::info!("searching EGO for: {query}");
                    match browse.search_extensions(&query, 1).await {
                        Ok(result) => {
                            tracing::info!("found {} extensions", result.items.len());
                            input_sender.emit(ExploreMsg::LoadResults(result.items));
                        }
                        Err(e) => {
                            tracing::error!("search failed: {e}");
                            input_sender.emit(ExploreMsg::SearchFailed(e.to_string()));
                        }
                    }
                });
            }
            ExploreMsg::LoadResults(extensions) => {
                self.is_loading = false;
                self.has_results = !extensions.is_empty();

                let mut guard = self.results.guard();
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
            ExploreMsg::SearchFailed(err) => {
                self.is_loading = false;
                self.has_results = false;
                tracing::warn!("search error displayed to user: {err}");
            }
            ExploreMsg::InstallExtension(uuid) => {
                tracing::info!("installing extension: {uuid}");
                let browse = self.browse.clone();
                let input_sender = sender.input_sender().clone();
                let uuid_clone = uuid.clone();
                self.handle.spawn(async move {
                    match browse.install_extension(&uuid_clone).await {
                        Ok(()) => {
                            tracing::info!("installed {uuid_clone}");
                            input_sender.emit(ExploreMsg::InstallComplete(uuid_clone));
                        }
                        Err(e) => {
                            tracing::error!("install failed: {e}");
                            input_sender.emit(ExploreMsg::InstallFailed(e.to_string()));
                        }
                    }
                });
            }
            ExploreMsg::InstallComplete(uuid) => {
                tracing::info!("extension {uuid} installed successfully");
                // TODO: show AdwToast confirmation
            }
            ExploreMsg::InstallFailed(err) => {
                tracing::warn!("install error: {err}");
                // TODO: show AdwToast error
            }
        }
    }
}
