// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Detail view — full-page view for an extension, theme, icon pack, or cursor.
//! Shows a large screenshot/preview, description, metadata, and action buttons.

use adw::prelude::*;
use gtk::gdk;
use relm4::prelude::*;

/// What kind of item we're showing details for.
#[derive(Debug, Clone)]
pub enum DetailItem {
    Extension {
        name: String,
        creator: String,
        uuid: String,
        description: String,
        screenshot_url: Option<String>,
        installed: bool,
    },
    Content {
        name: String,
        creator: String,
        id: u64,
        description: String,
        preview_url: Option<String>,
        installed: bool,
    },
}

impl DetailItem {
    pub fn name(&self) -> &str {
        match self {
            Self::Extension { name, .. } | Self::Content { name, .. } => name,
        }
    }

    pub fn creator(&self) -> &str {
        match self {
            Self::Extension { creator, .. } | Self::Content { creator, .. } => creator,
        }
    }

    pub fn description(&self) -> &str {
        match self {
            Self::Extension { description, .. } | Self::Content { description, .. } => description,
        }
    }

    pub fn image_url(&self) -> Option<&str> {
        match self {
            Self::Extension { screenshot_url, .. } => screenshot_url.as_deref(),
            Self::Content { preview_url, .. } => preview_url.as_deref(),
        }
    }

    pub fn is_installed(&self) -> bool {
        match self {
            Self::Extension { installed, .. } | Self::Content { installed, .. } => *installed,
        }
    }
}

pub struct DetailViewModel {
    item: Option<DetailItem>,
    picture: gtk::Picture,
    title_label: gtk::Label,
    creator_label: gtk::Label,
    description_label: gtk::Label,
    action_btn: gtk::Button,
}

#[derive(Debug)]
pub enum DetailViewMsg {
    Show(DetailItem),
    ScreenshotLoaded(gdk::Texture),
    ScreenshotFailed,
    ActionClicked,
    Back,
}

#[derive(Debug)]
pub enum DetailViewOutput {
    Back,
    InstallExtension(String),
    InstallContent(u64, String),
}

#[relm4::component(pub)]
impl SimpleComponent for DetailViewModel {
    type Init = ();
    type Input = DetailViewMsg;
    type Output = DetailViewOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
        }
    }

    fn init(
        _: (),
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let clamp = adw::Clamp::builder()
            .maximum_size(700)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();

        let outer = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(16)
            .build();

        // Back button
        let back_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .build();
        let back_btn = gtk::Button::builder()
            .icon_name("go-previous-symbolic")
            .css_classes(["flat"])
            .tooltip_text("Back")
            .build();
        {
            let sender = sender.clone();
            back_btn.connect_clicked(move |_| {
                sender.input(DetailViewMsg::Back);
            });
        }
        back_row.append(&back_btn);
        outer.append(&back_row);

        // Screenshot / preview
        let picture_frame = gtk::Frame::builder()
            .css_classes(["view"])
            .build();
        let picture = gtk::Picture::builder()
            .content_fit(gtk::ContentFit::Contain)
            .height_request(300)
            .build();
        picture_frame.set_child(Some(&picture));
        outer.append(&picture_frame);

        // Title + creator
        let title_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .css_classes(["title-1"])
            .wrap(true)
            .build();
        outer.append(&title_label);

        let creator_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .css_classes(["dim-label"])
            .build();
        outer.append(&creator_label);

        // Action button
        let action_btn = gtk::Button::builder()
            .label("Install")
            .css_classes(["suggested-action", "pill"])
            .halign(gtk::Align::Start)
            .build();
        {
            let sender = sender.clone();
            action_btn.connect_clicked(move |_| {
                sender.input(DetailViewMsg::ActionClicked);
            });
        }
        outer.append(&action_btn);

        // Description
        let description_label = gtk::Label::builder()
            .halign(gtk::Align::Start)
            .wrap(true)
            .css_classes(["body"])
            .selectable(true)
            .build();
        outer.append(&description_label);

        clamp.set_child(Some(&outer));
        scroll.set_child(Some(&clamp));
        root.append(&scroll);

        let model = DetailViewModel {
            item: None,
            picture: picture.clone(),
            title_label: title_label.clone(),
            creator_label: creator_label.clone(),
            description_label: description_label.clone(),
            action_btn: action_btn.clone(),
        };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: DetailViewMsg, sender: ComponentSender<Self>) {
        match msg {
            DetailViewMsg::Show(item) => {
                self.title_label.set_label(item.name());
                self.creator_label
                    .set_label(&format!("by {}", item.creator()));

                let desc = strip_html(item.description());
                self.description_label.set_label(&desc);

                if item.is_installed() {
                    self.action_btn.set_label("Installed");
                    self.action_btn.set_sensitive(false);
                    self.action_btn.remove_css_class("suggested-action");
                } else {
                    self.action_btn.set_label("Install");
                    self.action_btn.set_sensitive(true);
                    self.action_btn.add_css_class("suggested-action");
                }

                // Clear previous screenshot
                self.picture
                    .set_paintable(None::<&gdk::Paintable>);

                // Load screenshot via GIO (GVfs handles http:// URIs)
                if let Some(url) = item.image_url() {
                    let file = gio::File::for_uri(url);
                    let sender = sender.input_sender().clone();
                    // Read bytes async, then create texture on callback
                    file.load_bytes_async(
                        gio::Cancellable::NONE,
                        move |result| {
                            let Ok((bytes, _)) = result else {
                                sender.emit(DetailViewMsg::ScreenshotFailed);
                                return;
                            };
                            match gdk::Texture::from_bytes(&bytes) {
                                Ok(tex) => sender.emit(DetailViewMsg::ScreenshotLoaded(tex)),
                                Err(_) => sender.emit(DetailViewMsg::ScreenshotFailed),
                            }
                        },
                    );
                }

                self.item = Some(item);
            }
            DetailViewMsg::ScreenshotLoaded(texture) => {
                self.picture.set_paintable(Some(&texture));
            }
            DetailViewMsg::ScreenshotFailed => {
                self.picture
                    .set_paintable(None::<&gdk::Paintable>);
            }
            DetailViewMsg::ActionClicked => {
                if let Some(ref item) = self.item {
                    match item {
                        DetailItem::Extension { uuid, .. } => {
                            let _ = sender
                                .output(DetailViewOutput::InstallExtension(uuid.clone()));
                        }
                        DetailItem::Content { id, name, .. } => {
                            let _ = sender
                                .output(DetailViewOutput::InstallContent(*id, name.clone()));
                        }
                    }
                }
            }
            DetailViewMsg::Back => {
                let _ = sender.output(DetailViewOutput::Back);
            }
        }
    }
}

/// Naive HTML tag stripper for OCS descriptions.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}
