// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Theme specification — version-independent value objects for theming.
//!
//! These types capture *what* the user wants (radii, opacity, tint) without
//! any knowledge of *how* it maps to CSS selectors on a given GNOME version.

use crate::DomainError;

/// A validated border-radius in pixels (0–48).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Radius(f64);

impl Radius {
    pub fn new(px: f64) -> Result<Self, DomainError> {
        if !(0.0..=48.0).contains(&px) {
            return Err(DomainError::InvalidRadius(px));
        }
        Ok(Self(px))
    }

    pub fn as_px(&self) -> f64 {
        self.0
    }

    pub fn as_i32(&self) -> i32 {
        self.0 as i32
    }
}

/// A validated opacity (stored as 0.0–1.0 fraction, constructed from 0–100%).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opacity(f64);

impl Opacity {
    pub fn from_percent(pct: f64) -> Result<Self, DomainError> {
        if !(0.0..=100.0).contains(&pct) {
            return Err(DomainError::InvalidOpacity(pct));
        }
        Ok(Self(pct / 100.0))
    }

    pub fn from_fraction(f: f64) -> Result<Self, DomainError> {
        if !(0.0..=1.0).contains(&f) {
            return Err(DomainError::InvalidOpacity(f * 100.0));
        }
        Ok(Self(f))
    }

    pub fn as_fraction(&self) -> f64 {
        self.0
    }

    pub fn as_percent(&self) -> f64 {
        self.0 * 100.0
    }
}

/// A validated `#rrggbb` hex color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexColor(String);

impl HexColor {
    pub fn new(hex: &str) -> Result<Self, DomainError> {
        let hex = hex.trim();
        if hex.len() != 7
            || !hex.starts_with('#')
            || !hex[1..].chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(DomainError::InvalidColor(hex.into()));
        }
        Ok(Self(hex.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn to_rgb(&self) -> (u8, u8, u8) {
        let r = u8::from_str_radix(&self.0[1..3], 16).unwrap_or(0);
        let g = u8::from_str_radix(&self.0[3..5], 16).unwrap_or(0);
        let b = u8::from_str_radix(&self.0[5..7], 16).unwrap_or(0);
        (r, g, b)
    }
}

/// Panel customization values.
#[derive(Debug, Clone, PartialEq)]
pub struct PanelSpec {
    pub radius: Radius,
    pub opacity: Opacity,
    pub tint: HexColor,
}

/// Dash customization values.
#[derive(Debug, Clone, PartialEq)]
pub struct DashSpec {
    pub opacity: Opacity,
}

/// Accent tint customization values.
#[derive(Debug, Clone, PartialEq)]
pub struct TintSpec {
    pub accent_hex: HexColor,
    pub intensity: Opacity,
}

/// Headerbar / CSD customization values.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaderbarSpec {
    /// Minimum height in pixels (default 47, compact ~30).
    pub min_height: Radius,
    /// Drop shadow intensity below the headerbar (0.0 = flat, 1.0 = full).
    pub shadow_intensity: Opacity,
    /// Whether titlebar close/min/max buttons are circular.
    pub circular_buttons: bool,
}

/// Window frame customization values.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowFrameSpec {
    /// Whether to show the CSD drop shadow around windows.
    pub show_shadow: bool,
    /// Inset border width (0 = no visible border, 1 = thin line).
    pub inset_border: Radius,
}

/// Visual inset controls for cards, separators, focus rings.
#[derive(Debug, Clone, PartialEq)]
pub struct InsetSpec {
    pub card_border_width: Radius,
    pub separator_opacity: Opacity,
    pub focus_ring_width: Radius,
    pub combo_inset: bool,
}

/// Foreground / text color overrides (None = use Adwaita defaults).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ForegroundSpec {
    pub window_fg: Option<HexColor>,
    pub view_fg: Option<HexColor>,
    pub headerbar_fg: Option<HexColor>,
    pub headerbar_border: Option<HexColor>,
}

/// "Restore traditional widget styling" opt-ins. Modern Adwaita draws
/// flat, chromeless inputs, buttons, and headerbars. Users coming from
/// GNOME 3.x / pre-Libadwaita desktops often want some of that chrome
/// back. Each knob is a 0.0–1.0 intensity; 0.0 emits no CSS (byte-
/// identical to current output) and higher values scale the effect.
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetStyleSpec {
    /// Inset strength for input fields. Above 0, `entry` widgets gain
    /// a visible background and border distinct from the surrounding
    /// surface. At 1.0 inputs read as clearly depressed.
    pub input_inset: Opacity,
    /// Raised-button affordance. Above 0, `button` widgets gain a
    /// subtle border + shadow so they read as pressable rather than
    /// flat text. At 1.0 buttons look clearly 3-D.
    pub button_raise: Opacity,
    /// Headerbar / toolbar gradient intensity. Above 0, a top→bottom
    /// linear gradient is applied. Conflicts with Adwaita's flat
    /// philosophy — use sparingly.
    pub headerbar_gradient: Opacity,
}

impl Default for WidgetStyleSpec {
    fn default() -> Self {
        Self {
            input_inset: Opacity(0.0),
            button_raise: Opacity(0.0),
            headerbar_gradient: Opacity(0.0),
        }
    }
}

/// Explicit visual separators between the three major layers of a
/// Libadwaita window — headerbar / sidebar / content.
///
/// Modern Adwaita blends all three into a single flat surface; users
/// who want a more "traditional" desktop silhouette need to be able
/// to dial in visible boundary lines without editing CSS by hand.
#[derive(Debug, Clone, PartialEq)]
pub struct LayerSeparationSpec {
    /// Width of the line drawn under the headerbar (0 = flush/blended,
    /// Libadwaita default). Rendered in `@borders` / `@headerbar_border_color`.
    pub headerbar_bottom: Radius,
    /// Width of the vertical rule between the sidebar and the main
    /// content column (0 = blended). Rendered in `@borders`.
    pub sidebar_divider: Radius,
    /// Extra strength for the content-view backdrop contrast
    /// (0.0 = match Adwaita defaults, 1.0 = maximally darkened/lightened
    /// relative to the window background). Exposed as a separate knob
    /// from the global `TintSpec::intensity` so the user can sharpen
    /// layer boundaries without re-tinting every other surface.
    pub content_contrast: Opacity,
}

impl Default for LayerSeparationSpec {
    fn default() -> Self {
        Self {
            headerbar_bottom: Radius(0.0),
            sidebar_divider: Radius(0.0),
            content_contrast: Opacity(0.0),
        }
    }
}

/// Sidebar-specific controls. Nautilus, Files, Disks, Settings, and any
/// AdwOverlaySplitView app render a left nav sidebar; this spec groups
/// the knobs that govern it.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarSpec {
    /// Background opacity. 1.0 = fully opaque (default Adwaita
    /// behaviour); < 1.0 wraps `sidebar_bg_color` in `alpha()` so the
    /// wallpaper or compositor blur shows through.
    pub opacity: Opacity,
    /// Optional foreground override for sidebar text. `None` keeps the
    /// Adwaita default `sidebar_fg_color`.
    pub fg_override: Option<HexColor>,
}

impl Default for SidebarSpec {
    fn default() -> Self {
        Self {
            opacity: Opacity(1.0),
            fg_override: None,
        }
    }
}

/// Per-widget, per-scheme background-colour overrides.
///
/// Each field is scoped by `@media (prefers-color-scheme: ...)` so the
/// user can, for example, dial a warm orange button in light mode and
/// a cool blue button in dark mode. `None` means "follow the current
/// Adwaita / tinting behaviour unchanged".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WidgetColorOverrides {
    pub button_bg_light: Option<HexColor>,
    pub button_bg_dark: Option<HexColor>,
    pub entry_bg_light: Option<HexColor>,
    pub entry_bg_dark: Option<HexColor>,
    pub headerbar_bg_light: Option<HexColor>,
    pub headerbar_bg_dark: Option<HexColor>,
    pub sidebar_bg_light: Option<HexColor>,
    pub sidebar_bg_dark: Option<HexColor>,
}

impl WidgetColorOverrides {
    pub fn is_empty(&self) -> bool {
        self.button_bg_light.is_none()
            && self.button_bg_dark.is_none()
            && self.entry_bg_light.is_none()
            && self.entry_bg_dark.is_none()
            && self.headerbar_bg_light.is_none()
            && self.headerbar_bg_dark.is_none()
            && self.sidebar_bg_light.is_none()
            && self.sidebar_bg_dark.is_none()
    }
}

/// Semantic status color overrides (None = use Adwaita defaults).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct StatusColorSpec {
    pub destructive: Option<HexColor>,
    pub success: Option<HexColor>,
    pub warning: Option<HexColor>,
    pub error: Option<HexColor>,
}

/// Notification / calendar / OSD shell styling.
#[derive(Debug, Clone, PartialEq)]
pub struct NotificationSpec {
    pub radius: Radius,
    pub opacity: Opacity,
}

/// HiDPI / scaling controls that live orthogonal to the rest of the
/// theme. The three user-facing problems this addresses:
///
/// - **GXF-050 (fractional scaling fine-tuning)** — flipping Mutter's
///   experimental `scale-monitor-framebuffer` and
///   `x11-randr-fractional-scaling` flags without hand-editing dconf.
/// - **GXF-051 (per-app scaling overrides)** — registering a scale
///   factor for a single `.desktop` app so it launches with the
///   appropriate `GDK_SCALE` / `--force-device-scale-factor`.
/// - **GXF-053 (text-vs-widget scaling decoupling)** — exposing
///   `text-scaling-factor` independently from the global monitor
///   scale.
///
/// Defaults are chosen so nothing is written when the user hasn't
/// opted in: `text_scaling=1.0`, empty flag set, no per-app overrides.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ScalingSpec {
    /// `org.gnome.desktop.interface text-scaling-factor`. 1.0 = no
    /// change (Adwaita default). Range 0.5..=3.0 is enforced at
    /// construction time, matching gnome-control-center's bounds.
    pub text_scaling: TextScaling,
    /// Whether to write `scale-monitor-framebuffer` into
    /// `org.gnome.mutter experimental-features`. Required on Wayland
    /// for fractional scales to render crisp rather than bilinear-
    /// scaled.
    pub scale_monitor_framebuffer: bool,
    /// Whether to write `x11-randr-fractional-scaling` into the same
    /// strv. X11-session sibling of the above.
    pub x11_randr_fractional_scaling: bool,
    /// Per-app launcher overrides. Each entry requests that a copy of
    /// the named `.desktop` file be written to
    /// `~/.local/share/applications/` with `Exec=` prefixed by
    /// `env GDK_SCALE=<factor>`.
    pub per_app_overrides: Vec<PerAppScaleOverride>,
}

/// Validated `text-scaling-factor`. GNOME's settings daemon accepts
/// 0.5..=3.0 (clamped higher than that on most setups); we mirror the
/// bound to catch garbage values before we write to GSettings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextScaling(f64);

impl TextScaling {
    pub fn new(v: f64) -> Result<Self, DomainError> {
        if !(0.5..=3.0).contains(&v) {
            return Err(DomainError::InvalidTextScaling(v));
        }
        Ok(Self(v))
    }

    pub fn as_f64(self) -> f64 {
        self.0
    }

    /// Whether the value is the Adwaita default (no GSettings write
    /// needed). Written as a method so callers don't float-compare.
    pub fn is_default(self) -> bool {
        (self.0 - 1.0).abs() < f64::EPSILON
    }
}

impl Default for TextScaling {
    fn default() -> Self {
        Self(1.0)
    }
}

/// A single per-app scale override. Identified by the `.desktop`
/// file's canonical id (e.g. `org.gnome.Nautilus`); the adapter
/// resolves that to a file under `/usr/share/applications/` or
/// `~/.local/share/applications/` and writes a copy with the
/// modified `Exec` line.
#[derive(Debug, Clone, PartialEq)]
pub struct PerAppScaleOverride {
    pub app_id: String,
    pub scale: ScaleFactor,
}

/// A validated HiDPI scale factor. Restricted to the set of values
/// gnome-control-center exposes for monitor scaling plus 0.5 for
/// hi-dpi-on-lodpi-screen downsampling: 0.5, 1.0, 1.25, 1.5, 1.75,
/// 2.0. Any other value is rejected so the adapter can't generate a
/// `.desktop` file with `GDK_SCALE=0.873`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScaleFactor(f64);

impl ScaleFactor {
    pub const ALLOWED: &'static [f64] = &[0.5, 1.0, 1.25, 1.5, 1.75, 2.0];

    pub fn new(v: f64) -> Result<Self, DomainError> {
        if Self::ALLOWED.iter().any(|a| (*a - v).abs() < 1e-6) {
            Ok(Self(v))
        } else {
            Err(DomainError::InvalidScaleFactor(v))
        }
    }

    pub fn as_f64(self) -> f64 {
        self.0
    }
}

/// A complete, version-independent theme specification.
///
/// Contains all the values the user has set via the Theme Builder.
/// The version-specific CSS adapters in the infra layer interpret these
/// values using the correct selectors for the running GNOME version.
#[derive(Debug, Clone, PartialEq)]
pub struct ThemeSpec {
    pub window_radius: Radius,
    pub element_radius: Radius,
    pub panel: PanelSpec,
    pub dash: DashSpec,
    pub tint: TintSpec,
    pub headerbar: HeaderbarSpec,
    pub window_frame: WindowFrameSpec,
    pub insets: InsetSpec,
    pub foreground: ForegroundSpec,
    pub sidebar: SidebarSpec,
    pub layers: LayerSeparationSpec,
    pub widget_style: WidgetStyleSpec,
    pub widget_colors: WidgetColorOverrides,
    pub status_colors: StatusColorSpec,
    pub notifications: NotificationSpec,
    pub overview_blur: bool,
    /// HiDPI / fractional-scaling / per-app override controls.
    /// Defaulted so untouched packs emit no GSettings writes.
    pub scaling: ScalingSpec,
}

impl ThemeSpec {
    pub fn defaults() -> Self {
        Self {
            window_radius: Radius(12.0),
            element_radius: Radius(6.0),
            panel: PanelSpec {
                radius: Radius(0.0),
                opacity: Opacity(0.8),
                tint: HexColor("#1a1a1e".into()),
            },
            dash: DashSpec {
                opacity: Opacity(0.7),
            },
            tint: TintSpec {
                accent_hex: HexColor("#3584e4".into()),
                intensity: Opacity(0.05),
            },
            headerbar: HeaderbarSpec {
                min_height: Radius(47.0),
                shadow_intensity: Opacity(1.0),
                circular_buttons: false,
            },
            window_frame: WindowFrameSpec {
                show_shadow: true,
                inset_border: Radius(0.0),
            },
            insets: InsetSpec {
                card_border_width: Radius(1.0),
                separator_opacity: Opacity(1.0),
                focus_ring_width: Radius(2.0),
                combo_inset: true,
            },
            foreground: ForegroundSpec::default(),
            sidebar: SidebarSpec::default(),
            layers: LayerSeparationSpec::default(),
            widget_style: WidgetStyleSpec::default(),
            widget_colors: WidgetColorOverrides::default(),
            status_colors: StatusColorSpec::default(),
            notifications: NotificationSpec {
                radius: Radius(12.0),
                opacity: Opacity(0.95),
            },
            overview_blur: true,
            scaling: ScalingSpec::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn radius_valid() {
        assert!(Radius::new(0.0).is_ok());
        assert!(Radius::new(12.0).is_ok());
        assert!(Radius::new(48.0).is_ok());
    }

    #[test]
    fn radius_invalid() {
        assert!(Radius::new(-1.0).is_err());
        assert!(Radius::new(49.0).is_err());
    }

    #[test]
    fn opacity_valid() {
        assert!(Opacity::from_percent(0.0).is_ok());
        assert!(Opacity::from_percent(50.0).is_ok());
        assert!(Opacity::from_percent(100.0).is_ok());
        assert_eq!(Opacity::from_percent(80.0).unwrap().as_fraction(), 0.8);
    }

    #[test]
    fn opacity_invalid() {
        assert!(Opacity::from_percent(-1.0).is_err());
        assert!(Opacity::from_percent(101.0).is_err());
    }

    #[test]
    fn hex_color_valid() {
        let c = HexColor::new("#3584e4").unwrap();
        assert_eq!(c.as_str(), "#3584e4");
        assert_eq!(c.to_rgb(), (0x35, 0x84, 0xe4));
    }

    #[test]
    fn hex_color_invalid() {
        assert!(HexColor::new("red").is_err());
        assert!(HexColor::new("#12345").is_err());
        assert!(HexColor::new("#gggggg").is_err());
    }

    #[test]
    fn theme_spec_defaults() {
        let spec = ThemeSpec::defaults();
        assert_eq!(spec.window_radius.as_i32(), 12);
        assert_eq!(spec.panel.opacity.as_percent(), 80.0);
    }
}
