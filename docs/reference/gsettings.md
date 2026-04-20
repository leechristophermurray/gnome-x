---
title: GSettings keys reference
description: Every key in the io.github.gnomex.GnomeX schema with its type, default, and what it does.
---

# GSettings keys reference

GNOME X stores its configuration in the `io.github.gnomex.GnomeX` GSettings
schema. All keys are user-writable; you can change them through the GNOME X
UI, with `gsettings`, with `dconf-editor`, or by editing
`~/.config/dconf/user`. This page documents every key, its type, default,
and effect.

For the canonical schema source, see
[`data/io.github.gnomex.GnomeX.gschema.xml`][src].

[src]: https://github.com/leechristophermurray/gnome-x/blob/main/data/io.github.gnomex.GnomeX.gschema.xml

## Reading and writing keys from a terminal

```sh
# Read a key
gsettings get io.github.gnomex.GnomeX tb-window-radius

# Write a key
gsettings set io.github.gnomex.GnomeX tb-window-radius 14.0

# Reset to default
gsettings reset io.github.gnomex.GnomeX tb-window-radius

# Watch a key change in realtime
gsettings monitor io.github.gnomex.GnomeX tb-panel-opacity
```

GSettings type names: `b` boolean, `i` int32, `d` double, `s` string,
`as` array-of-strings.

## Window state

| Key                | Type | Default | Description                              |
|--------------------|------|---------|------------------------------------------|
| `window-width`     | `i`  | 900     | Restored window width on next launch.    |
| `window-height`    | `i`  | 640     | Restored window height on next launch.   |
| `window-maximized` | `b`  | false   | Restore maximized on next launch.        |

## Behaviour toggles

| Key                          | Type | Default | Description                                                                                       |
|------------------------------|------|---------|---------------------------------------------------------------------------------------------------|
| `disable-version-validation` | `b`  | false   | If true, allow installing extensions not marked compatible with the running Shell version. Surfaced in **Settings** tab. |
| `use-wallpaper-accent`       | `b`  | false   | If true, the experienced daemon re-extracts an accent colour every time the wallpaper changes.    |
| `tb-floating-dock-enabled`   | `b`  | false   | Turn the overview dash into an always-visible floating dock. Requires Dash to Dock to be installed. |

## Scheduled accent

The full accent-scheduling subsystem.

| Key                        | Type | Default | Description                                                              |
|----------------------------|------|---------|--------------------------------------------------------------------------|
| `scheduled-accent-enabled` | `b`  | false   | Master switch for the day/night accent scheduler.                        |
| `day-accent-color`         | `s`  | `blue`  | Accent applied during daytime. One of GNOME's nine accent ids.           |
| `night-accent-color`       | `s`  | `slate` | Accent applied during nighttime.                                         |
| `use-sunrise-sunset`       | `b`  | false   | If true, schedule follows GNOME's night-light sunrise/sunset times. If false, fixed schedule from the night-light schedule keys. |
| `shared-tinting-enabled`   | `b`  | false   | If true, the same accent tint is applied to the Shell panel and dash.    |

The scheduler reads the following GNOME-owned keys for timing:

```sh
gsettings get org.gnome.settings-daemon.plugins.color night-light-schedule-from
gsettings get org.gnome.settings-daemon.plugins.color night-light-schedule-to
gsettings get org.gnome.settings-daemon.plugins.color night-light-schedule-automatic
```

## Scheduled panel tint

| Key                       | Type | Default     | Description                                            |
|---------------------------|------|-------------|--------------------------------------------------------|
| `scheduled-panel-enabled` | `b`  | false       | Master switch for day/night panel tinting.             |
| `day-panel-tint`          | `s`  | `#1a1a1e`   | Hex colour for daytime panel tint.                     |
| `night-panel-tint`        | `s`  | `#1a1a1e`   | Hex colour for nighttime panel tint.                   |

## Theme Builder — radii and shadows

The `tb-*` keys collectively are GNOME X's "Theme Builder" surface.
Changing any one regenerates `~/.config/gtk-{3,4}.0/gtk.css` and emits the
matching Shell CSS.

| Key                       | Type | Default | Description                                                              |
|---------------------------|------|---------|--------------------------------------------------------------------------|
| `tb-window-radius`        | `d`  | 12.0    | Window corner radius in px.                                              |
| `tb-element-radius`       | `d`  | 6.0     | Buttons, entries, cards corner radius in px.                             |
| `tb-panel-radius`         | `d`  | 0.0     | Top-bar panel corner radius in px (0 = full-width).                      |
| `tb-headerbar-height`     | `d`  | 47.0    | Min headerbar height in px.                                              |
| `tb-headerbar-shadow`     | `d`  | 1.0     | Headerbar drop-shadow intensity (0 = none).                              |
| `tb-circular-buttons`     | `b`  | false   | If true, render window control buttons (close/min/max) as circles.       |
| `tb-show-window-shadow`   | `b`  | true    | If true, draw the standard CSD window drop shadow.                       |
| `tb-inset-border`         | `d`  | 0.0     | Inset border drawn just inside the window outline (px).                  |
| `tb-card-border-width`    | `d`  | 1.0     | Border width on `.card`-styled containers (px).                          |
| `tb-separator-opacity`    | `d`  | 1.0     | Opacity of separator lines (0–1).                                        |
| `tb-focus-ring-width`     | `d`  | 2.0     | Focus ring width on keyboard-focused widgets (px).                       |
| `tb-combo-inset`          | `b`  | true    | If true, add an inset border to combo buttons.                           |

## Theme Builder — panel and overview

| Key                  | Type | Default     | Description                                                  |
|----------------------|------|-------------|--------------------------------------------------------------|
| `tb-panel-opacity`   | `d`  | 80.0        | Panel opacity in **percent** (0–100).                        |
| `tb-panel-tint`      | `s`  | `#1a1a1e`   | Panel tint hex colour.                                       |
| `tb-tint-intensity`  | `d`  | 5.0         | Strength of the accent tint applied across surfaces (%).     |
| `tb-dash-opacity`    | `d`  | 70.0        | Overview dash opacity in **percent** (0–100).                |
| `tb-overview-blur`   | `b`  | true        | If true, apply blur to the overview backdrop.                |

## Theme Builder — widget style

The widget-style bundle introduced before per-widget colours. Each value
is a fraction `0.0`–`1.0`.

| Key                              | Type | Default | Description                                                  |
|----------------------------------|------|---------|--------------------------------------------------------------|
| `tb-widget-input-inset`          | `d`  | 0.0     | 0 = Adwaita-flat inputs. Higher = visible background + border. |
| `tb-widget-button-raise`         | `d`  | 0.0     | 0 = Adwaita-flat buttons. Higher = border + shadow on non-flat, non-suggested buttons. |
| `tb-widget-headerbar-gradient`   | `d`  | 0.0     | 0 = flat headerbar. Higher = subtle top-to-bottom gradient. Conflicts with Adwaita's flat philosophy — use sparingly. |

## Theme Builder — per-widget, per-scheme colour overrides

Each slot has a light and dark counterpart. Empty string = no override
(use Adwaita / accent-tint default). See
[the per-widget colours tutorial](../tutorials/widget-colors.md).

| Key                              | Type | Default | Description                              |
|----------------------------------|------|---------|------------------------------------------|
| `tb-color-button-bg-light`       | `s`  | `""`    | `@button_bg_color` override for light scheme. |
| `tb-color-button-bg-dark`        | `s`  | `""`    | `@button_bg_color` override for dark scheme. |
| `tb-color-entry-bg-light`        | `s`  | `""`    | `@entry_bg_color` override for light scheme. |
| `tb-color-entry-bg-dark`         | `s`  | `""`    | `@entry_bg_color` override for dark scheme. |
| `tb-color-headerbar-bg-light`    | `s`  | `""`    | `@headerbar_bg_color` override for light scheme. |
| `tb-color-headerbar-bg-dark`     | `s`  | `""`    | `@headerbar_bg_color` override for dark scheme. |
| `tb-color-sidebar-bg-light`      | `s`  | `""`    | `@sidebar_bg_color` override for light scheme. |
| `tb-color-sidebar-bg-dark`       | `s`  | `""`    | `@sidebar_bg_color` override for dark scheme. |

## Theme Builder — layer separation

| Key                            | Type | Default | Description                                                  |
|--------------------------------|------|---------|--------------------------------------------------------------|
| `tb-layer-headerbar-bottom`    | `d`  | 0.0     | Bottom-border width (px) under the headerbar in `@borders`.  |
| `tb-layer-sidebar-divider`     | `d`  | 0.0     | Vertical divider width (px) between sidebar and content.     |
| `tb-layer-content-contrast`    | `d`  | 0.0     | Extra contrast on the content backdrop (0–1).                |

## Theme Builder — sidebar

| Key                       | Type | Default | Description                                                  |
|---------------------------|------|---------|--------------------------------------------------------------|
| `tb-sidebar-opacity`      | `d`  | 1.0     | Sidebar background opacity (0–1). Below 1.0 lets compositor blur bleed through. |
| `tb-sidebar-fg-override`  | `s`  | `""`    | Optional `#rrggbb` override for sidebar text. Empty = Adwaita default. |

## Theme Builder — notifications

| Key                        | Type | Default | Description                                |
|----------------------------|------|---------|--------------------------------------------|
| `tb-notification-radius`   | `d`  | 12.0    | Corner radius (px) of notification banners. |
| `tb-notification-opacity`  | `d`  | 0.95    | Opacity (0–1) of notification banners.      |

## Daemon-managed

| Key                          | Type | Default | Description                                                                                       |
|------------------------------|------|---------|---------------------------------------------------------------------------------------------------|
| `cached-wallpaper-palette`   | `as` | `[]`    | Each entry is `"HEX:ACCENT_ID"`. Rewritten by the experienced daemon when the wallpaper changes so the UI shows last-known swatches without re-running extraction. |

## Resetting everything

To reset GNOME X to ship defaults (preserves saved Experience Packs):

```sh
gsettings reset-recursively io.github.gnomex.GnomeX
```

To also wipe the GTK CSS overrides:

```sh
rm ~/.config/gtk-4.0/gtk.css ~/.config/gtk-3.0/gtk.css
```

To wipe saved Experience Packs:

```sh
rm -rf ~/.local/share/gnome-x/packs/
```

To wipe **everything** GNOME X touches under your home directory:

```sh
gsettings reset-recursively io.github.gnomex.GnomeX
rm -f ~/.config/gtk-4.0/gtk.css ~/.config/gtk-3.0/gtk.css
rm -rf ~/.local/share/gnome-x/
rm -f ~/.config/systemd/user/gnome-x-theme-sync.service
systemctl --user daemon-reload
```

This does **not** uninstall themes, icons, cursors, or extensions GNOME X
downloaded — those live in standard XDG paths and are usable by the rest
of GNOME. To remove a specific downloaded theme:

```sh
rm -rf ~/.local/share/themes/<Name>
```

## Where to go next

- [experiencectl CLI](cli.md) — most of these keys have a CLI shortcut.
- [Pack TOML reference](pack-toml.md) — for `[[settings]]` blocks that
  write to *other* schemas.
- [Theme Builder tutorial — per-widget colours](../tutorials/widget-colors.md)
  — the high-level guide to the eight `tb-color-*` keys.
