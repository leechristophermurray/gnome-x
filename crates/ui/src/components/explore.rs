// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Explore view — unified search across extensions, themes, icons, and cursors.
//! HIG pattern: linked toggle-button category bar, SearchEntry, boxed-list results.

use crate::components::content_row::{ContentRowInit, ContentRowModel, ContentRowOutput};
use crate::components::extension_row::{
    ExtensionRowInit, ExtensionRowModel, ExtensionRowOutput, RowMode,
};
use crate::services::AppServices;
use adw::prelude::*;
use gnomex_app::use_cases::{BrowseUseCase, CustomizeUseCase};
use gnomex_domain::{ContentCategory, ContentId, ContentItem, Extension, ExtensionUuid};
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;
use std::sync::Arc;
use tokio::runtime::Handle;

/// Which kind of content the user is searching for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ExploreCategory {
    Extensions,
    Content(ContentCategory),
}

pub struct ExploreModel {
    handle: Handle,
    browse: Arc<BrowseUseCase>,
    customize: Arc<CustomizeUseCase>,
    category: ExploreCategory,
    search_query: String,
    ext_results: FactoryVecDeque<ExtensionRowModel>,
    content_results: FactoryVecDeque<ContentRowModel>,
    content_stack: gtk::Stack,
    results_stack: gtk::Stack,
    is_loading: bool,
    has_results: bool,
}

impl ExploreModel {
    fn update_stack(&self) {
        let name = if self.is_loading {
            "loading"
        } else if self.has_results {
            "results"
        } else {
            "empty"
        };
        self.content_stack.set_visible_child_name(name);
    }

    fn show_result_list(&self) {
        let page = if self.category == ExploreCategory::Extensions {
            "extensions"
        } else {
            "content"
        };
        self.results_stack.set_visible_child_name(page);
    }
}

#[derive(Debug)]
pub enum ExploreMsg {
    SetCategory(ExploreCategory),
    SearchChanged(String),
    SearchActivated,
    // Extension results
    LoadExtensions(Vec<Extension>),
    InstallExtension(ExtensionUuid),
    InstallExtComplete(ExtensionUuid),
    // Content results
    LoadContent(Vec<ContentItem>),
    InstallContent(ContentId, u64, String),
    InstallContentComplete(String),
    ApplyContent(String),
    // Shared
    SearchFailed(String),
    ActionFailed(String),
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
                set_spacing: 12,

                gtk::SearchEntry {
                    set_placeholder_text: Some("Search\u{2026}"),
                    set_hexpand: true,
                    connect_search_changed[sender] => move |entry| {
                        sender.input(ExploreMsg::SearchChanged(entry.text().to_string()));
                    },
                    connect_activate[sender] => move |_| {
                        sender.input(ExploreMsg::SearchActivated);
                    },
                },
            },
        }
    }

    fn init(
        services: AppServices,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        // Extension results factory
        let ext_results =
            FactoryVecDeque::builder()
                .launch(gtk::ListBox::default())
                .forward(sender.input_sender(), |output| match output {
                    ExtensionRowOutput::Install(uuid) => ExploreMsg::InstallExtension(uuid),
                    ExtensionRowOutput::Toggle(_, _) => unreachable!(),
                });

        // Content results factory
        let content_results =
            FactoryVecDeque::builder()
                .launch(gtk::ListBox::default())
                .forward(sender.input_sender(), |output| match output {
                    ContentRowOutput::Install(id, file_id, name) => {
                        ExploreMsg::InstallContent(id, file_id, name)
                    }
                    ContentRowOutput::Apply(name) => ExploreMsg::ApplyContent(name),
                });

        // Category toggle bar
        let category_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .halign(gtk::Align::Center)
            .spacing(0)
            .css_classes(["linked"])
            .build();

        let categories: &[(&str, ExploreCategory)] = &[
            ("Extensions", ExploreCategory::Extensions),
            ("GTK Themes", ExploreCategory::Content(ContentCategory::GtkTheme)),
            ("Shell Themes", ExploreCategory::Content(ContentCategory::ShellTheme)),
            ("Icons", ExploreCategory::Content(ContentCategory::IconTheme)),
            ("Cursors", ExploreCategory::Content(ContentCategory::CursorTheme)),
        ];

        for (i, &(label, cat)) in categories.iter().enumerate() {
            let btn = gtk::ToggleButton::builder()
                .label(label)
                .active(i == 0)
                .build();
            if i > 0 {
                if let Some(first) = category_box.first_child() {
                    btn.set_group(first.downcast_ref::<gtk::ToggleButton>());
                }
            }
            let sender = sender.clone();
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    sender.input(ExploreMsg::SetCategory(cat));
                }
            });
            category_box.append(&btn);
        }

        // Content stack (empty / loading / results)
        let content_stack = gtk::Stack::new();

        let empty_page = adw::StatusPage::builder()
            .icon_name("system-search-symbolic")
            .title("Explore")
            .description("Search for extensions, themes, icons, and cursors")
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
            .label("Searching\u{2026}")
            .css_classes(["dim-label"])
            .build();
        loading_box.append(&loading_label);
        content_stack.add_named(&loading_box, Some("loading"));

        // Results page contains a sub-stack switching between extension list and content list
        let results_stack = gtk::Stack::new();
        results_stack.set_transition_type(gtk::StackTransitionType::Crossfade);

        // Extension results scroll
        let ext_scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let ext_clamp = adw::Clamp::builder()
            .maximum_size(600)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        {
            let w: &gtk::ListBox = ext_results.widget();
            w.set_selection_mode(gtk::SelectionMode::None);
            w.add_css_class("boxed-list");
            ext_clamp.set_child(Some(w));
        }
        ext_scroll.set_child(Some(&ext_clamp));
        results_stack.add_named(&ext_scroll, Some("extensions"));

        // Content results scroll
        let content_scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let content_clamp = adw::Clamp::builder()
            .maximum_size(600)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        {
            let w: &gtk::ListBox = content_results.widget();
            w.set_selection_mode(gtk::SelectionMode::None);
            w.add_css_class("boxed-list");
            content_clamp.set_child(Some(w));
        }
        content_scroll.set_child(Some(&content_clamp));
        results_stack.add_named(&content_scroll, Some("content"));

        results_stack.set_visible_child_name("extensions");
        content_stack.add_named(&results_stack, Some("results"));

        content_stack.set_visible_child_name("empty");
        content_stack.set_vexpand(true);

        let model = ExploreModel {
            handle: services.handle,
            browse: services.browse,
            customize: services.customize,
            category: ExploreCategory::Extensions,
            search_query: String::new(),
            ext_results,
            content_results,
            content_stack: content_stack.clone(),
            results_stack: results_stack.clone(),
            is_loading: false,
            has_results: false,
        };

        let widgets = view_output!();

        // Insert category bar between the search box and the content stack
        root.insert_child_after(&category_box, root.first_child().as_ref());
        root.append(&content_stack);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ExploreMsg, sender: ComponentSender<Self>) {
        match msg {
            ExploreMsg::SetCategory(cat) => {
                self.category = cat;
                self.show_result_list();
                if !self.search_query.trim().is_empty() {
                    sender.input(ExploreMsg::SearchActivated);
                }
            }
            ExploreMsg::SearchChanged(query) => {
                self.search_query = query;
                if self.search_query.trim().is_empty() {
                    self.has_results = false;
                    self.is_loading = false;
                    self.ext_results.guard().clear();
                    self.content_results.guard().clear();
                    self.update_stack();
                }
            }
            ExploreMsg::SearchActivated => {
                if self.search_query.trim().is_empty() {
                    return;
                }
                self.is_loading = true;
                self.has_results = false;
                self.update_stack();
                self.show_result_list();

                let query = self.search_query.clone();
                let input_sender = sender.input_sender().clone();

                match self.category {
                    ExploreCategory::Extensions => {
                        let browse = self.browse.clone();
                        self.handle.spawn(async move {
                            tracing::info!("searching extensions: {query}");
                            match browse.search_extensions(&query, 1).await {
                                Ok(result) => {
                                    input_sender.emit(ExploreMsg::LoadExtensions(result.items));
                                }
                                Err(e) => {
                                    tracing::error!("search failed: {e}");
                                    input_sender.emit(ExploreMsg::SearchFailed(e.to_string()));
                                }
                            }
                        });
                    }
                    ExploreCategory::Content(category) => {
                        let customize = self.customize.clone();
                        self.handle.spawn(async move {
                            tracing::info!("searching {category:?}: {query}");
                            match customize.search_content(&query, category, 1).await {
                                Ok(result) => {
                                    input_sender.emit(ExploreMsg::LoadContent(result.items));
                                }
                                Err(e) => {
                                    tracing::error!("search failed: {e}");
                                    input_sender.emit(ExploreMsg::SearchFailed(e.to_string()));
                                }
                            }
                        });
                    }
                }
            }
            ExploreMsg::LoadExtensions(extensions) => {
                self.is_loading = false;
                self.has_results = !extensions.is_empty();
                self.update_stack();

                let mut guard = self.ext_results.guard();
                guard.clear();
                for ext in extensions {
                    guard.push_back(ExtensionRowInit {
                        name: ext.name,
                        uuid: ext.uuid.as_str().to_owned(),
                        creator: ext.creator,
                        description: ext.description,
                        state: ext.state,
                        mode: RowMode::Explore,
                    });
                }
            }
            ExploreMsg::LoadContent(items) => {
                self.is_loading = false;
                self.has_results = !items.is_empty();
                self.update_stack();

                let mut guard = self.content_results.guard();
                guard.clear();
                for item in items {
                    guard.push_back(ContentRowInit {
                        name: item.name,
                        creator: item.creator,
                        id: item.id.0,
                        file_id: 1,
                        state: item.state,
                    });
                }
            }
            ExploreMsg::SearchFailed(err) => {
                self.is_loading = false;
                self.has_results = false;
                self.update_stack();
                tracing::warn!("search error: {err}");
            }
            ExploreMsg::InstallExtension(uuid) => {
                tracing::info!("installing extension: {uuid}");
                let browse = self.browse.clone();
                let input_sender = sender.input_sender().clone();
                let uuid_clone = uuid.clone();
                self.handle.spawn(async move {
                    match browse.install_extension(&uuid_clone).await {
                        Ok(()) => {
                            input_sender.emit(ExploreMsg::InstallExtComplete(uuid_clone));
                        }
                        Err(e) => {
                            tracing::error!("install failed: {e}");
                            input_sender.emit(ExploreMsg::ActionFailed(e.to_string()));
                        }
                    }
                });
            }
            ExploreMsg::InstallExtComplete(uuid) => {
                tracing::info!("extension {uuid} installed");
            }
            ExploreMsg::InstallContent(id, file_id, name) => {
                tracing::info!("installing content: {name}");
                let customize = self.customize.clone();
                let category = match self.category {
                    ExploreCategory::Content(c) => c,
                    _ => return,
                };
                let input_sender = sender.input_sender().clone();
                let name_clone = name.clone();
                self.handle.spawn(async move {
                    match customize
                        .install_content(id, file_id, &name_clone, category)
                        .await
                    {
                        Ok(()) => {
                            input_sender.emit(ExploreMsg::InstallContentComplete(name_clone));
                        }
                        Err(e) => {
                            tracing::error!("install failed: {e}");
                            input_sender.emit(ExploreMsg::ActionFailed(e.to_string()));
                        }
                    }
                });
            }
            ExploreMsg::InstallContentComplete(name) => {
                tracing::info!("content {name} installed");
            }
            ExploreMsg::ApplyContent(name) => {
                tracing::info!("applying: {name}");
                let category = match self.category {
                    ExploreCategory::Content(c) => c,
                    _ => return,
                };
                match self.customize.apply_content(&name, category) {
                    Ok(()) => {
                        tracing::info!("{name} applied");
                    }
                    Err(e) => {
                        sender
                            .input_sender()
                            .emit(ExploreMsg::ActionFailed(e.to_string()));
                    }
                }
            }
            ExploreMsg::ActionFailed(err) => {
                tracing::warn!("action error: {err}");
            }
        }
    }
}
