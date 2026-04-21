// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! GSettings-backed `ThemeSpec` builder.
//!
//! Reads the `io.github.gnomex.GnomeX` schema's `tb-*` keys plus the
//! system `org.gnome.desktop.interface accent-color` and produces a
//! validated [`gnomex_domain::ThemeSpec`]. Shared between `experiencectl`
//! (CLI theme apply) and the `FsThemeRenderTrigger` that regenerates
//! CSS after a pack is applied (GXF-061).

use gio::prelude::*;
use gnomex_app::AppError;
use gnomex_domain::{
    DashSpec, ForegroundSpec, HeaderbarSpec, HexColor, InsetSpec, LayerSeparationSpec,
    NotificationSpec, Opacity, PanelSpec, PerAppScaleOverride, Radius, ScaleFactor, ScalingSpec,
    SidebarSpec, StatusColorSpec, TextScaling, ThemeSpec, TintSpec, WidgetColorOverrides,
    WidgetStyleSpec, WindowFrameSpec,
};

const APP_SCHEMA: &str = "io.github.gnomex.GnomeX";
const IFACE_SCHEMA: &str = "org.gnome.desktop.interface";

/// Read every `tb-*` key on the `io.github.gnomex.GnomeX` schema plus
/// the GNOME accent color and fold them into a validated `ThemeSpec`.
///
/// Errors surface as `AppError::Domain` when a value violates its
/// domain invariant (e.g. opacity outside `[0.0, 1.0]`) — infra never
/// silently clamps, callers see the exact first-bad-value.
pub fn build_theme_spec_from_gsettings() -> Result<ThemeSpec, AppError> {
    let app = gio::Settings::new(APP_SCHEMA);
    let iface = gio::Settings::new(IFACE_SCHEMA);
    let accent_name = iface.string("accent-color").to_string();
    let accent_hex = accent_name_to_hex(&accent_name);

    Ok(ThemeSpec {
        window_radius: Radius::new(app.double("tb-window-radius"))?,
        element_radius: Radius::new(app.double("tb-element-radius"))?,
        panel: PanelSpec {
            radius: Radius::new(app.double("tb-panel-radius"))?,
            opacity: Opacity::from_percent(app.double("tb-panel-opacity"))?,
            tint: HexColor::new(&app.string("tb-panel-tint"))?,
        },
        dash: DashSpec {
            opacity: Opacity::from_percent(app.double("tb-dash-opacity"))?,
        },
        tint: TintSpec {
            accent_hex: HexColor::new(&accent_hex)?,
            intensity: Opacity::from_percent(app.double("tb-tint-intensity"))?,
        },
        headerbar: HeaderbarSpec {
            min_height: Radius::new(app.double("tb-headerbar-height"))?,
            shadow_intensity: Opacity::from_fraction(app.double("tb-headerbar-shadow"))?,
            circular_buttons: app.boolean("tb-circular-buttons"),
        },
        window_frame: WindowFrameSpec {
            show_shadow: app.boolean("tb-show-window-shadow"),
            inset_border: Radius::new(app.double("tb-inset-border"))?,
        },
        insets: InsetSpec {
            card_border_width: Radius::new(app.double("tb-card-border-width"))?,
            separator_opacity: Opacity::from_percent(app.double("tb-separator-opacity"))?,
            focus_ring_width: Radius::new(app.double("tb-focus-ring-width"))?,
            combo_inset: app.boolean("tb-combo-inset"),
        },
        foreground: ForegroundSpec::default(),
        layers: LayerSeparationSpec {
            headerbar_bottom: Radius::new(app.double("tb-layer-headerbar-bottom"))?,
            sidebar_divider: Radius::new(app.double("tb-layer-sidebar-divider"))?,
            content_contrast: Opacity::from_fraction(app.double("tb-layer-content-contrast"))?,
        },
        widget_style: WidgetStyleSpec {
            input_inset: Opacity::from_fraction(app.double("tb-widget-input-inset"))?,
            button_raise: Opacity::from_fraction(app.double("tb-widget-button-raise"))?,
            headerbar_gradient: Opacity::from_fraction(
                app.double("tb-widget-headerbar-gradient"),
            )?,
        },
        widget_colors: WidgetColorOverrides {
            button_bg_light: parse_optional_hex(&app.string("tb-color-button-bg-light"))?,
            button_bg_dark: parse_optional_hex(&app.string("tb-color-button-bg-dark"))?,
            entry_bg_light: parse_optional_hex(&app.string("tb-color-entry-bg-light"))?,
            entry_bg_dark: parse_optional_hex(&app.string("tb-color-entry-bg-dark"))?,
            headerbar_bg_light: parse_optional_hex(&app.string("tb-color-headerbar-bg-light"))?,
            headerbar_bg_dark: parse_optional_hex(&app.string("tb-color-headerbar-bg-dark"))?,
            sidebar_bg_light: parse_optional_hex(&app.string("tb-color-sidebar-bg-light"))?,
            sidebar_bg_dark: parse_optional_hex(&app.string("tb-color-sidebar-bg-dark"))?,
        },
        sidebar: SidebarSpec {
            opacity: Opacity::from_fraction(app.double("tb-sidebar-opacity"))?,
            fg_override: {
                let raw = app.string("tb-sidebar-fg-override").to_string();
                let trimmed = raw.trim();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(HexColor::new(trimmed)?)
                }
            },
        },
        status_colors: StatusColorSpec::default(),
        notifications: NotificationSpec {
            radius: Radius::new(app.double("tb-notification-radius"))?,
            opacity: Opacity::from_fraction(app.double("tb-notification-opacity"))?,
        },
        overview_blur: app.boolean("tb-overview-blur"),
        scaling: build_scaling_from_gsettings(&app)?,
    })
}

/// Parse the scaling `tb-*` keys into a validated `ScalingSpec`. The
/// per-app list lives in a strv as `"APP_ID:SCALE"` entries; invalid
/// entries are logged and skipped so a typo in dconf can't abort the
/// whole build.
fn build_scaling_from_gsettings(app: &gio::Settings) -> Result<ScalingSpec, AppError> {
    let text_scaling = TextScaling::new(app.double("tb-scaling-text-factor"))?;
    let mut per_app = Vec::new();
    for entry in app.strv("tb-scaling-per-app-overrides").iter() {
        let raw = entry.as_str();
        let Some((id, scale_s)) = raw.split_once(':') else {
            tracing::warn!("skipping malformed per-app-override entry: {raw}");
            continue;
        };
        let Ok(scale_f) = scale_s.trim().parse::<f64>() else {
            tracing::warn!("skipping per-app-override with unparseable scale: {raw}");
            continue;
        };
        match ScaleFactor::new(scale_f) {
            Ok(scale) => per_app.push(PerAppScaleOverride {
                app_id: id.trim().to_owned(),
                scale,
            }),
            Err(e) => tracing::warn!("rejecting per-app-override for {id}: {e}"),
        }
    }
    Ok(ScalingSpec {
        text_scaling,
        scale_monitor_framebuffer: app.boolean("tb-scaling-monitor-framebuffer"),
        x11_randr_fractional_scaling: app.boolean("tb-scaling-x11-fractional"),
        per_app_overrides: per_app,
    })
}

fn parse_optional_hex(raw: &str) -> Result<Option<HexColor>, AppError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(HexColor::new(trimmed)?))
    }
}

/// Map a GNOME `accent-color` enum value to its Adwaita hex.
/// Mirrors the CLI's translation table so both paths produce
/// byte-identical CSS for the same accent.
pub fn accent_name_to_hex(name: &str) -> String {
    match name {
        "blue" => "#3584e4",
        "teal" => "#2190a4",
        "green" => "#3a944a",
        "yellow" => "#c88800",
        "orange" => "#ed5b00",
        "red" => "#e62d42",
        "pink" => "#d56199",
        "purple" => "#9141ac",
        "slate" => "#6f8396",
        _ => "#3584e4",
    }
    .to_owned()
}
