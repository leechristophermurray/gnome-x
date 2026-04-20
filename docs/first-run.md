---
title: First run
description: What to expect the first time you launch GNOME X — the tour, permissions, the tabs.
---

# First run

This page walks you through the first 60 seconds after launching GNOME X.
You'll learn what each tab is for, what the app needs from your system, and
where to look when something doesn't behave the way you expect.

## Launching the app

Find **GNOME X** in your Activities or applications grid and click it. If you
installed from source without `install.sh`, run:

```sh
cargo run -p gnomex-ui
```

![GNOME X — First run](assets/screenshots/first-run-launch.png)

## The four tabs

GNOME X uses an `AdwViewSwitcher` for primary navigation. On a wide window the
switcher sits in the header bar; on a narrow window it slides to a bottom bar
automatically.

### Explore

A discovery surface for everything you can install: GNOME Shell extensions,
GTK themes, Shell themes, icon packs, and cursor themes. The first row is
**popular this week** across sources. Subsequent rows are organised by
category.

Tap any item to open the **detail view** — full description, screenshot, and
either an **Install** or **Apply** button depending on the item type.

### Installed

Lists every GNOME Shell extension installed on your system, with an
enable/disable switch on each row. This is the same data
`gnome-extensions list` would return, but with a clickable detail view
instead of a man-page incantation.

### Customize

The control panel for the look of your desktop:

- **Accent colour** — picks one of the nine GNOME accent palettes
  (blue, teal, green, yellow, orange, red, pink, purple, slate)
- **Light / dark / system** colour scheme
- **GTK theme**, **icon pack**, **cursor theme** dropdowns populated from
  what you have installed locally
- A **Theme Builder** section for fine-grained CSS overrides
  (corner radius, panel opacity, headerbar height, per-widget colours)

### Packs

Lists your saved Experience Packs and exposes the **Snapshot current desktop**
and **Import** actions. See [Build a pack](tutorials/build-a-pack.md) for the
full walk-through.

## What GNOME X needs from your system

| Surface                     | Why                                       | Falls back to                       |
|-----------------------------|-------------------------------------------|-------------------------------------|
| `org.gnome.Shell.Extensions` D-Bus | Enable / disable / install extensions | Disabled if absent (non-GNOME session) |
| `org.gnome.desktop.interface` GSettings | Read / write accent, scheme, theme | Read-only if you lack write access |
| Network access to extensions.gnome.org | Browse and install extensions   | Explore tab shows an empty state    |
| Network access to <www.gnome-look.org>   | Browse themes / icons / cursors | Customize search returns nothing    |
| `~/.local/share/gnome-x/packs/`        | Persist Experience Packs        | Created on first save               |

GNOME X **does not** require root, does **not** install systemd services
without your consent, and does **not** modify any system-wide path under
`/usr/`. Everything happens under your `XDG_DATA_HOME` and `XDG_CONFIG_HOME`.

## Permissions you may be asked for

The first time you click **Install** on an extension, you'll see GNOME's
standard policy dialog asking you to confirm. This is enforced by GNOME Shell
itself — GNOME X cannot bypass it (and would not want to).

If you click **Apply** on an Experience Pack that includes a Shell theme, the
Shell theme is written to `~/.local/share/themes/` and activated through the
`user-theme@gnome-shell-extensions.gcampax.github.com` extension. If that
extension isn't installed, GNOME X will install it before applying. You'll see
a one-line toast confirming the action.

## Sanity check

Open a terminal and run:

```sh
experiencectl status
```

This should print your detected Shell version, the active accent colour, the
current colour scheme, and the current Theme Builder values. If it doesn't,
something in the install is wrong — see
[Verifying the install](install.md#verifying-the-install).

## Where things live on disk

Knowing this is rarely necessary, but useful when you want to back up or wipe:

| Path                                       | What                            |
|--------------------------------------------|---------------------------------|
| `~/.local/share/gnome-x/packs/`            | Saved Experience Packs (TOML + assets) |
| `~/.config/gtk-4.0/gtk.css`                | Theme Builder GTK 4 overrides   |
| `~/.config/gtk-3.0/gtk.css`                | Theme Builder GTK 3 overrides   |
| `~/.local/share/themes/<Name>/gnome-shell/` | Installed Shell themes         |
| `~/.local/share/themes/<Name>/gtk-4.0/`    | Installed GTK 4 themes          |
| `~/.local/share/icons/<Name>/`             | Installed icon packs            |
| `~/.local/share/gnome-shell/extensions/<uuid>/` | Installed extensions       |
| `dconf` path `/io/github/gnomex/GnomeX/`   | App settings & Theme Builder values |

## Where to go next

- [Browse and install an extension](tutorials/extensions.md) — your first
  meaningful action.
- [Snapshot your desktop into an Experience Pack](tutorials/build-a-pack.md) —
  the feature this app exists for.
- [Known limitations](known-limitations.md) — read this before reporting any
  issue with rendering, scaling, GDM, AppImage icons, or Chromium theming.
