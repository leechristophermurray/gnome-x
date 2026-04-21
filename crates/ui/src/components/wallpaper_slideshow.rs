// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Wallpaper slideshow authoring view (GXF-072).
//!
//! Lets the user pick multiple wallpapers, set a rotation interval,
//! toggle shuffle, preview the picture list, and apply the result.
//! On Apply, we drive [`WallpaperSlideshowUseCase`] which materialises
//! a GNOME `<background>` XML under
//! `~/.local/share/gnome-background-properties/` and points
//! `picture-uri` / `picture-uri-dark` at it. GNOME Shell handles
//! playback — no daemon needed.
//!
//! Placement rationale: the Theme Builder is already >3 000 lines of
//! dense sliders; jamming a picker + reorder list in there would be
//! risky and noisy. A standalone view keeps this feature testable in
//! isolation and adds a single row to the AdwViewSwitcher. The
//! existing wallpaper-accent plumbing in Theme Builder is orthogonal
//! — it reads `picture-uri`, which is still a `.jpg` for single
//! wallpapers and becomes `<file://…/gnome-x-slideshow.xml>` once a
//! slideshow is active. Both flows co-exist without change.

use crate::services::AppServices;
use adw::prelude::*;
use gnomex_app::use_cases::WallpaperSlideshowUseCase;
use gnomex_domain::{TimeOfDay, WallpaperSlideshow, MIN_INTERVAL_SECONDS};
use relm4::prelude::*;
use std::path::PathBuf;
use std::sync::Arc;

const SCHEMA_ID: &str = "io.github.gnomex.GnomeX";
const KEY_NAME: &str = "wallpaper-slideshow-name";
const KEY_PICTURES: &str = "wallpaper-slideshow-pictures";
const KEY_INTERVAL: &str = "wallpaper-slideshow-interval";
const KEY_SHUFFLE: &str = "wallpaper-slideshow-shuffle";
const KEY_MODE: &str = "wallpaper-slideshow-mode";
// Time-of-day keys. Times are stored as minutes-since-midnight (`u`)
// rather than `hh:mm` strings so the spin-row can write them directly.
const KEY_TOD_SUNRISE_PIC: &str = "wallpaper-tod-sunrise-picture";
const KEY_TOD_DAY_PIC: &str = "wallpaper-tod-day-picture";
const KEY_TOD_SUNSET_PIC: &str = "wallpaper-tod-sunset-picture";
const KEY_TOD_NIGHT_PIC: &str = "wallpaper-tod-night-picture";
const KEY_TOD_SUNRISE_MIN: &str = "wallpaper-tod-sunrise-minute";
const KEY_TOD_DAY_MIN: &str = "wallpaper-tod-day-minute";
const KEY_TOD_SUNSET_MIN: &str = "wallpaper-tod-sunset-minute";
const KEY_TOD_NIGHT_MIN: &str = "wallpaper-tod-night-minute";

/// Which of the four TimeOfDay slots a picker / time-spin targets.
#[derive(Debug, Clone, Copy)]
pub enum TodSlot {
    Sunrise,
    Day,
    Sunset,
    Night,
}

pub struct WallpaperSlideshowModel {
    use_case: Arc<WallpaperSlideshowUseCase>,
    settings: gio::Settings,
    name: String,
    pictures: Vec<PathBuf>,
    interval_seconds: u32,
    shuffle: bool,
    /// "interval" or "tod" — drives which section is editable.
    mode: String,
    /// Four TimeOfDay slot pictures in sunrise/day/sunset/night order.
    tod_pictures: [Option<PathBuf>; 4],
    /// Four TimeOfDay slot transition moments, minutes-since-midnight.
    tod_minutes: [u32; 4],
    // Widgets that need to re-render on state changes.
    pictures_group: adw::PreferencesGroup,
    tod_group: adw::PreferencesGroup,
    options_group: adw::PreferencesGroup,
    /// Every row we currently show for `pictures`. Kept so we can
    /// clear them en-masse when the list changes — the UI always
    /// re-renders from `pictures` rather than mutating individual
    /// rows.
    picture_rows: Vec<adw::ActionRow>,
    /// The four TimeOfDay rows (one per slot) kept so we can update
    /// their subtitles when the user picks a new file.
    tod_rows: [Option<adw::ActionRow>; 4],
    /// Summary label ("3 pictures · loop 18 min") under the picker
    /// toolbar.
    summary: gtk::Label,
    apply_button: gtk::Button,
    empty_label: gtk::Label,
}

#[derive(Debug)]
pub enum WallpaperSlideshowMsg {
    AddPictures,
    PicturesChosen(Vec<PathBuf>),
    RemovePicture(usize),
    MovePictureUp(usize),
    MovePictureDown(usize),
    ClearPictures,
    SetInterval(u32),
    SetShuffle(bool),
    SetMode(String),
    ChooseTodPicture(TodSlot),
    TodPictureChosen(TodSlot, PathBuf),
    SetTodMinute(TodSlot, u32),
    Apply,
    ApplySucceeded { uri: String, gsettings_updated: bool },
    ApplyFailed(String),
}

#[derive(Debug)]
pub enum WallpaperSlideshowOutput {
    Toast(String),
}

#[relm4::component(pub)]
impl SimpleComponent for WallpaperSlideshowModel {
    type Init = AppServices;
    type Input = WallpaperSlideshowMsg;
    type Output = WallpaperSlideshowOutput;

    view! {
        gtk::Box {
            set_orientation: gtk::Orientation::Vertical,
            set_vexpand: true,
        }
    }

    fn init(
        services: AppServices,
        root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let settings = gio::Settings::new(SCHEMA_ID);
        let name = {
            let s = settings.string(KEY_NAME).to_string();
            if s.is_empty() { "gnome-x-slideshow".into() } else { s }
        };
        let pictures: Vec<PathBuf> = settings
            .strv(KEY_PICTURES)
            .iter()
            .map(|g| PathBuf::from(g.as_str()))
            .collect();
        let interval_seconds = settings
            .uint(KEY_INTERVAL)
            .max(MIN_INTERVAL_SECONDS);
        let shuffle = settings.boolean(KEY_SHUFFLE);
        let mode = {
            let s = settings.string(KEY_MODE).to_string();
            if s == "tod" { "tod".into() } else { "interval".into() }
        };
        let tod_pictures: [Option<PathBuf>; 4] = [
            load_opt_path(&settings, KEY_TOD_SUNRISE_PIC),
            load_opt_path(&settings, KEY_TOD_DAY_PIC),
            load_opt_path(&settings, KEY_TOD_SUNSET_PIC),
            load_opt_path(&settings, KEY_TOD_NIGHT_PIC),
        ];
        let tod_minutes: [u32; 4] = [
            settings.uint(KEY_TOD_SUNRISE_MIN),
            settings.uint(KEY_TOD_DAY_MIN),
            settings.uint(KEY_TOD_SUNSET_MIN),
            settings.uint(KEY_TOD_NIGHT_MIN),
        ];

        // --- Layout -----------------------------------------------------
        let scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let clamp = adw::Clamp::builder()
            .maximum_size(720)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        let outer = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .build();

        // --- Header group: title + explanation -------------------------
        let header_group = adw::PreferencesGroup::builder()
            .title("Wallpaper Slideshow")
            .description(
                "Rotate between multiple wallpapers. GNOME Shell plays the \
                 slideshow natively — no background daemon required.",
            )
            .build();
        outer.append(&header_group);

        // --- Mode selector ---------------------------------------------
        // A discreet SwitchRow instead of a full AdwComboRow — the
        // user choice is binary, and the row's subtitle explains
        // what each mode does.
        let mode_group = adw::PreferencesGroup::builder().build();
        let tod_switch = adw::SwitchRow::builder()
            .title("Time-of-day mode")
            .subtitle(
                "Swap wallpapers at sunrise, day, sunset, and night \
                 instead of cycling on a fixed interval. The theme \
                 daemon re-extracts colours at each transition.",
            )
            .active(mode == "tod")
            .build();
        {
            let sender = sender.clone();
            tod_switch.connect_active_notify(move |r| {
                let m = if r.is_active() { "tod" } else { "interval" };
                sender.input(WallpaperSlideshowMsg::SetMode(m.into()));
            });
        }
        mode_group.add(&tod_switch);
        outer.append(&mode_group);

        // --- Pictures group --------------------------------------------
        let pictures_group = adw::PreferencesGroup::builder()
            .title("Pictures")
            .build();

        let toolbar = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(6)
            .build();

        let add_button = gtk::Button::builder()
            .icon_name("list-add-symbolic")
            .tooltip_text("Add pictures…")
            .css_classes(["flat"])
            .build();
        {
            let sender = sender.clone();
            add_button.connect_clicked(move |_| {
                sender.input(WallpaperSlideshowMsg::AddPictures);
            });
        }
        toolbar.append(&add_button);

        let clear_button = gtk::Button::builder()
            .icon_name("edit-clear-symbolic")
            .tooltip_text("Remove all pictures")
            .css_classes(["flat"])
            .build();
        {
            let sender = sender.clone();
            clear_button.connect_clicked(move |_| {
                sender.input(WallpaperSlideshowMsg::ClearPictures);
            });
        }
        toolbar.append(&clear_button);

        let summary = gtk::Label::builder()
            .halign(gtk::Align::End)
            .hexpand(true)
            .css_classes(["dim-label", "caption"])
            .build();
        toolbar.append(&summary);

        pictures_group.set_header_suffix(Some(&toolbar));

        let empty_label = gtk::Label::builder()
            .label("No pictures yet. Click + to add some.")
            .css_classes(["dim-label"])
            .halign(gtk::Align::Center)
            .margin_top(12)
            .margin_bottom(12)
            .build();
        pictures_group.add(&empty_label);

        outer.append(&pictures_group);

        // --- Rotation options group ------------------------------------
        let options_group = adw::PreferencesGroup::builder()
            .title("Rotation")
            .build();

        let interval_row = adw::SpinRow::builder()
            .title("Interval")
            .subtitle("Seconds each picture is shown before fading to the next")
            .build();
        let adj = gtk::Adjustment::new(
            interval_seconds as f64,
            MIN_INTERVAL_SECONDS as f64,
            // 24 hours — plenty of headroom for "one per day" setups.
            86_400.0,
            5.0,
            60.0,
            0.0,
        );
        interval_row.set_adjustment(Some(&adj));
        {
            let sender = sender.clone();
            interval_row.connect_value_notify(move |r| {
                let v = r.value() as u32;
                sender.input(WallpaperSlideshowMsg::SetInterval(v));
            });
        }
        options_group.add(&interval_row);

        let shuffle_row = adw::SwitchRow::builder()
            .title("Shuffle")
            .subtitle("Randomise picture order before saving the slideshow")
            .active(shuffle)
            .build();
        {
            let sender = sender.clone();
            shuffle_row.connect_active_notify(move |r| {
                sender.input(WallpaperSlideshowMsg::SetShuffle(r.is_active()));
            });
        }
        options_group.add(&shuffle_row);

        outer.append(&options_group);

        // --- Time-of-day group -----------------------------------------
        // Four rows (sunrise / day / sunset / night). Each carries a
        // file-picker button AND a spin row for the transition
        // minute-of-day. Populated below via `rebuild_tod_rows`.
        let tod_group = adw::PreferencesGroup::builder()
            .title("Time of Day")
            .description(
                "Pick a wallpaper for each quarter of the day. The \
                 transition time is the moment the NEXT picture takes \
                 over — e.g. \"Day at 09:00\" means the sunrise picture \
                 holds until 09:00, then the day picture shows.",
            )
            .build();
        outer.append(&tod_group);

        // --- Apply button ----------------------------------------------
        let apply_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .halign(gtk::Align::End)
            .build();
        let apply_button = gtk::Button::builder()
            .label("Apply")
            .css_classes(["pill", "suggested-action"])
            .sensitive(pictures.len() >= 2)
            .build();
        {
            let sender = sender.clone();
            apply_button.connect_clicked(move |_| {
                sender.input(WallpaperSlideshowMsg::Apply);
            });
        }
        apply_row.append(&apply_button);
        outer.append(&apply_row);

        clamp.set_child(Some(&outer));
        scroll.set_child(Some(&clamp));
        root.append(&scroll);

        let mut model = WallpaperSlideshowModel {
            use_case: services.wallpaper_slideshow.clone(),
            settings,
            name,
            pictures,
            interval_seconds,
            shuffle,
            mode,
            tod_pictures,
            tod_minutes,
            pictures_group: pictures_group.clone(),
            tod_group: tod_group.clone(),
            options_group: options_group.clone(),
            picture_rows: Vec::new(),
            tod_rows: [None, None, None, None],
            summary: summary.clone(),
            apply_button: apply_button.clone(),
            empty_label: empty_label.clone(),
        };
        model.rebuild_picture_rows(&sender);
        model.build_tod_rows(&sender);
        model.refresh_summary();
        model.apply_mode_visibility();

        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: WallpaperSlideshowMsg, sender: ComponentSender<Self>) {
        match msg {
            WallpaperSlideshowMsg::AddPictures => {
                self.spawn_add_dialog(&sender);
            }
            WallpaperSlideshowMsg::PicturesChosen(new_paths) => {
                for p in new_paths {
                    if p.is_absolute() && !self.pictures.contains(&p) {
                        self.pictures.push(p);
                    }
                }
                self.persist_pictures();
                self.rebuild_picture_rows(&sender);
                self.refresh_summary();
            }
            WallpaperSlideshowMsg::RemovePicture(idx) => {
                if idx < self.pictures.len() {
                    self.pictures.remove(idx);
                    self.persist_pictures();
                    self.rebuild_picture_rows(&sender);
                    self.refresh_summary();
                }
            }
            WallpaperSlideshowMsg::MovePictureUp(idx) => {
                if idx > 0 && idx < self.pictures.len() {
                    self.pictures.swap(idx - 1, idx);
                    self.persist_pictures();
                    self.rebuild_picture_rows(&sender);
                }
            }
            WallpaperSlideshowMsg::MovePictureDown(idx) => {
                if idx + 1 < self.pictures.len() {
                    self.pictures.swap(idx, idx + 1);
                    self.persist_pictures();
                    self.rebuild_picture_rows(&sender);
                }
            }
            WallpaperSlideshowMsg::ClearPictures => {
                self.pictures.clear();
                self.persist_pictures();
                self.rebuild_picture_rows(&sender);
                self.refresh_summary();
            }
            WallpaperSlideshowMsg::SetInterval(v) => {
                self.interval_seconds = v.max(MIN_INTERVAL_SECONDS);
                self.settings
                    .set_uint(KEY_INTERVAL, self.interval_seconds)
                    .ok();
                self.refresh_summary();
            }
            WallpaperSlideshowMsg::SetShuffle(on) => {
                self.shuffle = on;
                self.settings.set_boolean(KEY_SHUFFLE, on).ok();
            }
            WallpaperSlideshowMsg::SetMode(m) => {
                self.mode = m;
                self.settings.set_string(KEY_MODE, &self.mode).ok();
                self.apply_mode_visibility();
                self.refresh_apply_sensitivity();
                // The user's mental model is "toggle = switch
                // slideshow", not "toggle = re-arrange editor". If the
                // target mode already has valid data, apply it now so
                // the desktop actually follows the switch. When the
                // target mode is incomplete we leave the old slideshow
                // running — the Apply button stays disabled as a hint.
                if self.is_target_mode_valid() {
                    self.apply_slideshow(&sender);
                }
            }
            WallpaperSlideshowMsg::ChooseTodPicture(slot) => {
                self.spawn_tod_picker(slot, &sender);
            }
            WallpaperSlideshowMsg::TodPictureChosen(slot, path) => {
                if path.is_absolute() {
                    let idx = slot_index(slot);
                    self.tod_pictures[idx] = Some(path);
                    self.persist_tod_picture(slot);
                    self.refresh_tod_row(slot);
                    self.refresh_apply_sensitivity();
                }
            }
            WallpaperSlideshowMsg::SetTodMinute(slot, min) => {
                let idx = slot_index(slot);
                self.tod_minutes[idx] = min.min(23 * 60 + 59);
                self.persist_tod_minute(slot);
                self.refresh_apply_sensitivity();
            }
            WallpaperSlideshowMsg::Apply => self.apply_slideshow(&sender),
            WallpaperSlideshowMsg::ApplySucceeded {
                uri,
                gsettings_updated,
            } => {
                let text = if gsettings_updated {
                    format!("Slideshow applied — {uri}")
                } else {
                    "Slideshow saved, but couldn't update wallpaper setting".into()
                };
                let _ = sender.output(WallpaperSlideshowOutput::Toast(text));
            }
            WallpaperSlideshowMsg::ApplyFailed(err) => {
                let _ = sender
                    .output(WallpaperSlideshowOutput::Toast(format!("Slideshow failed: {err}")));
            }
        }
    }
}

impl WallpaperSlideshowModel {
    fn rebuild_picture_rows(&mut self, sender: &ComponentSender<Self>) {
        for row in self.picture_rows.drain(..) {
            self.pictures_group.remove(&row);
        }
        let has_any = !self.pictures.is_empty();
        self.empty_label.set_visible(!has_any);

        let total = self.pictures.len();
        for (i, path) in self.pictures.clone().into_iter().enumerate() {
            let row = adw::ActionRow::builder()
                .title(
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("(unnamed)"),
                )
                .subtitle(path.to_string_lossy())
                .build();

            let up = gtk::Button::builder()
                .icon_name("go-up-symbolic")
                .tooltip_text("Move up")
                .css_classes(["flat"])
                .valign(gtk::Align::Center)
                .sensitive(i > 0)
                .build();
            {
                let sender = sender.clone();
                up.connect_clicked(move |_| {
                    sender.input(WallpaperSlideshowMsg::MovePictureUp(i));
                });
            }
            row.add_suffix(&up);

            let down = gtk::Button::builder()
                .icon_name("go-down-symbolic")
                .tooltip_text("Move down")
                .css_classes(["flat"])
                .valign(gtk::Align::Center)
                .sensitive(i + 1 < total)
                .build();
            {
                let sender = sender.clone();
                down.connect_clicked(move |_| {
                    sender.input(WallpaperSlideshowMsg::MovePictureDown(i));
                });
            }
            row.add_suffix(&down);

            let remove = gtk::Button::builder()
                .icon_name("list-remove-symbolic")
                .tooltip_text("Remove from slideshow")
                .css_classes(["flat"])
                .valign(gtk::Align::Center)
                .build();
            {
                let sender = sender.clone();
                remove.connect_clicked(move |_| {
                    sender.input(WallpaperSlideshowMsg::RemovePicture(i));
                });
            }
            row.add_suffix(&remove);

            self.pictures_group.add(&row);
            self.picture_rows.push(row);
        }

        self.apply_button.set_sensitive(self.pictures.len() >= 2);
    }

    fn refresh_summary(&self) {
        if self.pictures.is_empty() {
            self.summary.set_text("");
            return;
        }
        let n = self.pictures.len();
        let loop_seconds =
            n as u64 * self.interval_seconds as u64 + n as u64; // +~1s transition each
        let mins = loop_seconds / 60;
        let secs = loop_seconds % 60;
        let loop_text = if mins > 0 {
            format!("{mins} min {secs} s")
        } else {
            format!("{secs} s")
        };
        self.summary
            .set_text(&format!("{n} pictures \u{2022} loop \u{2248} {loop_text}"));
    }

    fn persist_pictures(&self) {
        let strv: Vec<&str> = self
            .pictures
            .iter()
            .filter_map(|p| p.to_str())
            .collect();
        self.settings.set_strv(KEY_PICTURES, strv).ok();
    }

    fn spawn_add_dialog(&self, sender: &ComponentSender<Self>) {
        // Limit to common raster formats GNOME's own wallpaper
        // picker accepts. Users can still drop anything into the list
        // — this is just the file dialog filter.
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Images"));
        for mime in ["image/png", "image/jpeg", "image/webp", "image/tiff"] {
            filter.add_mime_type(mime);
        }

        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);

        let dialog = gtk::FileDialog::builder()
            .title("Add Wallpapers")
            .filters(&filters)
            .build();

        let window = self.window();
        let sender = sender.clone();
        dialog.open_multiple(
            window.as_ref(),
            gio::Cancellable::NONE,
            move |result| {
                if let Ok(files) = result {
                    let n = files.n_items();
                    let mut paths = Vec::with_capacity(n as usize);
                    for i in 0..n {
                        if let Some(file) = files.item(i).and_then(|o| o.downcast::<gio::File>().ok()) {
                            if let Some(path) = file.path() {
                                paths.push(path);
                            }
                        }
                    }
                    if !paths.is_empty() {
                        sender.input(WallpaperSlideshowMsg::PicturesChosen(paths));
                    }
                }
            },
        );
    }

    fn window(&self) -> Option<gtk::Window> {
        self.pictures_group
            .root()
            .and_then(|r| r.downcast::<gtk::Window>().ok())
    }

    fn apply_slideshow(&self, sender: &ComponentSender<Self>) {
        // Persist name so later opens round-trip to the same XML file.
        self.settings.set_string(KEY_NAME, &self.name).ok();

        let slideshow = if self.mode == "tod" {
            match self.build_tod_slideshow() {
                Ok(s) => s,
                Err(e) => {
                    sender.input(WallpaperSlideshowMsg::ApplyFailed(e));
                    return;
                }
            }
        } else {
            // Clone-on-Apply: if shuffle is on, the caller scrambles a
            // fresh Vec so we don't mutate our persisted order.
            let mut pictures = self.pictures.clone();
            if self.shuffle {
                shuffle_in_place(&mut pictures);
            }
            match WallpaperSlideshow::new(
                self.name.clone(),
                pictures,
                self.interval_seconds,
                self.shuffle,
            ) {
                Ok(s) => s,
                Err(e) => {
                    sender.input(WallpaperSlideshowMsg::ApplyFailed(e.to_string()));
                    return;
                }
            }
        };

        let use_case = self.use_case.clone();
        let sender = sender.clone();
        // Use the app's tokio runtime via glib's main loop spawner —
        // the write is synchronous and fast, but we don't want to
        // block the GTK main loop if GSettings takes a moment.
        glib::MainContext::default().spawn_local(async move {
            match use_case.apply(&slideshow) {
                Ok(applied) => sender.input(
                    WallpaperSlideshowMsg::ApplySucceeded {
                        uri: applied.uri,
                        gsettings_updated: applied.gsettings_updated,
                    },
                ),
                Err(e) => {
                    sender.input(WallpaperSlideshowMsg::ApplyFailed(e.to_string()));
                }
            }
        });
    }
}

impl WallpaperSlideshowModel {
    fn apply_mode_visibility(&self) {
        let tod = self.mode == "tod";
        self.pictures_group.set_visible(!tod);
        self.options_group.set_visible(!tod);
        self.tod_group.set_visible(tod);
    }

    fn refresh_apply_sensitivity(&self) {
        self.apply_button.set_sensitive(self.is_target_mode_valid());
    }

    /// Does the currently-selected mode have enough data to Apply?
    /// Drives both the Apply button's sensitivity and the auto-apply
    /// on mode-toggle.
    fn is_target_mode_valid(&self) -> bool {
        if self.mode == "tod" {
            self.tod_pictures.iter().all(|p| p.is_some())
                && times_strictly_increasing(&self.tod_minutes)
        } else {
            self.pictures.len() >= 2
        }
    }

    fn build_tod_rows(&mut self, sender: &ComponentSender<Self>) {
        const TITLES: [(&str, TodSlot); 4] = [
            ("Sunrise", TodSlot::Sunrise),
            ("Day", TodSlot::Day),
            ("Sunset", TodSlot::Sunset),
            ("Night", TodSlot::Night),
        ];
        for (i, (title, slot)) in TITLES.iter().enumerate() {
            let row = adw::ActionRow::builder().title(*title).build();

            let pick_btn = gtk::Button::builder()
                .icon_name("folder-pictures-symbolic")
                .tooltip_text("Choose wallpaper")
                .css_classes(["flat"])
                .valign(gtk::Align::Center)
                .build();
            {
                let sender = sender.clone();
                let slot = *slot;
                pick_btn.connect_clicked(move |_| {
                    sender.input(WallpaperSlideshowMsg::ChooseTodPicture(slot));
                });
            }
            row.add_suffix(&pick_btn);

            // Transition-time spin rows: two small AdwSpinButtons
            // inside a container — the Adwaita convention for
            // hour:minute compound entry.
            let time_box = gtk::Box::builder()
                .orientation(gtk::Orientation::Horizontal)
                .spacing(2)
                .valign(gtk::Align::Center)
                .build();
            let minute = self.tod_minutes[i];
            let hours_adj = gtk::Adjustment::new((minute / 60) as f64, 0.0, 23.0, 1.0, 1.0, 0.0);
            let mins_adj = gtk::Adjustment::new((minute % 60) as f64, 0.0, 59.0, 1.0, 5.0, 0.0);
            let hh = gtk::SpinButton::builder()
                .adjustment(&hours_adj)
                .tooltip_text("Hour")
                .width_chars(2)
                .build();
            let mm = gtk::SpinButton::builder()
                .adjustment(&mins_adj)
                .tooltip_text("Minute")
                .width_chars(2)
                .build();
            let colon = gtk::Label::builder().label(":").build();
            time_box.append(&hh);
            time_box.append(&colon);
            time_box.append(&mm);
            let sep = gtk::Label::builder()
                .label("at")
                .css_classes(["dim-label", "caption"])
                .margin_end(4)
                .margin_start(4)
                .build();
            row.add_suffix(&sep);
            row.add_suffix(&time_box);
            {
                let sender = sender.clone();
                let slot = *slot;
                let mm_clone = mm.clone();
                hh.connect_value_changed(move |spin| {
                    let h = spin.value_as_int() as u32;
                    let total = h * 60 + mm_clone.value_as_int() as u32;
                    sender.input(WallpaperSlideshowMsg::SetTodMinute(slot, total));
                });
            }
            {
                let sender = sender.clone();
                let slot = *slot;
                let hh_clone = hh.clone();
                mm.connect_value_changed(move |spin| {
                    let m = spin.value_as_int() as u32;
                    let total = hh_clone.value_as_int() as u32 * 60 + m;
                    sender.input(WallpaperSlideshowMsg::SetTodMinute(slot, total));
                });
            }

            // Initial subtitle reflects the persisted picture (if any).
            if let Some(ref p) = self.tod_pictures[i] {
                row.set_subtitle(&p.to_string_lossy());
            } else {
                row.set_subtitle("(no picture selected)");
            }

            self.tod_group.add(&row);
            self.tod_rows[i] = Some(row);
        }
    }

    fn refresh_tod_row(&self, slot: TodSlot) {
        let i = slot_index(slot);
        if let Some(row) = &self.tod_rows[i] {
            if let Some(ref p) = self.tod_pictures[i] {
                row.set_subtitle(&p.to_string_lossy());
            } else {
                row.set_subtitle("(no picture selected)");
            }
        }
    }

    fn persist_tod_picture(&self, slot: TodSlot) {
        let key = match slot {
            TodSlot::Sunrise => KEY_TOD_SUNRISE_PIC,
            TodSlot::Day => KEY_TOD_DAY_PIC,
            TodSlot::Sunset => KEY_TOD_SUNSET_PIC,
            TodSlot::Night => KEY_TOD_NIGHT_PIC,
        };
        let i = slot_index(slot);
        let s = self.tod_pictures[i]
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or("");
        self.settings.set_string(key, s).ok();
    }

    fn persist_tod_minute(&self, slot: TodSlot) {
        let key = match slot {
            TodSlot::Sunrise => KEY_TOD_SUNRISE_MIN,
            TodSlot::Day => KEY_TOD_DAY_MIN,
            TodSlot::Sunset => KEY_TOD_SUNSET_MIN,
            TodSlot::Night => KEY_TOD_NIGHT_MIN,
        };
        let i = slot_index(slot);
        self.settings.set_uint(key, self.tod_minutes[i]).ok();
    }

    fn spawn_tod_picker(&self, slot: TodSlot, sender: &ComponentSender<Self>) {
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("Images"));
        for mime in ["image/png", "image/jpeg", "image/webp", "image/tiff"] {
            filter.add_mime_type(mime);
        }
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        let dialog = gtk::FileDialog::builder()
            .title("Choose Wallpaper")
            .filters(&filters)
            .build();
        let window = self.window();
        let sender = sender.clone();
        dialog.open(
            window.as_ref(),
            gio::Cancellable::NONE,
            move |result| {
                if let Ok(file) = result {
                    if let Some(path) = file.path() {
                        sender.input(WallpaperSlideshowMsg::TodPictureChosen(slot, path));
                    }
                }
            },
        );
    }

    fn build_tod_slideshow(&self) -> Result<WallpaperSlideshow, String> {
        let pics: Vec<PathBuf> = self
            .tod_pictures
            .iter()
            .map(|p| p.clone().ok_or_else(|| "pick a picture for every slot".to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        if !times_strictly_increasing(&self.tod_minutes) {
            return Err("transition times must increase: sunrise < day < sunset < night".into());
        }
        let slot_time = |m: u32| TimeOfDay::new((m / 60) as u8, (m % 60) as u8);
        WallpaperSlideshow::time_of_day(
            self.name.clone(),
            (pics[0].clone(), slot_time(self.tod_minutes[0])),
            (pics[1].clone(), slot_time(self.tod_minutes[1])),
            (pics[2].clone(), slot_time(self.tod_minutes[2])),
            (pics[3].clone(), slot_time(self.tod_minutes[3])),
        )
        .map_err(|e| e.to_string())
    }
}

fn slot_index(slot: TodSlot) -> usize {
    match slot {
        TodSlot::Sunrise => 0,
        TodSlot::Day => 1,
        TodSlot::Sunset => 2,
        TodSlot::Night => 3,
    }
}

fn times_strictly_increasing(mins: &[u32; 4]) -> bool {
    mins[0] < mins[1] && mins[1] < mins[2] && mins[2] < mins[3]
}

fn load_opt_path(settings: &gio::Settings, key: &str) -> Option<PathBuf> {
    let s = settings.string(key).to_string();
    if s.is_empty() {
        None
    } else {
        Some(PathBuf::from(s))
    }
}

/// Tiny Fisher-Yates using `glib::random_int_range` so we don't pull
/// in a new crate. Not cryptographically random — good enough for
/// "show pictures in a surprising order".
fn shuffle_in_place<T>(v: &mut [T]) {
    let n = v.len();
    if n < 2 {
        return;
    }
    for i in (1..n).rev() {
        let j = glib::random_int_range(0, (i + 1) as i32) as usize;
        v.swap(i, j);
    }
}
