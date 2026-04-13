// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

use core::fmt;

/// Discriminant for theme technology target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ThemeType {
    Gtk3,
    Gtk4,
    Shell,
    Libadwaita,
}

impl fmt::Display for ThemeType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gtk3 => write!(f, "GTK 3"),
            Self::Gtk4 => write!(f, "GTK 4"),
            Self::Shell => write!(f, "GNOME Shell"),
            Self::Libadwaita => write!(f, "Libadwaita"),
        }
    }
}

/// Lifecycle state of a theme/icon pack on this system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentState {
    Available,
    Installed,
    Active,
}

/// Category of content on gnome-look.org (OCS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContentCategory {
    GtkTheme,
    ShellTheme,
    IconTheme,
    CursorTheme,
    Wallpaper,
}

impl ContentCategory {
    /// OCS category ID on gnome-look.org.
    pub fn ocs_id(&self) -> u32 {
        match self {
            Self::GtkTheme => 135,
            Self::ShellTheme => 134,
            Self::IconTheme => 132,
            Self::CursorTheme => 107,
            Self::Wallpaper => 300,
        }
    }
}

/// Identifier for OCS content items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ContentId(pub u64);

/// A theme, icon pack, cursor pack, or wallpaper from gnome-look.org.
#[derive(Debug, Clone)]
pub struct ContentItem {
    pub id: ContentId,
    pub name: String,
    pub description: String,
    pub creator: String,
    pub category: ContentCategory,
    pub download_url: Option<String>,
    pub preview_url: Option<String>,
    pub rating: Option<f32>,
    pub state: ContentState,
}
