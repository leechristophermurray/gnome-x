// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Explore view — unified search across extensions, themes, icons, and cursors.
//! Shows popular/recent landing content when idle, search results when active.

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
    // Landing page widgets
    popular_ext_list: gtk::ListBox,
    recent_ext_list: gtk::ListBox,
    popular_themes_list: gtk::ListBox,
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
            "landing"
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
    // Landing data
    LoadLanding,
    LandingPopularExt(Vec<Extension>),
    LandingRecentExt(Vec<Extension>),
    LandingPopularThemes(Vec<ContentItem>),
    // Search results
    LoadExtensions(Vec<Extension>),
    LoadContent(Vec<ContentItem>),
    // Actions
    InstallExtension(ExtensionUuid),
    InstallExtComplete(ExtensionUuid),
    InstallContent(ContentId, u64, String),
    InstallContentComplete(String),
    ApplyContent(String),
    SearchFailed(String),
    ActionFailed(String),
}

#[derive(Debug)]
pub enum ExploreOutput {
    Toast(String),
}

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

        // Content stack
        let content_stack = gtk::Stack::new();

        // --- Landing page ---
        let landing_scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let landing_clamp = adw::Clamp::builder()
            .maximum_size(600)
            .margin_top(12)
            .margin_bottom(24)
            .margin_start(12)
            .margin_end(12)
            .build();
        let landing_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .build();

        // Popular extensions section
        let popular_ext_label = gtk::Label::builder()
            .label("Popular Extensions")
            .halign(gtk::Align::Start)
            .css_classes(["title-3"])
            .build();
        let popular_ext_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        landing_box.append(&popular_ext_label);
        landing_box.append(&popular_ext_list);

        // Recently updated extensions section
        let recent_ext_label = gtk::Label::builder()
            .label("Recently Updated")
            .halign(gtk::Align::Start)
            .css_classes(["title-3"])
            .build();
        let recent_ext_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        landing_box.append(&recent_ext_label);
        landing_box.append(&recent_ext_list);

        // Popular themes section
        let popular_themes_label = gtk::Label::builder()
            .label("Popular Themes")
            .halign(gtk::Align::Start)
            .css_classes(["title-3"])
            .build();
        let popular_themes_list = gtk::ListBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        landing_box.append(&popular_themes_label);
        landing_box.append(&popular_themes_list);

        landing_clamp.set_child(Some(&landing_box));
        landing_scroll.set_child(Some(&landing_clamp));
        content_stack.add_named(&landing_scroll, Some("landing"));

        // --- Loading page ---
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

        // --- Results page ---
        let results_stack = gtk::Stack::new();
        results_stack.set_transition_type(gtk::StackTransitionType::Crossfade);

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

        content_stack.set_visible_child_name("landing");
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
            popular_ext_list: popular_ext_list.clone(),
            recent_ext_list: recent_ext_list.clone(),
            popular_themes_list: popular_themes_list.clone(),
            is_loading: false,
            has_results: false,
        };

        let widgets = view_output!();

        root.insert_child_after(&category_box, root.first_child().as_ref());
        root.append(&content_stack);

        // Kick off landing data load
        sender.input(ExploreMsg::LoadLanding);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ExploreMsg, sender: ComponentSender<Self>) {
        match msg {
            ExploreMsg::LoadLanding => {
                // Fire three parallel requests for landing content
                let browse = self.browse.clone();
                let sender1 = sender.input_sender().clone();
                self.handle.spawn(async move {
                    if let Ok(result) = browse.list_popular().await {
                        let items: Vec<_> = result.items.into_iter().take(8).collect();
                        sender1.emit(ExploreMsg::LandingPopularExt(items));
                    }
                });

                let browse2 = self.browse.clone();
                let sender2 = sender.input_sender().clone();
                self.handle.spawn(async move {
                    if let Ok(result) = browse2.list_recent().await {
                        let items: Vec<_> = result.items.into_iter().take(8).collect();
                        sender2.emit(ExploreMsg::LandingRecentExt(items));
                    }
                });

                let customize = self.customize.clone();
                let sender3 = sender.input_sender().clone();
                self.handle.spawn(async move {
                    if let Ok(result) = customize.list_popular(ContentCategory::GtkTheme).await {
                        let items: Vec<_> = result.items.into_iter().take(8).collect();
                        sender3.emit(ExploreMsg::LandingPopularThemes(items));
                    }
                });
            }
            ExploreMsg::LandingPopularExt(extensions) => {
                populate_ext_landing_list(&self.popular_ext_list, &extensions);
            }
            ExploreMsg::LandingRecentExt(extensions) => {
                populate_ext_landing_list(&self.recent_ext_list, &extensions);
            }
            ExploreMsg::LandingPopularThemes(items) => {
                populate_content_landing_list(&self.popular_themes_list, &items);
            }
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
                            match browse.search_extensions(&query, 1).await {
                                Ok(result) => {
                                    input_sender.emit(ExploreMsg::LoadExtensions(result.items));
                                }
                                Err(e) => {
                                    input_sender.emit(ExploreMsg::SearchFailed(e.to_string()));
                                }
                            }
                        });
                    }
                    ExploreCategory::Content(category) => {
                        let customize = self.customize.clone();
                        self.handle.spawn(async move {
                            match customize.search_content(&query, category, 1).await {
                                Ok(result) => {
                                    input_sender.emit(ExploreMsg::LoadContent(result.items));
                                }
                                Err(e) => {
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
                let _ = sender.output(ExploreOutput::Toast(format!("Search failed: {err}")));
            }
            ExploreMsg::InstallExtension(uuid) => {
                let browse = self.browse.clone();
                let input_sender = sender.input_sender().clone();
                let uuid_clone = uuid.clone();
                self.handle.spawn(async move {
                    match browse.install_extension(&uuid_clone).await {
                        Ok(()) => {
                            input_sender.emit(ExploreMsg::InstallExtComplete(uuid_clone));
                        }
                        Err(e) => {
                            input_sender.emit(ExploreMsg::ActionFailed(e.to_string()));
                        }
                    }
                });
            }
            ExploreMsg::InstallExtComplete(uuid) => {
                let _ = sender.output(ExploreOutput::Toast(format!("Installed {uuid}")));
            }
            ExploreMsg::InstallContent(id, file_id, name) => {
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
                            input_sender.emit(ExploreMsg::ActionFailed(e.to_string()));
                        }
                    }
                });
            }
            ExploreMsg::InstallContentComplete(name) => {
                let _ = sender.output(ExploreOutput::Toast(format!("Installed {name}")));
            }
            ExploreMsg::ApplyContent(name) => {
                let category = match self.category {
                    ExploreCategory::Content(c) => c,
                    _ => return,
                };
                match self.customize.apply_content(&name, category) {
                    Ok(()) => {
                        let _ = sender.output(ExploreOutput::Toast(format!("{name} applied")));
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
                let _ = sender.output(ExploreOutput::Toast(err));
            }
        }
    }
}

/// Populate a landing list box with extension preview rows.
fn populate_ext_landing_list(list: &gtk::ListBox, extensions: &[Extension]) {
    // Clear existing
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for ext in extensions {
        let row = adw::ActionRow::builder()
            .title(&ext.name)
            .subtitle(&format!("by {}", ext.creator))
            .build();
        list.append(&row);
    }
}

/// Populate a landing list box with content item preview rows.
fn populate_content_landing_list(list: &gtk::ListBox, items: &[ContentItem]) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    for item in items {
        let row = adw::ActionRow::builder()
            .title(&item.name)
            .subtitle(&format!("by {}", item.creator))
            .build();
        list.append(&row);
    }
}
