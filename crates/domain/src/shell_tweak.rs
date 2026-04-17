// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Shell tweak capability model — domain knowledge about which GNOME
//! Shell behavioral tweaks each version supports. Pure policy, no IO.
//!
//! Mirrors the shape of [`crate::theme_capability`]: a flat `Id` enum,
//! per-id `Hint`s carrying a [`ControlSupport`] support level, and a
//! `Profile` keyed by GNOME major version. Reusing `ControlSupport`
//! lets the UI share its badge-rendering code between theme controls
//! and shell tweaks.

use crate::theme_capability::ControlSupport;
use crate::ShellVersion;

/// A visual grouping of shell tweaks for UI layout. Returned by
/// [`ShellTweakId::surface`] so Yellow can lay out `AdwPreferencesGroup`s
/// without inventing its own grouping or pattern-matching on ids.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellTweakSurface {
    Motion,
    Panel,
    Overview,
    Workspaces,
    WindowManagement,
    Fonts,
}

impl ShellTweakSurface {
    /// Human-readable label used as the `AdwPreferencesGroup` title.
    pub fn label(self) -> &'static str {
        match self {
            Self::Motion => "Motion & Animations",
            Self::Panel => "Panel",
            Self::Overview => "Overview",
            Self::Workspaces => "Workspaces",
            Self::WindowManagement => "Window Management",
            Self::Fonts => "Fonts & Cursor",
        }
    }

    /// Render order: surfaces appear top-to-bottom in this sequence.
    pub const ALL: [Self; 6] = [
        Self::Panel,
        Self::Overview,
        Self::Workspaces,
        Self::WindowManagement,
        Self::Fonts,
        Self::Motion,
    ];
}

/// Identifies one GNOME Shell behavioral tweak.
///
/// Grouped here by surface (panel / overview / workspaces / motion /
/// window management) so the UI can render them in matching
/// `AdwPreferencesGroup`s without baking the grouping into Yellow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellTweakId {
    // Panel surface
    TopBarPosition,
    ShowClock,
    ClockFormat,
    ShowWeekday,
    ShowBattery,

    // Overview
    OverviewHotCorner,
    WorkspaceThumbnails,
    AppsGridColumns,

    // Workspaces
    WorkspacesOnAllMonitors,
    DynamicWorkspaces,

    // Motion / accessibility
    ReducedMotion,
    WindowAnimationSpeed,
    EnableAnimations,

    // Window management (Mutter + WM preferences)
    FocusMode,
    AttachModalDialogs,
    TitlebarDoubleClickAction,
    ButtonLayout,
    NumWorkspaces,

    // Fonts / cursor
    CursorSize,
    FontAntialiasing,
    FontHinting,

    // Extension-backed (delegate to an extension when installed,
    // persist the intent in `io.github.gnomex.GnomeX` always)
    OverviewBlur,
    FloatingDock,
}

impl ShellTweakId {
    /// Canonical, stable snake_case string used as the tweak's TOML
    /// identifier. Round-trips through [`Self::from_slug`].
    ///
    /// **Stability contract:** never rename a slug that has shipped.
    /// Packs from v0.2 must still deserialize correctly on v1.x.
    pub fn slug(self) -> &'static str {
        match self {
            Self::EnableAnimations => "enable_animations",
            Self::ReducedMotion => "reduced_motion",
            Self::WindowAnimationSpeed => "window_animation_speed",
            Self::TopBarPosition => "top_bar_position",
            Self::ShowClock => "show_clock",
            Self::ClockFormat => "clock_format",
            Self::ShowWeekday => "show_weekday",
            Self::ShowBattery => "show_battery",
            Self::OverviewHotCorner => "overview_hot_corner",
            Self::WorkspaceThumbnails => "workspace_thumbnails",
            Self::AppsGridColumns => "apps_grid_columns",
            Self::WorkspacesOnAllMonitors => "workspaces_on_all_monitors",
            Self::DynamicWorkspaces => "dynamic_workspaces",
            Self::FocusMode => "focus_mode",
            Self::AttachModalDialogs => "attach_modal_dialogs",
            Self::TitlebarDoubleClickAction => "titlebar_double_click_action",
            Self::ButtonLayout => "button_layout",
            Self::NumWorkspaces => "num_workspaces",
            Self::CursorSize => "cursor_size",
            Self::FontAntialiasing => "font_antialiasing",
            Self::FontHinting => "font_hinting",
            Self::OverviewBlur => "overview_blur",
            Self::FloatingDock => "floating_dock",
        }
    }

    /// Reverse of [`Self::slug`]. Returns `None` for unknown slugs so
    /// callers can log-and-skip rather than error a whole pack import.
    pub fn from_slug(slug: &str) -> Option<Self> {
        Some(match slug {
            "enable_animations" => Self::EnableAnimations,
            "reduced_motion" => Self::ReducedMotion,
            "window_animation_speed" => Self::WindowAnimationSpeed,
            "top_bar_position" => Self::TopBarPosition,
            "show_clock" => Self::ShowClock,
            "clock_format" => Self::ClockFormat,
            "show_weekday" => Self::ShowWeekday,
            "show_battery" => Self::ShowBattery,
            "overview_hot_corner" => Self::OverviewHotCorner,
            "workspace_thumbnails" => Self::WorkspaceThumbnails,
            "apps_grid_columns" => Self::AppsGridColumns,
            "workspaces_on_all_monitors" => Self::WorkspacesOnAllMonitors,
            "dynamic_workspaces" => Self::DynamicWorkspaces,
            "focus_mode" => Self::FocusMode,
            "attach_modal_dialogs" => Self::AttachModalDialogs,
            "titlebar_double_click_action" => Self::TitlebarDoubleClickAction,
            "button_layout" => Self::ButtonLayout,
            "num_workspaces" => Self::NumWorkspaces,
            "cursor_size" => Self::CursorSize,
            "font_antialiasing" => Self::FontAntialiasing,
            "font_hinting" => Self::FontHinting,
            "overview_blur" => Self::OverviewBlur,
            "floating_dock" => Self::FloatingDock,
            _ => return None,
        })
    }

    /// Which surface this tweak belongs to.
    pub fn surface(self) -> ShellTweakSurface {
        match self {
            Self::EnableAnimations
            | Self::ReducedMotion
            | Self::WindowAnimationSpeed => ShellTweakSurface::Motion,

            Self::TopBarPosition
            | Self::ShowClock
            | Self::ClockFormat
            | Self::ShowWeekday
            | Self::ShowBattery => ShellTweakSurface::Panel,

            Self::OverviewHotCorner
            | Self::WorkspaceThumbnails
            | Self::AppsGridColumns
            | Self::OverviewBlur
            | Self::FloatingDock => ShellTweakSurface::Overview,

            Self::WorkspacesOnAllMonitors
            | Self::DynamicWorkspaces
            | Self::NumWorkspaces => ShellTweakSurface::Workspaces,

            Self::FocusMode
            | Self::AttachModalDialogs
            | Self::TitlebarDoubleClickAction
            | Self::ButtonLayout => ShellTweakSurface::WindowManagement,

            Self::CursorSize
            | Self::FontAntialiasing
            | Self::FontHinting => ShellTweakSurface::Fonts,
        }
    }

    /// UI title for a row rendering this tweak.
    pub fn label(self) -> &'static str {
        match self {
            Self::EnableAnimations => "Enable Animations",
            Self::ReducedMotion => "Reduce Motion",
            Self::WindowAnimationSpeed => "Animation Speed",
            Self::TopBarPosition => "Top Bar Position",
            Self::ShowClock => "Show Date in Clock",
            Self::ClockFormat => "Clock Format",
            Self::ShowWeekday => "Show Weekday",
            Self::ShowBattery => "Show Battery Percentage",
            Self::OverviewHotCorner => "Hot Corner",
            Self::WorkspaceThumbnails => "Workspace Thumbnails",
            Self::AppsGridColumns => "App Grid Columns",
            Self::WorkspacesOnAllMonitors => "Workspaces on All Monitors",
            Self::DynamicWorkspaces => "Dynamic Workspaces",
            Self::NumWorkspaces => "Fixed Workspace Count",
            Self::FocusMode => "Window Focus Mode",
            Self::AttachModalDialogs => "Attach Modal Dialogs",
            Self::TitlebarDoubleClickAction => "Titlebar Double-Click",
            Self::ButtonLayout => "Window Button Layout",
            Self::CursorSize => "Cursor Size",
            Self::FontAntialiasing => "Font Antialiasing",
            Self::FontHinting => "Font Hinting",
            Self::OverviewBlur => "Overview Background Blur",
            Self::FloatingDock => "Floating Dock",
        }
    }

    /// For enum-valued tweaks, the `(value, label)` pairs to present
    /// in a combo row. Returns an empty slice for non-enum tweaks.
    ///
    /// Order is render order. The first entry is the fallback when
    /// the current value doesn't match any declared option.
    pub fn enum_options(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Self::ClockFormat => &[("24h", "24-hour"), ("12h", "12-hour")],
            Self::FocusMode => &[
                ("click", "Click to Focus"),
                ("sloppy", "Focus Follows Mouse"),
                ("mouse", "Mouse (raise-on-click off)"),
            ],
            Self::TitlebarDoubleClickAction => &[
                ("toggle-maximize", "Toggle Maximize"),
                ("minimize", "Minimize"),
                ("toggle-shade", "Toggle Shade"),
                ("lower", "Lower Window"),
                ("menu", "Window Menu"),
                ("none", "Do Nothing"),
            ],
            Self::ButtonLayout => &[
                (":close", "Close only"),
                (":minimize,close", "Minimize + Close"),
                (":minimize,maximize,close", "Standard (right)"),
                ("close,maximize,minimize:", "Left side"),
            ],
            Self::FontAntialiasing => &[
                ("rgba", "Subpixel (RGBA)"),
                ("grayscale", "Grayscale"),
                ("none", "None"),
            ],
            Self::FontHinting => &[
                ("slight", "Slight"),
                ("medium", "Medium"),
                ("full", "Full"),
                ("none", "None"),
            ],
            _ => &[],
        }
    }

    /// Longer description shown as the row subtitle when the tweak is
    /// fully supported. (Degraded / Unsupported reasons take precedence
    /// and are set by the UI from the `ControlSupport` hint.)
    pub fn subtitle(self) -> &'static str {
        match self {
            Self::EnableAnimations => "Window, workspace, and overview animations",
            Self::ReducedMotion => "Minimize animations system-wide",
            Self::WindowAnimationSpeed => "Relative speed multiplier for shell animations",
            Self::TopBarPosition => "Where the GNOME panel sits on screen",
            Self::ShowClock => "Append the date next to the clock",
            Self::ClockFormat => "12-hour or 24-hour time",
            Self::ShowWeekday => "Prefix the clock with the current weekday",
            Self::ShowBattery => "Display the remaining battery as a percentage",
            Self::OverviewHotCorner => "Activate the overview by moving the cursor to the corner",
            Self::WorkspaceThumbnails => "Show workspace thumbnails along the side of the overview",
            Self::AppsGridColumns => "Number of columns in the app grid",
            Self::WorkspacesOnAllMonitors => "Extend workspaces across every display",
            Self::DynamicWorkspaces => "Add and remove workspaces automatically as needed",
            Self::NumWorkspaces => "Fixed number of workspaces when dynamic workspaces are off",
            Self::FocusMode => "How windows gain focus when the cursor enters them",
            Self::AttachModalDialogs => "Pin modal dialogs to their parent window",
            Self::TitlebarDoubleClickAction => "Action when double-clicking a window titlebar",
            Self::ButtonLayout => "Position of minimize, maximize, and close buttons",
            Self::CursorSize => "Size of the mouse cursor, in pixels",
            Self::FontAntialiasing => "How fonts are smoothed when rendered",
            Self::FontHinting => "How aggressively fonts snap to the pixel grid",
            Self::OverviewBlur => "Dim the overview background; Blur My Shell adds wallpaper blur",
            Self::FloatingDock => "Always-visible floating dock via the Dash to Dock extension",
        }
    }
}

/// Where the GNOME panel sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPosition {
    Top,
    Bottom,
}

/// Type-tagged value carried by a [`ShellTweak`]. Adapters convert to
/// and from GVariant on either side of this boundary.
#[derive(Debug, Clone, PartialEq)]
pub enum TweakValue {
    Bool(bool),
    Int(i32),
    /// A short, stable enum variant tag (e.g. `"clock"`, `"hidden"`).
    Enum(String),
    Position(PanelPosition),
}

/// Coarse-grained type tag used by the UI to pick the right row
/// widget (`AdwSwitchRow` for booleans, `AdwSpinRow` for ints, etc.).
/// Each [`ShellTweakId`] has exactly one [`TweakKind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TweakKind {
    Bool,
    Int,
    Enum,
    Position,
}

impl ShellTweakId {
    /// Which broad [`TweakKind`] this tweak carries. Source of truth
    /// for UI widget selection — kept adjacent to the `parse_for_id`
    /// table so adding a new id updates one conceptual spot.
    pub fn value_kind(self) -> TweakKind {
        match self {
            Self::EnableAnimations
            | Self::ReducedMotion
            | Self::ShowClock
            | Self::ShowWeekday
            | Self::ShowBattery
            | Self::OverviewHotCorner
            | Self::WorkspaceThumbnails
            | Self::DynamicWorkspaces
            | Self::WorkspacesOnAllMonitors
            | Self::AttachModalDialogs
            | Self::OverviewBlur
            | Self::FloatingDock => TweakKind::Bool,
            Self::WindowAnimationSpeed
            | Self::AppsGridColumns
            | Self::CursorSize
            | Self::NumWorkspaces => TweakKind::Int,
            Self::ClockFormat
            | Self::FocusMode
            | Self::TitlebarDoubleClickAction
            | Self::ButtonLayout
            | Self::FontAntialiasing
            | Self::FontHinting => TweakKind::Enum,
            Self::TopBarPosition => TweakKind::Position,
        }
    }
}

impl TweakValue {
    /// Encode the value as a short string for storage in Experience
    /// Packs. The accompanying [`ShellTweakId`] disambiguates the type
    /// on parse, so we don't need a tag prefix.
    pub fn as_toml_string(&self) -> String {
        match self {
            Self::Bool(b) => (if *b { "true" } else { "false" }).to_owned(),
            Self::Int(i) => i.to_string(),
            Self::Enum(s) => s.clone(),
            Self::Position(PanelPosition::Top) => "top".to_owned(),
            Self::Position(PanelPosition::Bottom) => "bottom".to_owned(),
        }
    }

    /// Parse a TOML-serialized value given the id it belongs to. The
    /// id tells us which domain type to decode into. Returns `None`
    /// on any parse failure so pack loading skips malformed entries.
    pub fn parse_for_id(id: ShellTweakId, raw: &str) -> Option<Self> {
        // Each id fixes the value type. Keep this mapping co-located
        // with the id → surface mapping so schema additions update
        // both at once.
        match id {
            ShellTweakId::EnableAnimations
            | ShellTweakId::ReducedMotion
            | ShellTweakId::ShowClock
            | ShellTweakId::ShowWeekday
            | ShellTweakId::ShowBattery
            | ShellTweakId::OverviewHotCorner
            | ShellTweakId::WorkspaceThumbnails
            | ShellTweakId::DynamicWorkspaces
            | ShellTweakId::WorkspacesOnAllMonitors
            | ShellTweakId::AttachModalDialogs
            | ShellTweakId::OverviewBlur
            | ShellTweakId::FloatingDock => match raw {
                "true" => Some(Self::Bool(true)),
                "false" => Some(Self::Bool(false)),
                _ => None,
            },
            ShellTweakId::WindowAnimationSpeed
            | ShellTweakId::AppsGridColumns
            | ShellTweakId::CursorSize
            | ShellTweakId::NumWorkspaces => raw.parse::<i32>().ok().map(Self::Int),
            ShellTweakId::ClockFormat
            | ShellTweakId::FocusMode
            | ShellTweakId::TitlebarDoubleClickAction
            | ShellTweakId::ButtonLayout
            | ShellTweakId::FontAntialiasing
            | ShellTweakId::FontHinting => Some(Self::Enum(raw.to_owned())),
            ShellTweakId::TopBarPosition => match raw {
                "top" => Some(Self::Position(PanelPosition::Top)),
                "bottom" => Some(Self::Position(PanelPosition::Bottom)),
                _ => None,
            },
        }
    }
}

/// One shell tweak with the value to apply or the value just read.
#[derive(Debug, Clone, PartialEq)]
pub struct ShellTweak {
    pub id: ShellTweakId,
    pub value: TweakValue,
}

/// Per-tweak metadata for a specific GNOME version: how well the tweak
/// is supported, plus an optional safe range for numeric values.
#[derive(Debug, Clone)]
pub struct TweakHint {
    pub id: ShellTweakId,
    pub support: ControlSupport,
    /// If set, integer values outside this range are clamped or
    /// degraded on this version.
    pub safe_range: Option<(i32, i32)>,
}

/// Capability profile for a specific GNOME version.
#[derive(Debug, Clone)]
pub struct ShellTweakProfile {
    pub version_major: u32,
    pub hints: Vec<TweakHint>,
}

impl ShellTweakProfile {
    /// Build the capability profile for a given shell version.
    /// Single source of truth for shell-tweak compatibility policy.
    pub fn for_version(version: &ShellVersion) -> Self {
        let major = version.major;
        let mut hints = Vec::new();

        // --- Motion / accessibility ---
        hints.push(TweakHint {
            id: ShellTweakId::EnableAnimations,
            support: ControlSupport::Full,
            safe_range: None,
        });

        // ReducedMotion: GNOME has no dedicated GSetting distinct from
        // enable-animations. We surface it but mark it Unsupported until
        // the bundled GJS extension can drive `St.Settings.slow_down_factor`
        // or the GTK4/Libadwaita portal signal individually.
        hints.push(TweakHint {
            id: ShellTweakId::ReducedMotion,
            support: ControlSupport::Unsupported {
                reason: "Redundant with Enable Animations on stock GNOME",
            },
            safe_range: None,
        });

        // WindowAnimationSpeed: no standard GSettings key; Shell extensions
        // like "Just Perfection" ship their own. Placeholder for the
        // bundled extension.
        hints.push(TweakHint {
            id: ShellTweakId::WindowAnimationSpeed,
            support: ControlSupport::Unsupported {
                reason: "Requires a Shell extension to scale animation speeds",
            },
            safe_range: Some((1, 8)),
        });

        // --- Panel surface ---
        // The clock itself is always visible on stock GNOME — only its
        // contents are configurable. "Show Clock" maps to clock-show-date.
        hints.push(TweakHint {
            id: ShellTweakId::ShowClock,
            support: ControlSupport::Full,
            safe_range: None,
        });
        hints.push(TweakHint {
            id: ShellTweakId::ClockFormat,
            support: ControlSupport::Full,
            safe_range: None,
        });
        hints.push(TweakHint {
            id: ShellTweakId::ShowWeekday,
            support: ControlSupport::Full,
            safe_range: None,
        });
        hints.push(TweakHint {
            id: ShellTweakId::ShowBattery,
            support: ControlSupport::Full,
            safe_range: None,
        });

        // TopBarPosition: stock GNOME can't move the panel. Placeholder
        // for the bundled GJS extension.
        hints.push(TweakHint {
            id: ShellTweakId::TopBarPosition,
            support: ControlSupport::Unsupported {
                reason: "Requires a Shell extension to move the panel",
            },
            safe_range: None,
        });

        // --- Workspaces ---
        hints.push(TweakHint {
            id: ShellTweakId::DynamicWorkspaces,
            support: ControlSupport::Full,
            safe_range: None,
        });
        hints.push(TweakHint {
            id: ShellTweakId::WorkspacesOnAllMonitors,
            support: ControlSupport::Full,
            safe_range: None,
        });

        // --- Window management ---
        hints.push(TweakHint {
            id: ShellTweakId::AttachModalDialogs,
            support: ControlSupport::Full,
            safe_range: None,
        });
        hints.push(TweakHint {
            id: ShellTweakId::FocusMode,
            support: ControlSupport::Full,
            safe_range: None,
        });

        // --- Overview ---
        hints.push(TweakHint {
            id: ShellTweakId::OverviewHotCorner,
            support: ControlSupport::Full,
            safe_range: None,
        });
        // WorkspaceThumbnails: no stock GSetting — gnome-shell renders
        // them unconditionally. Placeholder for bundled extension.
        hints.push(TweakHint {
            id: ShellTweakId::WorkspaceThumbnails,
            support: ControlSupport::Unsupported {
                reason: "Requires a Shell extension to toggle thumbnails",
            },
            safe_range: None,
        });
        // AppsGridColumns: the app-picker-layout key is a complex
        // GVariant we haven't codified yet. Placeholder for later.
        hints.push(TweakHint {
            id: ShellTweakId::AppsGridColumns,
            support: if major >= 46 {
                ControlSupport::Degraded {
                    reason: "Changes require a shell restart to take effect",
                }
            } else {
                ControlSupport::Unsupported {
                    reason: "Apps grid layout key was added in GNOME 46",
                }
            },
            safe_range: Some((4, 8)),
        });

        // OverviewBlur: always Full — the dim overlay from our CSS
        // generator works on bare GNOME. When Blur My Shell is installed
        // the adapter additionally enables its `overview.blur` key.
        hints.push(TweakHint {
            id: ShellTweakId::OverviewBlur,
            support: ControlSupport::Full,
            safe_range: None,
        });

        // FloatingDock: Degraded because the effect is only visible
        // when Dash to Dock is installed. The intent is still
        // persisted to GSettings and takes effect once the extension
        // appears on the system — we don't want the pack apply path
        // to drop it silently.
        hints.push(TweakHint {
            id: ShellTweakId::FloatingDock,
            support: ControlSupport::Degraded {
                reason: "Requires the Dash to Dock extension",
            },
            safe_range: None,
        });

        // --- Fonts & cursor ---
        hints.push(TweakHint {
            id: ShellTweakId::CursorSize,
            support: ControlSupport::Full,
            safe_range: Some((16, 96)),
        });
        hints.push(TweakHint {
            id: ShellTweakId::FontAntialiasing,
            support: ControlSupport::Full,
            safe_range: None,
        });
        hints.push(TweakHint {
            id: ShellTweakId::FontHinting,
            support: ControlSupport::Full,
            safe_range: None,
        });

        // --- Window management additions ---
        hints.push(TweakHint {
            id: ShellTweakId::TitlebarDoubleClickAction,
            support: ControlSupport::Full,
            safe_range: None,
        });
        hints.push(TweakHint {
            id: ShellTweakId::ButtonLayout,
            support: ControlSupport::Full,
            safe_range: None,
        });

        // --- Workspaces additions ---
        hints.push(TweakHint {
            id: ShellTweakId::NumWorkspaces,
            support: ControlSupport::Full,
            safe_range: Some((1, 8)),
        });

        Self {
            version_major: major,
            hints,
        }
    }

    pub fn hint_for(&self, id: ShellTweakId) -> Option<&TweakHint> {
        self.hints.iter().find(|h| h.id == id)
    }

    /// Tweaks supported (Full or Degraded) on this version, in the
    /// declaration order. `Unsupported` ones are filtered out.
    pub fn supported_ids(&self) -> Vec<ShellTweakId> {
        self.hints
            .iter()
            .filter(|h| !h.support.is_unsupported())
            .map(|h| h.id)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enable_animations_is_full_on_every_version() {
        for major in [45u32, 46, 47, 48, 50] {
            let p = ShellTweakProfile::for_version(&ShellVersion::new(major, 0));
            let hint = p.hint_for(ShellTweakId::EnableAnimations).unwrap();
            assert!(hint.support.is_full(), "version {major}");
        }
    }

    #[test]
    fn apps_grid_columns_unsupported_on_45() {
        let p = ShellTweakProfile::for_version(&ShellVersion::new(45, 0));
        let hint = p.hint_for(ShellTweakId::AppsGridColumns).unwrap();
        assert!(hint.support.is_unsupported());
    }

    #[test]
    fn apps_grid_columns_degraded_on_46_plus() {
        // Changes require a shell restart — that's the Degraded reason.
        for major in [46u32, 47, 50] {
            let p = ShellTweakProfile::for_version(&ShellVersion::new(major, 0));
            let hint = p.hint_for(ShellTweakId::AppsGridColumns).unwrap();
            assert!(
                matches!(hint.support, ControlSupport::Degraded { .. }),
                "version {major}"
            );
        }
    }

    #[test]
    fn reduced_motion_unsupported_until_bundled_extension() {
        for major in [45u32, 46, 47, 50] {
            let p = ShellTweakProfile::for_version(&ShellVersion::new(major, 0));
            assert!(p
                .hint_for(ShellTweakId::ReducedMotion)
                .unwrap()
                .support
                .is_unsupported());
        }
    }

    #[test]
    fn top_bar_position_unsupported_until_bundled_extension() {
        for major in [45u32, 46, 47, 50] {
            let p = ShellTweakProfile::for_version(&ShellVersion::new(major, 0));
            assert!(p
                .hint_for(ShellTweakId::TopBarPosition)
                .unwrap()
                .support
                .is_unsupported());
        }
    }

    #[test]
    fn supported_ids_excludes_unsupported() {
        let p = ShellTweakProfile::for_version(&ShellVersion::new(47, 0));
        assert!(!p.supported_ids().contains(&ShellTweakId::TopBarPosition));
        assert!(p.supported_ids().contains(&ShellTweakId::EnableAnimations));
    }

    #[test]
    fn every_id_round_trips_through_slug() {
        // Exhaustive across every variant so adding a new id without
        // updating both arms fails the suite.
        let all = [
            ShellTweakId::EnableAnimations,
            ShellTweakId::ReducedMotion,
            ShellTweakId::WindowAnimationSpeed,
            ShellTweakId::TopBarPosition,
            ShellTweakId::ShowClock,
            ShellTweakId::ClockFormat,
            ShellTweakId::ShowWeekday,
            ShellTweakId::ShowBattery,
            ShellTweakId::OverviewHotCorner,
            ShellTweakId::WorkspaceThumbnails,
            ShellTweakId::AppsGridColumns,
            ShellTweakId::WorkspacesOnAllMonitors,
            ShellTweakId::DynamicWorkspaces,
            ShellTweakId::NumWorkspaces,
            ShellTweakId::FocusMode,
            ShellTweakId::AttachModalDialogs,
            ShellTweakId::TitlebarDoubleClickAction,
            ShellTweakId::ButtonLayout,
            ShellTweakId::CursorSize,
            ShellTweakId::FontAntialiasing,
            ShellTweakId::FontHinting,
            ShellTweakId::OverviewBlur,
            ShellTweakId::FloatingDock,
        ];
        for id in all {
            assert_eq!(ShellTweakId::from_slug(id.slug()), Some(id), "{id:?}");
        }
    }

    #[test]
    fn from_slug_returns_none_on_unknown() {
        assert_eq!(ShellTweakId::from_slug("not_a_real_tweak"), None);
        assert_eq!(ShellTweakId::from_slug(""), None);
    }

    #[test]
    fn tweak_value_round_trips_for_each_type() {
        let cases = [
            (ShellTweakId::EnableAnimations, TweakValue::Bool(true)),
            (ShellTweakId::EnableAnimations, TweakValue::Bool(false)),
            (ShellTweakId::AppsGridColumns, TweakValue::Int(6)),
            (
                ShellTweakId::ClockFormat,
                TweakValue::Enum("24h".to_owned()),
            ),
            (
                ShellTweakId::FocusMode,
                TweakValue::Enum("sloppy".to_owned()),
            ),
            (
                ShellTweakId::TopBarPosition,
                TweakValue::Position(PanelPosition::Bottom),
            ),
        ];
        for (id, value) in cases {
            let encoded = value.as_toml_string();
            let decoded = TweakValue::parse_for_id(id, &encoded);
            assert_eq!(decoded, Some(value.clone()), "roundtrip {id:?}");
        }
    }

    #[test]
    fn tweak_value_parse_rejects_malformed() {
        assert_eq!(
            TweakValue::parse_for_id(ShellTweakId::EnableAnimations, "maybe"),
            None
        );
        assert_eq!(
            TweakValue::parse_for_id(ShellTweakId::AppsGridColumns, "not_a_number"),
            None
        );
        assert_eq!(
            TweakValue::parse_for_id(ShellTweakId::TopBarPosition, "diagonal"),
            None
        );
    }
}
