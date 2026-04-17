// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! Shared helpers for `ShellCustomizer` adapters.
//!
//! Adapters in this tree all do the same kind of thing: map a
//! [`ShellTweakId`] to a `(schema, key)` pair, then read or write a
//! [`TweakValue`] through GVariant. The conversions are exhausting to
//! reimplement per version, so they live here.

use gio::glib::Variant;
use gio::prelude::*;
use gnomex_app::ports::{BlurMyShellController, FloatingDockController};
use gnomex_app::AppError;
use gnomex_domain::{PanelPosition, ShellTweak, ShellTweakId, TweakValue};
use std::sync::Arc;

const IFACE: &str = "org.gnome.desktop.interface";
const MUTTER: &str = "org.gnome.mutter";
const WM: &str = "org.gnome.desktop.wm.preferences";
const GNOMEX: &str = "io.github.gnomex.GnomeX";

/// A `(schema, key)` reference to a single GSettings entry.
#[derive(Debug, Clone, Copy)]
pub struct GSettingsKey {
    pub schema: &'static str,
    pub key: &'static str,
}

impl GSettingsKey {
    pub const fn new(schema: &'static str, key: &'static str) -> Self {
        Self { schema, key }
    }
}

/// The baseline key map shared by every version adapter. Version
/// adapters call this first; they can override by returning their
/// own mapping *before* delegating, or return `None` themselves to
/// withhold a tweak on a version that can't support it yet.
///
/// Adding a new tweak here makes it immediately available on every
/// version that doesn't override it — which is usually what we want,
/// because GNOME's stable `org.gnome.desktop.interface` schema rarely
/// drifts between releases.
pub fn baseline_key_for(id: ShellTweakId) -> Option<(GSettingsKey, ValueShape)> {
    Some(match id {
        // --- Motion ---
        ShellTweakId::EnableAnimations => (
            GSettingsKey::new(IFACE, "enable-animations"),
            ValueShape::Bool,
        ),

        // --- Clock / panel readouts ---
        // "Show Clock" maps to clock-show-date: GNOME's clock is always
        // visible on stock shell; toggling the date content is the
        // closest honest equivalent.
        ShellTweakId::ShowClock => (
            GSettingsKey::new(IFACE, "clock-show-date"),
            ValueShape::Bool,
        ),
        ShellTweakId::ClockFormat => (
            GSettingsKey::new(IFACE, "clock-format"),
            ValueShape::StringEnum,
        ),
        ShellTweakId::ShowWeekday => (
            GSettingsKey::new(IFACE, "clock-show-weekday"),
            ValueShape::Bool,
        ),
        ShellTweakId::ShowBattery => (
            GSettingsKey::new(IFACE, "show-battery-percentage"),
            ValueShape::Bool,
        ),

        // --- Overview ---
        ShellTweakId::OverviewHotCorner => (
            GSettingsKey::new(IFACE, "enable-hot-corners"),
            ValueShape::Bool,
        ),

        // --- Workspaces ---
        ShellTweakId::DynamicWorkspaces => (
            GSettingsKey::new(MUTTER, "dynamic-workspaces"),
            ValueShape::Bool,
        ),
        // Upstream key is "workspaces-only-on-primary" — a *negative*
        // expression of the same intent. We invert in both directions.
        ShellTweakId::WorkspacesOnAllMonitors => (
            GSettingsKey::new(MUTTER, "workspaces-only-on-primary"),
            ValueShape::InvertedBool,
        ),

        // --- Window management ---
        ShellTweakId::AttachModalDialogs => (
            GSettingsKey::new(MUTTER, "attach-modal-dialogs"),
            ValueShape::Bool,
        ),
        ShellTweakId::FocusMode => (
            GSettingsKey::new(WM, "focus-mode"),
            ValueShape::StringEnum,
        ),
        ShellTweakId::TitlebarDoubleClickAction => (
            GSettingsKey::new(WM, "action-double-click-titlebar"),
            ValueShape::StringEnum,
        ),
        ShellTweakId::ButtonLayout => (
            GSettingsKey::new(WM, "button-layout"),
            ValueShape::StringEnum,
        ),
        ShellTweakId::NumWorkspaces => (
            GSettingsKey::new(WM, "num-workspaces"),
            ValueShape::Int32,
        ),

        // --- Fonts & cursor ---
        ShellTweakId::CursorSize => (
            GSettingsKey::new(IFACE, "cursor-size"),
            ValueShape::Int32,
        ),
        ShellTweakId::FontAntialiasing => (
            GSettingsKey::new(IFACE, "font-antialiasing"),
            ValueShape::StringEnum,
        ),
        ShellTweakId::FontHinting => (
            GSettingsKey::new(IFACE, "font-hinting"),
            ValueShape::StringEnum,
        ),

        // --- Extension-backed: persist the intent in our own schema
        // so it round-trips through packs; the adapter additionally
        // calls the relevant controller (BlurMyShell / FloatingDock)
        // to drive the visible effect when available.
        ShellTweakId::OverviewBlur => (
            GSettingsKey::new(GNOMEX, "tb-overview-blur"),
            ValueShape::Bool,
        ),
        ShellTweakId::FloatingDock => (
            GSettingsKey::new(GNOMEX, "tb-floating-dock-enabled"),
            ValueShape::Bool,
        ),

        // Tweaks the baseline deliberately omits (unsupported on stock
        // GNOME, waiting for bundled extension):
        ShellTweakId::TopBarPosition
        | ShellTweakId::ReducedMotion
        | ShellTweakId::WindowAnimationSpeed
        | ShellTweakId::WorkspaceThumbnails
        | ShellTweakId::AppsGridColumns => return None,
    })
}

/// The set of ids `baseline_key_for` resolves to a real `(schema, key)`.
/// Adapters use this as their starting `SUPPORTED` list, optionally
/// filtering or extending it. Ordered by the surfaces they render in.
pub const BASELINE_SUPPORTED: &[ShellTweakId] = &[
    // Motion
    ShellTweakId::EnableAnimations,
    // Panel
    ShellTweakId::ShowClock,
    ShellTweakId::ClockFormat,
    ShellTweakId::ShowWeekday,
    ShellTweakId::ShowBattery,
    // Overview
    ShellTweakId::OverviewHotCorner,
    ShellTweakId::OverviewBlur,
    ShellTweakId::FloatingDock,
    // Workspaces
    ShellTweakId::DynamicWorkspaces,
    ShellTweakId::WorkspacesOnAllMonitors,
    ShellTweakId::NumWorkspaces,
    // Window management
    ShellTweakId::AttachModalDialogs,
    ShellTweakId::FocusMode,
    ShellTweakId::TitlebarDoubleClickAction,
    ShellTweakId::ButtonLayout,
    // Fonts & cursor
    ShellTweakId::CursorSize,
    ShellTweakId::FontAntialiasing,
    ShellTweakId::FontHinting,
];

/// Read one tweak through a `(schema, key)` map function. The closure
/// returns `None` for ids the adapter doesn't support.
pub fn read_tweak(
    id: ShellTweakId,
    map: impl Fn(ShellTweakId) -> Option<(GSettingsKey, ValueShape)>,
) -> Result<Option<ShellTweak>, AppError> {
    let Some((gk, shape)) = map(id) else {
        return Ok(None);
    };
    if !schema_present(gk.schema) {
        return Ok(None);
    }
    let settings = gio::Settings::new(gk.schema);
    let value = read_value(&settings, gk.key, shape)?;
    Ok(Some(ShellTweak { id, value }))
}

/// Apply one tweak through a `(schema, key)` map function. Unsupported
/// ids and missing schemas are logged and treated as success.
pub fn apply_tweak(
    tweak: &ShellTweak,
    map: impl Fn(ShellTweakId) -> Option<(GSettingsKey, ValueShape)>,
) -> Result<(), AppError> {
    let Some((gk, shape)) = map(tweak.id) else {
        tracing::debug!("shell tweak: id {:?} unsupported on this version", tweak.id);
        return Ok(());
    };
    if !schema_present(gk.schema) {
        tracing::warn!("shell tweak: schema {} missing — skipped", gk.schema);
        return Ok(());
    }
    let settings = gio::Settings::new(gk.schema);
    write_value(&settings, gk.key, &tweak.value, shape)
}

/// Snapshot every supported tweak, returning the ones whose schema is
/// actually present (so a degraded environment doesn't poison the pack).
pub fn snapshot(
    ids: &[ShellTweakId],
    map: impl Fn(ShellTweakId) -> Option<(GSettingsKey, ValueShape)>,
) -> Result<Vec<ShellTweak>, AppError> {
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(t) = read_tweak(*id, &map)? {
            out.push(t);
        }
    }
    Ok(out)
}

/// Controllers that back extension-only shell effects. Adapters hold
/// these so the same `ShellTweak::apply` call both persists the
/// intent in GSettings *and* drives the live effect through whichever
/// third-party extension provides it.
#[derive(Clone)]
pub struct ExtensionControllers {
    pub floating_dock: Arc<dyn FloatingDockController>,
    pub blur_my_shell: Arc<dyn BlurMyShellController>,
}

/// UUID of our bundled `io.github.gnomex@io.github.gnomex` Shell
/// extension. When this extension is installed and enabled it
/// listens to our GSettings schema directly and drives overview
/// blur + floating dock natively — no third-party extensions needed.
const GNOMEX_EXTENSION_UUID: &str = "io.github.gnomex@io.github.gnomex";

/// Post-hook to run after a successful GSettings write. Dispatches
/// extension-backed tweaks to the appropriate controller and logs
/// rather than errors so an unavailable extension never fails the
/// whole apply chain.
///
/// **Precedence:** when our own `io.github.gnomex@io.github.gnomex`
/// extension is enabled, it handles `OverviewBlur` and `FloatingDock`
/// itself (via a GSettings listener). We skip the BMS/DtD fallbacks
/// in that case to avoid double-applying effects.
pub fn dispatch_extension(tweak: &ShellTweak, ext: &ExtensionControllers) {
    // Our extension is the source of truth when present. It reads the
    // same tb-overview-blur / tb-floating-dock-enabled keys apply_tweak
    // just wrote, so by the time we get here the effect is already on
    // its way through the shell. Nothing else to do.
    if gnomex_extension_enabled() {
        return;
    }

    match (tweak.id, &tweak.value) {
        (ShellTweakId::OverviewBlur, TweakValue::Bool(enabled)) => {
            if ext.blur_my_shell.is_available() {
                if let Err(e) = ext.blur_my_shell.apply(*enabled) {
                    tracing::warn!("blur-my-shell apply failed: {e}");
                }
            }
            // When neither our extension nor BMS is installed, the
            // CSS dim overlay from the theme generator still gives a
            // visible effect on the overview.
        }
        (ShellTweakId::FloatingDock, TweakValue::Bool(enabled)) => {
            if ext.floating_dock.is_available() {
                if let Err(e) = ext.floating_dock.apply(*enabled) {
                    tracing::warn!("floating-dock apply failed: {e}");
                }
            } else {
                tracing::debug!(
                    "floating-dock: neither GNOME X nor Dash to Dock \
                     installed — intent saved, effect will activate when \
                     one of them appears"
                );
            }
        }
        _ => {}
    }
}

/// Read `org.gnome.shell.enabled-extensions` and check for our UUID.
/// Returns false on any GSettings error (better to fall back than to
/// silently skip when we can't read the state).
fn gnomex_extension_enabled() -> bool {
    let has_schema = gio::SettingsSchemaSource::default()
        .and_then(|src| src.lookup("org.gnome.shell", true))
        .is_some();
    if !has_schema {
        return false;
    }
    let s = gio::Settings::new("org.gnome.shell");
    s.strv("enabled-extensions")
        .iter()
        .any(|v| v.as_str() == GNOMEX_EXTENSION_UUID)
}

/// What underlying GVariant shape the adapter expects for this key.
/// Adapters declare it alongside the (schema, key) so the codec stays
/// in one place.
#[derive(Debug, Clone, Copy)]
pub enum ValueShape {
    Bool,
    /// Stored as `bool` upstream, but *inverted* against our domain
    /// value. Used for `WorkspacesOnAllMonitors` which upstream names
    /// `workspaces-only-on-primary` (true means *restricted* to primary).
    InvertedBool,
    /// Reserved for future numeric tweaks (AppsGridColumns, font
    /// scale). Unused at the baseline today; kept in the codec so
    /// adapters can opt in without touching shared code.
    #[allow(dead_code)]
    Int32,
    /// String enum — the `TweakValue::Enum` payload is the variant tag.
    StringEnum,
    /// Boolean encoded as a panel-position enum:
    /// `false` → `Top`, `true` → `Bottom`. (No GNOME schema today
    /// natively types a "panel position" key — the bundled extension
    /// will own this; placeholder for now.)
    #[allow(dead_code)]
    PanelPositionAsBool,
}

fn read_value(
    settings: &gio::Settings,
    key: &str,
    shape: ValueShape,
) -> Result<TweakValue, AppError> {
    let v = settings.value(key);
    Ok(match shape {
        ValueShape::Bool => TweakValue::Bool(v.get::<bool>().unwrap_or(false)),
        ValueShape::InvertedBool => TweakValue::Bool(!v.get::<bool>().unwrap_or(false)),
        ValueShape::Int32 => TweakValue::Int(v.get::<i32>().unwrap_or(0)),
        ValueShape::StringEnum => TweakValue::Enum(
            v.get::<String>().unwrap_or_default(),
        ),
        ValueShape::PanelPositionAsBool => {
            let bottom = v.get::<bool>().unwrap_or(false);
            TweakValue::Position(if bottom {
                PanelPosition::Bottom
            } else {
                PanelPosition::Top
            })
        }
    })
}

fn write_value(
    settings: &gio::Settings,
    key: &str,
    value: &TweakValue,
    shape: ValueShape,
) -> Result<(), AppError> {
    let variant = encode(value, shape).ok_or_else(|| {
        AppError::Settings(format!(
            "shell tweak: value {:?} doesn't match expected shape {:?} for key '{}'",
            value, shape, key
        ))
    })?;
    settings
        .set_value(key, &variant)
        .map_err(|e| AppError::Settings(format!("shell tweak: set '{key}' failed: {e}")))?;
    Ok(())
}

fn encode(value: &TweakValue, shape: ValueShape) -> Option<Variant> {
    match (value, shape) {
        (TweakValue::Bool(b), ValueShape::Bool) => Some(b.to_variant()),
        (TweakValue::Bool(b), ValueShape::InvertedBool) => Some((!b).to_variant()),
        (TweakValue::Int(i), ValueShape::Int32) => Some(i.to_variant()),
        (TweakValue::Enum(s), ValueShape::StringEnum) => Some(s.to_variant()),
        (TweakValue::Position(p), ValueShape::PanelPositionAsBool) => {
            Some(matches!(p, PanelPosition::Bottom).to_variant())
        }
        _ => None,
    }
}

fn schema_present(schema: &str) -> bool {
    gio::SettingsSchemaSource::default()
        .and_then(|src| src.lookup(schema, true))
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_bool_matches_shape() {
        let v = encode(&TweakValue::Bool(true), ValueShape::Bool).unwrap();
        assert_eq!(v.get::<bool>(), Some(true));
    }

    #[test]
    fn encode_inverted_bool_flips() {
        // Domain "WorkspacesOnAllMonitors = true" writes the upstream
        // key (workspaces-only-on-primary) as `false`.
        let v = encode(&TweakValue::Bool(true), ValueShape::InvertedBool).unwrap();
        assert_eq!(v.get::<bool>(), Some(false));
        let v = encode(&TweakValue::Bool(false), ValueShape::InvertedBool).unwrap();
        assert_eq!(v.get::<bool>(), Some(true));
    }

    #[test]
    fn encode_int32_matches_shape() {
        let v = encode(&TweakValue::Int(6), ValueShape::Int32).unwrap();
        assert_eq!(v.get::<i32>(), Some(6));
    }

    #[test]
    fn encode_string_enum_matches_shape() {
        let v = encode(&TweakValue::Enum("24h".into()), ValueShape::StringEnum).unwrap();
        assert_eq!(v.get::<String>().as_deref(), Some("24h"));
    }

    #[test]
    fn encode_rejects_type_mismatch() {
        // Bool value against Int32 shape is a bug, not a silent cast.
        assert!(encode(&TweakValue::Bool(true), ValueShape::Int32).is_none());
        assert!(encode(&TweakValue::Int(5), ValueShape::Bool).is_none());
        assert!(encode(&TweakValue::Enum("x".into()), ValueShape::Bool).is_none());
    }

    #[test]
    fn baseline_resolves_every_supported_id() {
        for id in BASELINE_SUPPORTED {
            assert!(
                baseline_key_for(*id).is_some(),
                "baseline_key_for({id:?}) returned None but id is in BASELINE_SUPPORTED",
            );
        }
    }

    #[test]
    fn baseline_excludes_placeholder_ids() {
        // These tweaks are placeholders for the bundled extension and
        // must remain unresolved at the baseline. Leaking them through
        // would silently half-implement a feature.
        for id in [
            ShellTweakId::TopBarPosition,
            ShellTweakId::ReducedMotion,
            ShellTweakId::WindowAnimationSpeed,
            ShellTweakId::WorkspaceThumbnails,
            ShellTweakId::AppsGridColumns,
        ] {
            assert!(
                baseline_key_for(id).is_none(),
                "baseline_key_for({id:?}) should return None — placeholder for bundled extension",
            );
        }
    }

    #[test]
    fn invertedbool_used_only_where_semantics_differ() {
        // Audit: if we accidentally used InvertedBool on a key whose
        // upstream semantics already match our domain, every user
        // would see the wrong default state. This test is a rubric:
        // grep the baseline for InvertedBool and confirm the keys
        // it applies to.
        let mut inverted_keys = Vec::new();
        for id in BASELINE_SUPPORTED {
            if let Some((gk, ValueShape::InvertedBool)) = baseline_key_for(*id) {
                inverted_keys.push((gk.schema, gk.key));
            }
        }
        assert_eq!(
            inverted_keys,
            vec![("org.gnome.mutter", "workspaces-only-on-primary")],
            "Only workspaces-only-on-primary is known to have inverted semantics \
             versus our domain. If you added another, extend this assertion and \
             document why in baseline_key_for.",
        );
    }
}
