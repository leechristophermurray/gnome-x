---
title: experiencectl CLI reference
description: Every command, option, and exit code of the experiencectl command-line tool.
---

# `experiencectl` CLI reference

`experiencectl` is GNOME X's command-line interface. It uses the **same use
cases** as the GTK app, so anything you can do through the UI can be
scripted through the CLI. This is the complete reference.

For the CLI source, see
[`crates/cli/src/main.rs`][src].

[src]: https://github.com/leechristophermurray/gnome-x/blob/main/crates/cli/src/main.rs

## Synopsis

```text
experiencectl [OPTIONS] <COMMAND>
```

## Global options

```text
-h, --help          Print help (also works on each subcommand)
-V, --version       Print version
```

Logging is controlled by the `RUST_LOG` environment variable using
[tracing-subscriber's `EnvFilter` syntax][envfilter]. Default:
`experiencectl=info`.

[envfilter]: https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html

```sh
# Verbose
RUST_LOG=experiencectl=debug experiencectl status

# Trace one specific module
RUST_LOG=gnomex_infra::dbus_shell_proxy=trace experiencectl status
```

## Commands

### `accent` — get / set the system accent colour

```text
experiencectl accent <SUBCOMMAND>

  set <COLOR>    Set the accent color (one of: blue, teal, green, yellow,
                 orange, red, pink, purple, slate)
  get            Print the current accent color
```

Writes to `org.gnome.desktop.interface accent-color`. Effective
immediately for any GTK 4 / Libadwaita app that watches the key (which
is most of them).

```sh
$ experiencectl accent get
blue

$ experiencectl accent set teal
Accent color set to: teal
```

??? note "Accent name → hex mapping"
    The CLI displays names; internally each maps to a fixed hex used by
    the Theme Builder when rendering CSS:

    | Name    | Hex       |
    |---------|-----------|
    | blue    | `#3584e4` |
    | teal    | `#2190a4` |
    | green   | `#3a944a` |
    | yellow  | `#c88800` |
    | orange  | `#ed5b00` |
    | red     | `#e62d42` |
    | pink    | `#d56199` |
    | purple  | `#9141ac` |
    | slate   | `#6f8396` |

    Source: `accent_name_to_hex()` in `crates/cli/src/main.rs`.

### `theme` — apply or reset Theme Builder values

```text
experiencectl theme <SUBCOMMAND>

  apply          Apply current theme builder settings (writes GTK + Shell CSS)
  reset          Reset all custom theme overrides
```

`apply` reads every `tb-*` key from `io.github.gnomex.GnomeX`, builds a
`ThemeSpec`, and rewrites:

- `~/.config/gtk-4.0/gtk.css`
- `~/.config/gtk-3.0/gtk.css`
- The Shell-side stylesheet (under `~/.local/share/themes/`)
- VS Code settings (`~/.config/Code/User/settings.json`) for the colour
  block we manage
- Chromium-family preference files for the theme block we manage

`reset` removes the GTK CSS overrides and clears Shell-side overrides.
GSettings values are **not** reset — to do that, use `gsettings
reset-recursively io.github.gnomex.GnomeX`. See the
[GSettings reset section](gsettings.md#resetting-everything).

```sh
$ experiencectl theme apply
Theme applied (47.2) — restart apps to see changes

$ experiencectl theme reset
Theme overrides removed
```

### `pack` — manage Experience Packs

```text
experiencectl pack <SUBCOMMAND>

  list                  List saved Experience Packs
  apply <ID>            Apply an Experience Pack by id
```

`list` reads `~/.local/share/gnome-x/packs/` and prints a one-line
summary per pack:

```sh
$ experiencectl pack list
nordic-elegance  —  Nordic Elegance  (by Ada Lovelace)
oled-minimal     —  OLED Minimal     (by you)
```

`apply` runs the same flow as the UI's **Apply** button (download missing
content, install missing extensions, write GSettings, regenerate Theme
Builder CSS):

```sh
$ experiencectl pack apply nordic-elegance
Pack 'nordic-elegance' applied (theme re-rendered).
```

Network operations (downloads, extension installs) run synchronously and
print errors to stderr. The pack manifest is loaded from
`~/.local/share/gnome-x/packs/<id>/pack.toml`.

### `status` — show current state and detected environment

```text
experiencectl status
```

Prints the running Shell version, current accent / colour scheme, and the
core Theme Builder values. Useful for debugging "why doesn't this look
right" reports.

```sh
$ experiencectl status
=== GNOME X Status ===
Shell version:         47.2
Accent color:          blue
Color scheme:          default
Scheduled accent:      ON
  Day accent:          blue
  Night accent:        slate
  Schedule:            18:00 → 6:00 (sunrise/sunset)
Shared tinting:        true
Window radius:         12px
Element radius:        9px
Panel opacity:         85%
Tint intensity:        12%
```

If `Shell version:` is missing, GNOME X could not reach the GNOME Shell
D-Bus interface — usually because the session isn't a GNOME Shell session.

## Exit codes

| Code | Meaning                                            |
|------|----------------------------------------------------|
| 0    | Success                                            |
| 1    | Error — see stderr for the message and `RUST_LOG=debug` for context |
| 2    | Argument parsing failed (clap error)               |

## Scripting examples

### Apply a pack on login (as part of `gsettings`-style setup)

```sh
#!/bin/sh
# ~/.config/autostart/apply-pack.sh — apply a pack at session start
experiencectl pack apply nordic-elegance >/tmp/apply-pack.log 2>&1
```

### Cycle accent colours for fun

```sh
#!/bin/sh
for c in blue teal green orange red pink purple; do
    experiencectl accent set "$c"
    sleep 5
done
experiencectl accent set blue
```

### Re-apply theme after editing GSettings directly

```sh
gsettings set io.github.gnomex.GnomeX tb-window-radius 16.0
gsettings set io.github.gnomex.GnomeX tb-element-radius 8.0
experiencectl theme apply
```

(The UI does this automatically on every slider change. The CLI flow is
the same path.)

## Where to go next

- [GSettings reference](gsettings.md) — for direct key access without
  going through `experiencectl`.
- [Pack TOML reference](pack-toml.md) — for hand-writing packs to
  `experiencectl pack apply`.
- [Apply a pack tutorial](../tutorials/apply-a-pack.md) — UI-side flow
  the CLI mirrors.
