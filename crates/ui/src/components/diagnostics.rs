// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Diagnostics view — surfaces two classes of invisible-but-important
//! system state:
//!
//! 1. **Shadowed resources** (GXF-012): a theme / icon / cursor
//!    installed in more than one XDG search path. The freedesktop
//!    lookup spec picks the first match; later copies are masked.
//!    Users regularly report "my new theme isn't applying" when the
//!    fresh user-local install is shadowed by an older system copy.
//!    We list every such conflict so the user knows which location
//!    is winning and can clean up the other.
//!
//! 2. **Window-decoration mix** (GXF-021): CSD vs SSD counts for
//!    currently-open windows. Theme-builder controls (headerbar
//!    height, window radius, drop-shadow) only reach client-side
//!    decorated apps. Showing the mix explains why some apps (Slack,
//!    Steam, Wine) stay visually unchanged.
//!
//! This view is **read-only**. We don't offer to delete shadowing
//! copies or flip decoration modes — those actions need confirmation
//! dialogs and elevated-privilege plumbing that belong in follow-up
//! work. Users do the cleanup themselves with the information we
//! surface.

use crate::services::AppServices;
use adw::prelude::*;
use gnomex_app::ports::{LocalInstaller, WindowDecorationProbe};
use gnomex_domain::{DecorationMode, DecorationReport, ResourceKind, ShadowedResource};
use relm4::prelude::*;
use std::sync::Arc;

pub struct DiagnosticsModel {
    installer: Arc<dyn LocalInstaller>,
    decoration_probe: Arc<dyn WindowDecorationProbe>,
    content_box: gtk::Box,
}

#[derive(Debug)]
pub enum DiagnosticsMsg {
    Refresh,
}

#[derive(Debug)]
pub enum DiagnosticsOutput {
    Toast(String),
}

#[relm4::component(pub)]
impl SimpleComponent for DiagnosticsModel {
    type Init = AppServices;
    type Input = DiagnosticsMsg;
    type Output = DiagnosticsOutput;

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
        let scroll = gtk::ScrolledWindow::builder().vexpand(true).build();
        let clamp = adw::Clamp::builder()
            .maximum_size(720)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .build();
        let content_box = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .spacing(24)
            .build();

        // Header with refresh button — diagnostics are a snapshot;
        // re-probing on demand is cheap and expected.
        let header_row = gtk::Box::builder()
            .orientation(gtk::Orientation::Horizontal)
            .spacing(12)
            .build();
        let title = gtk::Label::builder()
            .label("Diagnostics")
            .halign(gtk::Align::Start)
            .hexpand(true)
            .css_classes(["title-1"])
            .build();
        header_row.append(&title);
        let refresh_btn = gtk::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Re-scan diagnostics")
            .css_classes(["flat"])
            .build();
        {
            let sender = sender.clone();
            refresh_btn.connect_clicked(move |_| {
                sender.input(DiagnosticsMsg::Refresh);
            });
        }
        header_row.append(&refresh_btn);
        content_box.append(&header_row);

        let subtitle = gtk::Label::builder()
            .label(
                "These checks surface invisible conflicts \u{2014} shadowed theme copies on \
                 your search path, and apps whose decorations the theme builder cannot \
                 reach. Everything here is read-only.",
            )
            .wrap(true)
            .halign(gtk::Align::Start)
            .css_classes(["dim-label"])
            .build();
        content_box.append(&subtitle);

        clamp.set_child(Some(&content_box));
        scroll.set_child(Some(&clamp));
        root.append(&scroll);

        let model = DiagnosticsModel {
            installer: services.installer.clone(),
            decoration_probe: services.decoration_probe.clone(),
            content_box: content_box.clone(),
        };

        let widgets = view_output!();
        sender.input(DiagnosticsMsg::Refresh);
        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: DiagnosticsMsg, sender: ComponentSender<Self>) {
        match msg {
            DiagnosticsMsg::Refresh => {
                // Remove everything after the subtitle (children 0 = header,
                // 1 = subtitle). Rebuild the dynamic sections.
                let keep_count = 2;
                let mut seen = 0;
                let mut to_remove = Vec::new();
                let mut child = self.content_box.first_child();
                while let Some(c) = child {
                    let next = c.next_sibling();
                    if seen >= keep_count {
                        to_remove.push(c);
                    }
                    seen += 1;
                    child = next;
                }
                for c in to_remove {
                    self.content_box.remove(&c);
                }

                let themes = self
                    .installer
                    .list_shadowed_resources(ResourceKind::Theme)
                    .unwrap_or_default();
                let icons = self
                    .installer
                    .list_shadowed_resources(ResourceKind::Icon)
                    .unwrap_or_default();
                let cursors = self
                    .installer
                    .list_shadowed_resources(ResourceKind::Cursor)
                    .unwrap_or_default();

                let shadowed_group = build_shadowed_group(&themes, &icons, &cursors);
                self.content_box.append(&shadowed_group);

                let report = self.decoration_probe.detect_decoration_mix();
                let decoration_group = build_decoration_group(&report);
                self.content_box.append(&decoration_group);

                let _ = sender.output(DiagnosticsOutput::Toast(format!(
                    "Diagnostics refreshed: {} shadowed, {} windows",
                    themes.len() + icons.len() + cursors.len(),
                    report.windows.len(),
                )));
            }
        }
    }
}

fn build_shadowed_group(
    themes: &[ShadowedResource],
    icons: &[ShadowedResource],
    cursors: &[ShadowedResource],
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder()
        .title("Shadowed Resources")
        .description(
            "A theme / icon pack / cursor installed in more than one search path. \
             Per the freedesktop spec GTK picks the first match \u{2014} the other \
             copies are masked. If your new install isn\u{2019}t showing up, it\u{2019}s \
             probably being shadowed by an older system copy.",
        )
        .build();

    let total = themes.len() + icons.len() + cursors.len();
    if total == 0 {
        let empty = adw::ActionRow::builder()
            .title("No shadowing detected")
            .subtitle("Every installed theme, icon pack, and cursor exists in exactly one location.")
            .css_classes(["success"])
            .build();
        let icon = gtk::Image::from_icon_name("emblem-ok-symbolic");
        icon.add_css_class("success");
        empty.add_prefix(&icon);
        group.add(&empty);
        return group;
    }

    for entry in themes.iter().chain(icons.iter()).chain(cursors.iter()) {
        group.add(&build_shadowed_row(entry));
    }
    group
}

/// Build an expander row for one shadowed resource — title shows the
/// resource kind + name, the winning location sits on the row itself,
/// and every masked copy is exposed as a sub-row when the user opens
/// the expander.
fn build_shadowed_row(entry: &ShadowedResource) -> adw::ExpanderRow {
    let winner = entry.winning_location();
    let title = format!("{}: {}", entry.kind.label(), entry.name);
    let subtitle = if entry.user_install_masked() {
        format!(
            "Winning: {} (system) \u{2014} your user install is masked",
            winner.path.display()
        )
    } else {
        format!(
            "Winning: {} ({})",
            winner.path.display(),
            if winner.user_writable { "user" } else { "system" },
        )
    };
    let row = adw::ExpanderRow::builder()
        .title(title)
        .subtitle(subtitle)
        .build();

    let css = if entry.user_install_masked() {
        "warning"
    } else {
        "dim-label"
    };
    let icon = gtk::Image::from_icon_name(if entry.user_install_masked() {
        "dialog-warning-symbolic"
    } else {
        "emblem-default-symbolic"
    });
    icon.add_css_class(css);
    row.add_prefix(&icon);

    // Winning row — repeated inside the expander so the user sees the
    // full list when the row opens.
    let winner_row = adw::ActionRow::builder()
        .title(winner.path.display().to_string())
        .subtitle(if winner.user_writable {
            "User-writable \u{2014} this copy wins"
        } else {
            "System-wide \u{2014} this copy wins"
        })
        .css_classes(["property"])
        .build();
    let badge = gtk::Label::builder()
        .label("Winning")
        .css_classes(["success", "caption-heading"])
        .valign(gtk::Align::Center)
        .build();
    winner_row.add_suffix(&badge);
    row.add_row(&winner_row);

    for loc in entry.masked_locations() {
        let masked_row = adw::ActionRow::builder()
            .title(loc.path.display().to_string())
            .subtitle(if loc.user_writable {
                "User-writable \u{2014} masked by earlier copy"
            } else {
                "System-wide \u{2014} masked by earlier copy"
            })
            .build();
        let masked_badge = gtk::Label::builder()
            .label("Masked")
            .css_classes(["warning", "caption-heading"])
            .valign(gtk::Align::Center)
            .build();
        masked_row.add_suffix(&masked_badge);
        row.add_row(&masked_row);
    }

    row
}

fn build_decoration_group(report: &DecorationReport) -> adw::PreferencesGroup {
    let total = report.windows.len();
    let csd = report.csd_count();
    let ssd = report.ssd_count();
    let unknown = report.unknown_count();

    let description = if total == 0 {
        "No window information available. Install `wmctrl` for a detailed decoration \
         breakdown, or open a few apps and refresh."
            .to_string()
    } else if ssd == 0 {
        format!(
            "All {total} open windows draw their own decorations (CSD). The theme \
             builder reaches every visible app.",
        )
    } else {
        format!(
            "{csd} window(s) use client-side decorations (CSD) \u{2014} the theme \
             builder reaches these. {ssd} use server-side decorations (SSD) and \
             will not pick up headerbar styling or window radius. {unknown} \
             could not be classified.",
        )
    };

    let group = adw::PreferencesGroup::builder()
        .title("Window Decorations")
        .description(description)
        .build();

    if total == 0 {
        return group;
    }

    // Stable order: SSD first (that's what the user cares about),
    // then CSD, then Unknown; sort by class within each bucket.
    let mut sorted: Vec<_> = report.windows.iter().collect();
    sorted.sort_by(|a, b| {
        let ak = mode_order(a.mode);
        let bk = mode_order(b.mode);
        ak.cmp(&bk).then_with(|| a.app_class.cmp(&b.app_class))
    });

    for win in sorted {
        let row = adw::ActionRow::builder()
            .title(win.app_class.clone())
            .subtitle(win.title.clone().unwrap_or_default())
            .build();
        let badge = gtk::Label::builder()
            .label(win.mode.label())
            .css_classes([
                match win.mode {
                    DecorationMode::Csd => "success",
                    DecorationMode::Ssd => "warning",
                    DecorationMode::Unknown => "dim-label",
                },
                "caption-heading",
            ])
            .valign(gtk::Align::Center)
            .build();
        row.add_suffix(&badge);
        group.add(&row);
    }
    group
}

fn mode_order(mode: DecorationMode) -> u8 {
    match mode {
        DecorationMode::Ssd => 0,
        DecorationMode::Unknown => 1,
        DecorationMode::Csd => 2,
    }
}
