// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Shell Tweaks page.
//!
//! Renders every tweak [`CustomizeShellUseCase`] reports for the
//! running version, grouped by [`ShellTweakSurface`]. The page itself
//! knows nothing about GNOME versions — Yellow stays version-agnostic,
//! Red absorbs the drift.
//!
//! Rendering rules (plan §7, Decision 2):
//!
//! * `ControlSupport::Full` — row is interactive.
//! * `ControlSupport::Degraded` — row is interactive; subtitle shows
//!   the reason in muted text.
//! * `ControlSupport::Unsupported` — row is disabled and the reason
//!   replaces the subtitle. Keeps the educational signal for users
//!   considering a GNOME upgrade.

use crate::services::AppServices;
use adw::prelude::*;
use glib;
use gnomex_app::use_cases::CustomizeShellUseCase;
use gnomex_domain::{
    theme_capability::ControlSupport, ShellTweak, ShellTweakId, ShellTweakSurface, TweakKind,
    TweakValue,
};
use relm4::prelude::*;
use std::sync::Arc;
use tokio::runtime::Handle;

pub struct ShellTweaksModel {
    use_case: Arc<CustomizeShellUseCase>,
    handle: Handle,
}

#[derive(Debug)]
pub enum ShellTweaksMsg {
    ApplyBool(ShellTweakId, bool),
    ApplyEnum(ShellTweakId, String),
    ApplyInt(ShellTweakId, i32),
}

#[derive(Debug)]
pub enum ShellTweaksOutput {}

#[relm4::component(pub)]
impl SimpleComponent for ShellTweaksModel {
    type Init = AppServices;
    type Input = ShellTweaksMsg;
    type Output = ShellTweaksOutput;

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
        let use_case = services.customize_shell.clone();
        let handle = services.handle.clone();

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

        // Version banner.
        let banner = adw::ActionRow::builder()
            .title("GNOME Shell")
            .subtitle(use_case.version_label())
            .css_classes(["property"])
            .build();
        banner.add_prefix(&gtk::Image::from_icon_name("computer-symbolic"));
        let banner_group = adw::PreferencesGroup::new();
        banner_group.add(&banner);
        outer.append(&banner_group);

        // Fan out hints into per-surface groups in the declared surface
        // order. This lets us render Panel before Overview, etc.,
        // independently of the profile's declaration order.
        let all_hints = use_case.list_all_hints();
        for surface in ShellTweakSurface::ALL {
            let surface_hints: Vec<_> = all_hints
                .iter()
                .filter(|(id, _)| id.surface() == surface)
                .collect();
            if surface_hints.is_empty() {
                continue;
            }
            let group = adw::PreferencesGroup::builder()
                // `PreferencesGroup::title` parses Pango markup, so any
                // `&` in a surface label (Motion & Animations, Fonts &
                // Cursor) must be escaped. We do it here rather than in
                // domain so the domain labels stay plain-text.
                .title(glib::markup_escape_text(surface.label()).as_str())
                .build();
            for (id, hint) in &surface_hints {
                let row = build_row(
                    *id,
                    &hint.support,
                    hint.safe_range,
                    &use_case,
                    &handle,
                    sender.clone(),
                );
                group.add(&row);
            }
            outer.append(&group);
        }

        clamp.set_child(Some(&outer));
        scroll.set_child(Some(&clamp));
        root.append(&scroll);

        let model = ShellTweaksModel { use_case, handle };
        let widgets = view_output!();
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: ShellTweaksMsg, _sender: ComponentSender<Self>) {
        let use_case = self.use_case.clone();
        match msg {
            ShellTweaksMsg::ApplyBool(id, value) => {
                let tweak = ShellTweak {
                    id,
                    value: TweakValue::Bool(value),
                };
                self.handle.spawn(async move {
                    if let Err(e) = use_case.apply(tweak).await {
                        tracing::warn!("apply {id:?}: {e}");
                    }
                });
            }
            ShellTweaksMsg::ApplyEnum(id, value) => {
                let tweak = ShellTweak {
                    id,
                    value: TweakValue::Enum(value),
                };
                self.handle.spawn(async move {
                    if let Err(e) = use_case.apply(tweak).await {
                        tracing::warn!("apply {id:?}: {e}");
                    }
                });
            }
            ShellTweaksMsg::ApplyInt(id, value) => {
                let tweak = ShellTweak {
                    id,
                    value: TweakValue::Int(value),
                };
                self.handle.spawn(async move {
                    if let Err(e) = use_case.apply(tweak).await {
                        tracing::warn!("apply {id:?}: {e}");
                    }
                });
            }
        }
    }
}

/// Build the appropriate row for this tweak + support level.
///
/// Dispatches on the tweak's [`TweakKind`] so booleans become
/// `SwitchRow`s, enums become `ComboRow`s, and ints become
/// `SpinRow`s. `Unsupported` short-circuits to a flat disabled row.
fn build_row(
    id: ShellTweakId,
    support: &ControlSupport,
    safe_range: Option<(i32, i32)>,
    use_case: &Arc<CustomizeShellUseCase>,
    handle: &Handle,
    sender: ComponentSender<ShellTweaksModel>,
) -> gtk::Widget {
    if support.is_unsupported() {
        let row = adw::ActionRow::builder()
            .title(id.label())
            .subtitle(support.reason().unwrap_or(""))
            .sensitive(false)
            .build();
        row.add_suffix(&unsupported_badge());
        return row.upcast();
    }

    // Read the current value once, synchronously through the tokio
    // handle. Tweak reads are cheap (one GSettings call).
    let current = handle.block_on(use_case.read(id)).ok().flatten();

    match id.value_kind() {
        TweakKind::Bool => build_bool_row(id, support, current, sender),
        TweakKind::Enum => build_enum_row(id, support, id.enum_options(), current, sender),
        TweakKind::Int => build_int_row(id, support, safe_range, current, sender),
        TweakKind::Position => {
            // Position tweaks (TopBarPosition) are all Unsupported today
            // — they need the bundled extension. Fall back to a disabled
            // row rather than inventing a widget we don't need yet.
            let row = adw::ActionRow::builder()
                .title(id.label())
                .subtitle("Requires a Shell extension")
                .sensitive(false)
                .build();
            row.upcast()
        }
    }
}

fn build_bool_row(
    id: ShellTweakId,
    support: &ControlSupport,
    current: Option<ShellTweak>,
    sender: ComponentSender<ShellTweaksModel>,
) -> gtk::Widget {
    let active = matches!(
        current.map(|t| t.value),
        Some(TweakValue::Bool(true))
    );
    let row = adw::SwitchRow::builder()
        .title(id.label())
        .subtitle(subtitle_for(id, support))
        .active(active)
        .build();
    row.connect_active_notify(move |r| {
        sender.input(ShellTweaksMsg::ApplyBool(id, r.is_active()));
    });
    row.upcast()
}

fn build_enum_row(
    id: ShellTweakId,
    support: &ControlSupport,
    options: &'static [(&'static str, &'static str)],
    current: Option<ShellTweak>,
    sender: ComponentSender<ShellTweaksModel>,
) -> gtk::Widget {
    let model = gtk::StringList::new(&options.iter().map(|(_, l)| *l).collect::<Vec<_>>());

    let current_str = match current {
        Some(ShellTweak {
            value: TweakValue::Enum(s),
            ..
        }) => s,
        _ => String::new(),
    };
    let selected = options
        .iter()
        .position(|(v, _)| *v == current_str)
        .unwrap_or(0) as u32;

    let row = adw::ComboRow::builder()
        .title(id.label())
        .subtitle(subtitle_for(id, support))
        .model(&model)
        .selected(selected)
        .build();
    row.connect_selected_notify(move |r| {
        let idx = r.selected() as usize;
        if let Some((value, _)) = options.get(idx) {
            sender.input(ShellTweaksMsg::ApplyEnum(id, (*value).to_string()));
        }
    });
    row.upcast()
}

fn build_int_row(
    id: ShellTweakId,
    support: &ControlSupport,
    safe_range: Option<(i32, i32)>,
    current: Option<ShellTweak>,
    sender: ComponentSender<ShellTweaksModel>,
) -> gtk::Widget {
    // Safe range defaults cover every int-valued tweak we ship today.
    // Out-of-range values from the user's current GSettings aren't
    // clamped at read time — they're clamped the moment they edit.
    let (min, max) = safe_range.unwrap_or((0, 100));
    let initial = match current.map(|t| t.value) {
        Some(TweakValue::Int(n)) => n.clamp(min, max),
        _ => min,
    };

    let row = adw::SpinRow::builder()
        .title(id.label())
        .subtitle(subtitle_for(id, support))
        .build();
    let adj = gtk::Adjustment::new(
        initial as f64,
        min as f64,
        max as f64,
        1.0,
        1.0,
        0.0,
    );
    row.set_adjustment(Some(&adj));
    row.connect_value_notify(move |r| {
        sender.input(ShellTweaksMsg::ApplyInt(id, r.value() as i32));
    });
    row.upcast()
}

fn subtitle_for(id: ShellTweakId, support: &ControlSupport) -> String {
    match support {
        ControlSupport::Full => id.subtitle().to_owned(),
        ControlSupport::Degraded { reason } => {
            // Leading bullet keeps the Degraded notice visually distinct
            // from plain subtitles without needing a separate widget.
            format!("{} \u{2022} {}", id.subtitle(), reason)
        }
        ControlSupport::Unsupported { reason } => (*reason).to_owned(),
    }
}

fn unsupported_badge() -> gtk::Label {
    gtk::Label::builder()
        .label("Unavailable")
        .css_classes(["dim-label", "caption"])
        .valign(gtk::Align::Center)
        .build()
}
