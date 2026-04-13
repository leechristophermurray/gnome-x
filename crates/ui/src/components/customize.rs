// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Customize view — browse and install themes, icons, and cursors from
//! gnome-look.org. HIG pattern: category toggle row + SearchEntry + boxed-list.

use crate::components::content_row::{ContentRowInit, ContentRowModel, ContentRowOutput};
use crate::services::AppServices;
use adw::prelude::*;
use gnomex_app::use_cases::CustomizeUseCase;
use gnomex_domain::{ContentCategory, ContentId, ContentItem};
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;
use std::sync::Arc;
use tokio::runtime::Handle;

pub struct CustomizeModel {
    handle: Handle,
    customize: Arc<CustomizeUseCase>,
    category: ContentCategory,
    search_query: String,
    results: FactoryVecDeque<ContentRowModel>,
    content_stack: gtk::Stack,
    is_loading: bool,
    has_results: bool,
}

impl CustomizeModel {
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
}

#[derive(Debug)]
pub enum CustomizeMsg {
    SetCategory(ContentCategory),
    SearchChanged(String),
    SearchActivated,
    LoadResults(Vec<ContentItem>),
    SearchFailed(String),
    InstallContent(ContentId, u64, String),
    InstallComplete(String),
    InstallFailed(String),
    ApplyContent(String),
    ApplyComplete(String),
    ApplyFailed(String),
}

#[derive(Debug)]
pub enum CustomizeOutput {}

#[relm4::component(pub)]
impl SimpleComponent for CustomizeModel {
    type Init = AppServices;
    type Input = CustomizeMsg;
    type Output = CustomizeOutput;

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
                    set_placeholder_text: Some("Search themes, icons, cursors\u{2026}"),
                    set_hexpand: true,
                    connect_search_changed[sender] => move |entry| {
                        sender.input(CustomizeMsg::SearchChanged(entry.text().to_string()));
                    },
                    connect_activate[sender] => move |_| {
                        sender.input(CustomizeMsg::SearchActivated);
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
        let results =
            FactoryVecDeque::builder()
                .launch(gtk::ListBox::default())
                .forward(sender.input_sender(), |output| match output {
                    ContentRowOutput::Install(id, file_id, name) => {
                        CustomizeMsg::InstallContent(id, file_id, name)
                    }
                    ContentRowOutput::Apply(name) => CustomizeMsg::ApplyContent(name),
                });

        // Category toggle row
        let category_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .halign(gtk::Align::Center)
            .spacing(0)
            .css_classes(["linked"])
            .build();

        let categories = [
            ("GTK Themes", ContentCategory::GtkTheme),
            ("Shell Themes", ContentCategory::ShellTheme),
            ("Icons", ContentCategory::IconTheme),
            ("Cursors", ContentCategory::CursorTheme),
        ];

        for (i, (label, cat)) in categories.iter().enumerate() {
            let btn = gtk::ToggleButton::builder()
                .label(*label)
                .active(i == 0)
                .build();
            if i > 0 {
                // Link all buttons to the first one for radio-group behavior
                if let Some(first) = category_box.first_child() {
                    btn.set_group(first.downcast_ref::<gtk::ToggleButton>());
                }
            }
            let sender = sender.clone();
            let cat = *cat;
            btn.connect_toggled(move |b| {
                if b.is_active() {
                    sender.input(CustomizeMsg::SetCategory(cat));
                }
            });
            category_box.append(&btn);
        }

        // Content stack
        let content_stack = gtk::Stack::new();

        let empty_page = adw::StatusPage::builder()
            .icon_name("applications-graphics-symbolic")
            .title("Customize Your Desktop")
            .description("Search for themes, icons, and cursors to install")
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

        let results_scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let results_clamp = adw::Clamp::builder()
            .maximum_size(600)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(12)
            .margin_end(12)
            .build();
        {
            let w: &gtk::ListBox = results.widget();
            w.set_selection_mode(gtk::SelectionMode::None);
            w.add_css_class("boxed-list");
            results_clamp.set_child(Some(w));
        }
        results_scroll.set_child(Some(&results_clamp));
        content_stack.add_named(&results_scroll, Some("results"));

        content_stack.set_visible_child_name("empty");
        content_stack.set_vexpand(true);

        let model = CustomizeModel {
            handle: services.handle,
            customize: services.customize,
            category: ContentCategory::GtkTheme,
            search_query: String::new(),
            results,
            content_stack: content_stack.clone(),
            is_loading: false,
            has_results: false,
        };

        let widgets = view_output!();

        // Insert category bar and stack into root
        root.insert_child_after(&category_box, root.first_child().as_ref());
        root.append(&content_stack);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: CustomizeMsg, sender: ComponentSender<Self>) {
        match msg {
            CustomizeMsg::SetCategory(cat) => {
                self.category = cat;
                // Re-search if there's an active query
                if !self.search_query.trim().is_empty() {
                    sender.input(CustomizeMsg::SearchActivated);
                }
            }
            CustomizeMsg::SearchChanged(query) => {
                self.search_query = query;
                if self.search_query.trim().is_empty() {
                    self.has_results = false;
                    self.is_loading = false;
                    self.results.guard().clear();
                    self.update_stack();
                }
            }
            CustomizeMsg::SearchActivated => {
                if self.search_query.trim().is_empty() {
                    return;
                }
                self.is_loading = true;
                self.has_results = false;
                self.update_stack();

                let customize = self.customize.clone();
                let query = self.search_query.clone();
                let category = self.category;
                let input_sender = sender.input_sender().clone();
                self.handle.spawn(async move {
                    tracing::info!("searching OCS for {category:?}: {query}");
                    match customize.search_content(&query, category, 1).await {
                        Ok(result) => {
                            tracing::info!("found {} items", result.items.len());
                            input_sender.emit(CustomizeMsg::LoadResults(result.items));
                        }
                        Err(e) => {
                            tracing::error!("search failed: {e}");
                            input_sender.emit(CustomizeMsg::SearchFailed(e.to_string()));
                        }
                    }
                });
            }
            CustomizeMsg::LoadResults(items) => {
                self.is_loading = false;
                self.has_results = !items.is_empty();
                self.update_stack();

                let mut guard = self.results.guard();
                guard.clear();
                for item in items {
                    guard.push_back(ContentRowInit {
                        name: item.name,
                        creator: item.creator,
                        id: item.id.0,
                        file_id: 1, // OCS default first download file
                        state: item.state,
                    });
                }
            }
            CustomizeMsg::SearchFailed(err) => {
                self.is_loading = false;
                self.has_results = false;
                self.update_stack();
                tracing::warn!("search error: {err}");
            }
            CustomizeMsg::InstallContent(id, file_id, name) => {
                tracing::info!("installing content: {name}");
                let customize = self.customize.clone();
                let category = self.category;
                let input_sender = sender.input_sender().clone();
                let name_clone = name.clone();
                self.handle.spawn(async move {
                    match customize
                        .install_content(id, file_id, &name_clone, category)
                        .await
                    {
                        Ok(()) => {
                            tracing::info!("installed {name_clone}");
                            input_sender.emit(CustomizeMsg::InstallComplete(name_clone));
                        }
                        Err(e) => {
                            tracing::error!("install failed: {e}");
                            input_sender.emit(CustomizeMsg::InstallFailed(e.to_string()));
                        }
                    }
                });
            }
            CustomizeMsg::InstallComplete(name) => {
                tracing::info!("content {name} installed successfully");
            }
            CustomizeMsg::InstallFailed(err) => {
                tracing::warn!("install error: {err}");
            }
            CustomizeMsg::ApplyContent(name) => {
                tracing::info!("applying content: {name}");
                let category = self.category;
                match self.customize.apply_content(&name, category) {
                    Ok(()) => {
                        sender
                            .input_sender()
                            .emit(CustomizeMsg::ApplyComplete(name));
                    }
                    Err(e) => {
                        sender
                            .input_sender()
                            .emit(CustomizeMsg::ApplyFailed(e.to_string()));
                    }
                }
            }
            CustomizeMsg::ApplyComplete(name) => {
                tracing::info!("{name} applied");
            }
            CustomizeMsg::ApplyFailed(err) => {
                tracing::warn!("apply error: {err}");
            }
        }
    }
}
