// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Reusable GNOME color picker — 9 accent swatches + custom color button.
//! Supports external selection (wallpaper swatches) with proper visual feedback.

use adw::prelude::*;

const GNOME_COLORS: &[(&str, &str, &str)] = &[
    ("blue", "Blue", "#3584e4"),
    ("teal", "Teal", "#2190a4"),
    ("green", "Green", "#3a944a"),
    ("yellow", "Yellow", "#c88800"),
    ("orange", "Orange", "#ed5b00"),
    ("red", "Red", "#e62d42"),
    ("pink", "Pink", "#d56199"),
    ("purple", "Purple", "#9141ac"),
    ("slate", "Slate", "#6f8396"),
];

// CSS class for the custom color ring
const CUSTOM_ACTIVE_CLASS: &str = "gnomex-custom-active";

/// Ensure the custom-active CSS is registered (idempotent).
fn ensure_custom_css() {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&format!(
            ".{CUSTOM_ACTIVE_CLASS} {{ \
                border: 2px solid @theme_fg_color; \
                border-radius: 6px; \
                padding: 1px; \
            }}"
        ));
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().unwrap(),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });
}

/// Build a color picker box with 9 GNOME accent swatches + a custom color button.
///
/// `current_hex` — initial selected color.
/// `on_selected` — called with `(accent_id_or_hex, hex_color)` when a color is picked.
pub fn build_color_picker(
    current_hex: &str,
    on_selected: impl Fn(String, String) + Clone + 'static,
) -> gtk::Box {
    ensure_custom_css();

    let container = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .valign(gtk::Align::Center)
        .build();

    let current_id = GNOME_COLORS
        .iter()
        .find(|(_, _, hex)| hex.eq_ignore_ascii_case(current_hex))
        .map(|(id, _, _)| *id);

    // Custom color button (built first so toggles can reference it)
    let custom_btn = gtk::ColorDialogButton::builder()
        .valign(gtk::Align::Center)
        .tooltip_text("Custom color")
        .build();
    let dialog = gtk::ColorDialog::builder()
        .title("Custom Color")
        .with_alpha(false)
        .build();
    custom_btn.set_dialog(&dialog);

    if current_id.is_none() && !current_hex.is_empty() {
        if let Ok(rgba) = gtk::gdk::RGBA::parse(current_hex) {
            custom_btn.set_rgba(&rgba);
        }
        custom_btn.add_css_class(CUSTOM_ACTIVE_CLASS);
    }

    // Build 9 GNOME accent toggle buttons
    for (i, &(id, label, hex)) in GNOME_COLORS.iter().enumerate() {
        let btn = gtk::ToggleButton::builder()
            .width_request(28)
            .height_request(28)
            .tooltip_text(label)
            .active(current_id == Some(id))
            .build();

        let css_class = format!("gnomex-color-{id}");
        btn.add_css_class(&css_class);

        let provider = gtk::CssProvider::new();
        provider.load_from_string(&format!(
            ".{css_class} {{ \
                background: {hex}; \
                border-radius: 50%; \
                min-width: 24px; \
                min-height: 24px; \
                padding: 0; \
                border: 2px solid transparent; \
            }} \
            .{css_class}:checked {{ \
                border-color: @theme_fg_color; \
            }}"
        ));
        gtk::style_context_add_provider_for_display(
            &gtk::gdk::Display::default().unwrap(),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        if i > 0 {
            if let Some(first) = container.first_child() {
                btn.set_group(first.downcast_ref::<gtk::ToggleButton>());
            }
        }

        let cb = on_selected.clone();
        let accent_id = id.to_owned();
        let hex_owned = hex.to_owned();
        let custom_ref = custom_btn.clone();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                // Remove custom ring when a standard color is picked
                custom_ref.remove_css_class(CUSTOM_ACTIVE_CLASS);
                cb(accent_id.clone(), hex_owned.clone());
            }
        });

        container.append(&btn);
    }

    // Custom color handler — deselects all toggles, shows ring on custom button
    {
        let cb = on_selected.clone();
        let container_ref = container.clone();
        let custom_ref = custom_btn.clone();
        custom_btn.connect_rgba_notify(move |btn| {
            let rgba = btn.rgba();
            let hex = format!(
                "#{:02x}{:02x}{:02x}",
                (rgba.red() * 255.0) as u8,
                (rgba.green() * 255.0) as u8,
                (rgba.blue() * 255.0) as u8,
            );
            let id = GNOME_COLORS
                .iter()
                .find(|(_, _, h)| h.eq_ignore_ascii_case(&hex))
                .map(|(id, _, _)| id.to_string())
                .unwrap_or_else(|| hex.clone());

            // Deselect all toggle buttons
            deselect_toggles(&container_ref);
            // Show active ring on custom button
            custom_ref.add_css_class(CUSTOM_ACTIVE_CLASS);

            cb(id, hex);
        });
    }

    container.append(&custom_btn);
    container
}

/// Deselect all toggle buttons in a color picker container.
pub fn deselect_all(picker: &gtk::Box) {
    deselect_toggles(picker);
    // Also remove custom active ring
    let mut child = picker.first_child();
    while let Some(widget) = child {
        if widget.downcast_ref::<gtk::ToggleButton>().is_none() {
            // It's the custom button container
            widget.remove_css_class(CUSTOM_ACTIVE_CLASS);
        }
        child = widget.next_sibling();
    }
}

/// Deselect only the toggle buttons (not the custom button).
fn deselect_toggles(picker: &gtk::Box) {
    let mut child = picker.first_child();
    while let Some(widget) = child {
        if let Some(toggle) = widget.downcast_ref::<gtk::ToggleButton>() {
            toggle.set_active(false);
        }
        child = widget.next_sibling();
    }
}

/// Lookup the hex value for a GNOME accent color ID.
pub fn accent_id_to_hex(id: &str) -> String {
    GNOME_COLORS
        .iter()
        .find(|(cid, _, _)| *cid == id)
        .map(|(_, _, hex)| hex.to_string())
        .unwrap_or_else(|| id.to_owned())
}

/// Lookup the GNOME accent ID for a hex color, or return the hex itself.
#[allow(dead_code)]
pub fn hex_to_accent_id(hex: &str) -> String {
    GNOME_COLORS
        .iter()
        .find(|(_, _, h)| h.eq_ignore_ascii_case(hex))
        .map(|(id, _, _)| id.to_string())
        .unwrap_or_else(|| hex.to_owned())
}
