// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Theming conflict reports — surfaced to the user when another
//! extension or configuration tool is actively fighting GNOME X's
//! output.
//!
//! These are **advisory**, not errors. The user may intentionally be
//! running Blur My Shell alongside our theme; the goal is to make
//! visible what's otherwise invisible (an extension silently
//! overriding our CSS) so the user can decide whether to reconcile.

/// Classification of a detected conflict. Each variant maps to a
/// specific external tool or extension that's known to write its own
/// appearance overrides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// User Themes extension — GNOME X writes its shell theme; User
    /// Themes also reads `org.gnome.shell.extensions.user-theme` and
    /// may apply a different theme on top of it.
    UserThemes,
    /// Blur My Shell — applies its own panel/overview/dash blur rules
    /// that compete with our accent-tinted surfaces.
    BlurMyShell,
    /// Dash to Dock — replaces the overview dash with its own widget
    /// tree, bypassing our `#dash` tint.
    DashToDock,
    /// Dash to Panel — similar, more aggressive panel replacement.
    DashToPanel,
    /// Night Theme Switcher — flips the `color-scheme` GSetting on a
    /// schedule, potentially reverting our applied theme variant.
    NightThemeSwitcher,
    /// GNOME Tweaks wrote a legacy `gtk-theme` GSetting that we no
    /// longer control via the normal appearance port.
    LegacyGtkTheme,
    /// A hand-edited `~/.config/gtk-4.0/gtk.css` exists that was not
    /// written by GNOME X (missing our managed-region marker).
    UnmanagedGtkCss,
}

impl ConflictKind {
    /// Canonical, short source label shown in the UI ("Blur My Shell",
    /// "Dash to Dock", etc.).
    pub fn source_label(&self) -> &'static str {
        match self {
            ConflictKind::UserThemes => "User Themes extension",
            ConflictKind::BlurMyShell => "Blur My Shell",
            ConflictKind::DashToDock => "Dash to Dock",
            ConflictKind::DashToPanel => "Dash to Panel",
            ConflictKind::NightThemeSwitcher => "Night Theme Switcher",
            ConflictKind::LegacyGtkTheme => "GNOME Tweaks (legacy gtk-theme)",
            ConflictKind::UnmanagedGtkCss => "Unmanaged ~/.config/gtk-4.0/gtk.css",
        }
    }
}

/// A single detected conflict. Advisory — surface in the UI so the
/// user can decide whether to disable the fighting extension, accept
/// the layered behaviour, or reconfigure.
#[derive(Debug, Clone, PartialEq)]
pub struct ConflictReport {
    pub kind: ConflictKind,
    /// Free-form description of what the conflict does to GNOME X's
    /// applied theme.
    pub description: String,
    /// User-facing suggestion for resolving the conflict.
    pub recommendation: String,
}

impl ConflictReport {
    pub fn new(
        kind: ConflictKind,
        description: impl Into<String>,
        recommendation: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            description: description.into(),
            recommendation: recommendation.into(),
        }
    }
}
