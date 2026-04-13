// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Packs view — create, list, import/export, and apply Experience Packs.
//! HIG pattern: boxed-list with AdwActionRow, AdwStatusPage for empty state,
//! toolbar buttons for New Snapshot and Import.

use crate::components::pack_row::{PackRowInit, PackRowModel, PackRowOutput};
use crate::services::AppServices;
use adw::prelude::*;
use gnomex_app::ports::PackSummary;
use gnomex_app::use_cases::PacksUseCase;
use relm4::factory::FactoryVecDeque;
use relm4::prelude::*;
use std::sync::Arc;
use tokio::runtime::Handle;

pub struct PacksModel {
    handle: Handle,
    packs_uc: Arc<PacksUseCase>,
    packs: FactoryVecDeque<PackRowModel>,
    content_stack: gtk::Stack,
    has_packs: bool,
    is_loading: bool,
    dialog_name: String,
    dialog_desc: String,
    dialog_author: String,
}

impl PacksModel {
    fn update_stack(&self) {
        let name = if self.is_loading {
            "loading"
        } else if self.has_packs {
            "list"
        } else {
            "empty"
        };
        self.content_stack.set_visible_child_name(name);
    }

    fn get_window(&self) -> Option<gtk::Window> {
        self.content_stack
            .root()
            .and_then(|r| r.downcast::<gtk::Window>().ok())
    }
}

#[derive(Debug)]
pub enum PacksMsg {
    Refresh,
    Loaded(Vec<PackSummary>),
    LoadFailed(String),
    ShowCreateDialog,
    CreatePack,
    SetDialogName(String),
    SetDialogDesc(String),
    SetDialogAuthor(String),
    PackCreated(String),
    ApplyPack(String),
    ApplyComplete(String),
    ExportPack(String, String), // (id, suggested filename)
    ExportReady(Vec<u8>, String), // (archive bytes, filename)
    ImportPack,
    ImportFile(Vec<u8>),
    Imported(String),
    DeletePack(String),
    Deleted(String),
    ActionFailed(String),
}

#[derive(Debug)]
pub enum PacksOutput {
    Toast(String),
}

#[relm4::component(pub)]
impl SimpleComponent for PacksModel {
    type Init = AppServices;
    type Input = PacksMsg;
    type Output = PacksOutput;

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
        let packs =
            FactoryVecDeque::builder()
                .launch(gtk::ListBox::default())
                .forward(sender.input_sender(), |output| match output {
                    PackRowOutput::Apply(id) => PacksMsg::ApplyPack(id),
                    PackRowOutput::Export(id, name) => PacksMsg::ExportPack(id, name),
                    PackRowOutput::Delete(id) => PacksMsg::DeletePack(id),
                });

        // Toolbar with New Snapshot + Import
        let toolbar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .halign(gtk::Align::End)
            .margin_top(12)
            .margin_end(24)
            .spacing(8)
            .build();

        let import_btn = gtk::Button::builder()
            .label("Import")
            .css_classes(["flat"])
            .build();
        {
            let sender = sender.clone();
            import_btn.connect_clicked(move |_| {
                sender.input(PacksMsg::ImportPack);
            });
        }
        toolbar.append(&import_btn);

        let create_btn = gtk::Button::builder()
            .label("New Snapshot")
            .css_classes(["suggested-action", "pill"])
            .build();
        {
            let sender = sender.clone();
            create_btn.connect_clicked(move |_| {
                sender.input(PacksMsg::ShowCreateDialog);
            });
        }
        toolbar.append(&create_btn);

        // Content stack
        let content_stack = gtk::Stack::new();

        let empty_page = adw::StatusPage::builder()
            .icon_name("package-x-generic-symbolic")
            .title("Experience Packs")
            .description("Snapshot your desktop configuration to share or restore later.\nClick \u{201c}New Snapshot\u{201d} to get started.")
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
            .label("Loading packs\u{2026}")
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
            .label("Saved Packs")
            .halign(gtk::Align::Start)
            .css_classes(["title-2"])
            .build();
        list_box_container.append(&title);
        {
            let w: &gtk::ListBox = packs.widget();
            w.set_selection_mode(gtk::SelectionMode::None);
            w.add_css_class("boxed-list");
            list_box_container.append(w);
        }
        list_clamp.set_child(Some(&list_box_container));
        list_scroll.set_child(Some(&list_clamp));
        content_stack.add_named(&list_scroll, Some("list"));

        content_stack.set_visible_child_name("loading");
        content_stack.set_vexpand(true);

        let model = PacksModel {
            handle: services.handle,
            packs_uc: services.packs,
            packs,
            content_stack: content_stack.clone(),
            has_packs: false,
            is_loading: true,
            dialog_name: String::new(),
            dialog_desc: String::new(),
            dialog_author: String::new(),
        };

        let widgets = view_output!();

        root.append(&toolbar);
        root.append(&content_stack);

        sender.input(PacksMsg::Refresh);

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: PacksMsg, sender: ComponentSender<Self>) {
        match msg {
            PacksMsg::Refresh => {
                self.is_loading = true;
                self.update_stack();

                match self.packs_uc.list_packs() {
                    Ok(summaries) => {
                        sender.input_sender().emit(PacksMsg::Loaded(summaries));
                    }
                    Err(e) => {
                        sender
                            .input_sender()
                            .emit(PacksMsg::LoadFailed(e.to_string()));
                    }
                }
            }
            PacksMsg::Loaded(summaries) => {
                self.is_loading = false;
                self.has_packs = !summaries.is_empty();
                self.update_stack();

                let mut guard = self.packs.guard();
                guard.clear();
                for s in summaries {
                    guard.push_back(PackRowInit {
                        id: s.id,
                        name: s.name,
                        author: s.author,
                        description: s.description,
                    });
                }
            }
            PacksMsg::LoadFailed(err) => {
                self.is_loading = false;
                self.has_packs = false;
                self.update_stack();
                tracing::warn!("load packs failed: {err}");
            }
            PacksMsg::ShowCreateDialog => {
                let dialog = adw::AlertDialog::builder()
                    .heading("New Experience Pack")
                    .body("Snapshot your current theme, icons, cursors, and enabled extensions.")
                    .close_response("cancel")
                    .default_response("create")
                    .build();
                dialog.add_response("cancel", "Cancel");
                dialog.add_response("create", "Create");
                dialog.set_response_appearance("create", adw::ResponseAppearance::Suggested);

                let fields = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .spacing(12)
                    .margin_top(12)
                    .build();

                let name_row = adw::EntryRow::builder().title("Name").build();
                let desc_row = adw::EntryRow::builder().title("Description").build();
                let author_row = adw::EntryRow::builder().title("Author").build();

                let field_list = gtk::ListBox::builder()
                    .selection_mode(gtk::SelectionMode::None)
                    .css_classes(["boxed-list"])
                    .build();
                field_list.append(&name_row);
                field_list.append(&desc_row);
                field_list.append(&author_row);
                fields.append(&field_list);

                dialog.set_extra_child(Some(&fields));

                let sender = sender.clone();
                let name_row_ref = name_row.clone();
                let desc_row_ref = desc_row.clone();
                let author_row_ref = author_row.clone();

                dialog.connect_response(None, move |_, response| {
                    if response == "create" {
                        let name = name_row_ref.text().to_string();
                        let desc = desc_row_ref.text().to_string();
                        let author = author_row_ref.text().to_string();

                        if !name.trim().is_empty() {
                            sender.input(PacksMsg::SetDialogName(name));
                            sender.input(PacksMsg::SetDialogDesc(desc));
                            sender.input(PacksMsg::SetDialogAuthor(author));
                            sender.input(PacksMsg::CreatePack);
                        }
                    }
                });

                if let Some(window) = self.get_window() {
                    dialog.present(Some(&window));
                }
            }
            PacksMsg::SetDialogName(n) => self.dialog_name = n,
            PacksMsg::SetDialogDesc(d) => self.dialog_desc = d,
            PacksMsg::SetDialogAuthor(a) => self.dialog_author = a,
            PacksMsg::CreatePack => {
                let packs_uc = self.packs_uc.clone();
                let name = self.dialog_name.clone();
                let desc = self.dialog_desc.clone();
                let author = self.dialog_author.clone();
                let input_sender = sender.input_sender().clone();

                self.handle.spawn(async move {
                    tracing::info!("snapshotting current desktop as \"{name}\"");
                    match packs_uc.snapshot_current(name, desc, author).await {
                        Ok(id) => {
                            tracing::info!("pack created: {id}");
                            input_sender.emit(PacksMsg::PackCreated(id));
                        }
                        Err(e) => {
                            tracing::error!("snapshot failed: {e}");
                            input_sender.emit(PacksMsg::ActionFailed(e.to_string()));
                        }
                    }
                });
            }
            PacksMsg::PackCreated(_id) => {
                let _ = sender.output(PacksOutput::Toast("Pack created".into()));
                sender.input(PacksMsg::Refresh);
            }
            PacksMsg::ApplyPack(id) => {
                tracing::info!("applying pack: {id}");
                let packs_uc = self.packs_uc.clone();
                let input_sender = sender.input_sender().clone();
                let id_clone = id.clone();
                self.handle.spawn(async move {
                    match packs_uc.apply_pack(&id_clone).await {
                        Ok(()) => {
                            input_sender.emit(PacksMsg::ApplyComplete(id_clone));
                        }
                        Err(e) => {
                            tracing::error!("apply failed: {e}");
                            input_sender.emit(PacksMsg::ActionFailed(e.to_string()));
                        }
                    }
                });
            }
            PacksMsg::ApplyComplete(id) => {
                tracing::info!("pack {id} applied");
                let _ = sender.output(PacksOutput::Toast("Pack applied".into()));
            }
            PacksMsg::ExportPack(id, filename) => {
                tracing::info!("exporting pack: {id}");
                // Export without screenshot for now (screenshot capture is a future enhancement)
                match self.packs_uc.export_pack(&id, None) {
                    Ok(archive) => {
                        sender
                            .input_sender()
                            .emit(PacksMsg::ExportReady(archive, filename));
                    }
                    Err(e) => {
                        sender
                            .input_sender()
                            .emit(PacksMsg::ActionFailed(e.to_string()));
                    }
                }
            }
            PacksMsg::ExportReady(archive, filename) => {
                let dialog = gtk::FileDialog::builder()
                    .title("Export Experience Pack")
                    .initial_name(&filename)
                    .build();

                let sender = sender.clone();
                if let Some(window) = self.get_window() {
                    dialog.save(
                        Some(&window),
                        gio::Cancellable::NONE,
                        move |result| {
                            if let Ok(file) = result {
                                if let Some(path) = file.path() {
                                    match std::fs::write(&path, &archive) {
                                        Ok(()) => {
                                            let _ = sender.output(PacksOutput::Toast(
                                                format!("Exported to {}", path.display()),
                                            ));
                                        }
                                        Err(e) => {
                                            sender
                                                .input_sender()
                                                .emit(PacksMsg::ActionFailed(e.to_string()));
                                        }
                                    }
                                }
                            }
                        },
                    );
                }
            }
            PacksMsg::ImportPack => {
                let filter = gtk::FileFilter::new();
                filter.add_pattern("*.gnomex-pack.tar.gz");
                filter.add_pattern("*.tar.gz");
                filter.set_name(Some("Experience Packs"));

                let filters = gio::ListStore::new::<gtk::FileFilter>();
                filters.append(&filter);

                let dialog = gtk::FileDialog::builder()
                    .title("Import Experience Pack")
                    .filters(&filters)
                    .build();

                let sender = sender.clone();
                if let Some(window) = self.get_window() {
                    dialog.open(
                        Some(&window),
                        gio::Cancellable::NONE,
                        move |result| {
                            if let Ok(file) = result {
                                if let Some(path) = file.path() {
                                    match std::fs::read(&path) {
                                        Ok(data) => {
                                            sender.input(PacksMsg::ImportFile(data));
                                        }
                                        Err(e) => {
                                            sender
                                                .input_sender()
                                                .emit(PacksMsg::ActionFailed(e.to_string()));
                                        }
                                    }
                                }
                            }
                        },
                    );
                }
            }
            PacksMsg::ImportFile(data) => {
                match self.packs_uc.import_pack(&data) {
                    Ok((id, _screenshot)) => {
                        sender.input_sender().emit(PacksMsg::Imported(id));
                    }
                    Err(e) => {
                        sender
                            .input_sender()
                            .emit(PacksMsg::ActionFailed(e.to_string()));
                    }
                }
            }
            PacksMsg::Imported(_id) => {
                let _ = sender.output(PacksOutput::Toast("Pack imported".into()));
                sender.input(PacksMsg::Refresh);
            }
            PacksMsg::DeletePack(id) => {
                tracing::info!("deleting pack: {id}");
                match self.packs_uc.delete_pack(&id) {
                    Ok(()) => {
                        sender.input_sender().emit(PacksMsg::Deleted(id));
                    }
                    Err(e) => {
                        sender
                            .input_sender()
                            .emit(PacksMsg::ActionFailed(e.to_string()));
                    }
                }
            }
            PacksMsg::Deleted(_id) => {
                let _ = sender.output(PacksOutput::Toast("Pack deleted".into()));
                sender.input(PacksMsg::Refresh);
            }
            PacksMsg::ActionFailed(err) => {
                tracing::warn!("pack action error: {err}");
                let _ = sender.output(PacksOutput::Toast(err));
            }
        }
    }
}
