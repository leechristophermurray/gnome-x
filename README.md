<p align="center">
  <img src="data/icons/128x128/apps/io.github.gnomex.GnomeX.png" width="128" height="128" alt="GNOME X logo">
</p>

<h1 align="center">GNOME X</h1>

<p align="center">
  <strong>Unified GNOME customization hub</strong>
</p>

<p align="center">
  <a href="https://github.com/gnomex/gnome-x/actions"><img src="https://img.shields.io/badge/build-passing-brightgreen" alt="Build"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue" alt="License"></a>
  <a href="https://github.com/gnomex/gnome-x/releases"><img src="https://img.shields.io/badge/version-0.2.0-orange" alt="Version"></a>
  <a href="https://gnome.pages.gitlab.gnome.org/libadwaita/"><img src="https://img.shields.io/badge/toolkit-GTK4%20%2B%20Libadwaita-4a86cf" alt="GTK4 + Libadwaita"></a>
</p>

---

GNOME X brings extension management, theme installation, icon pack management,
and **Experience Packs** into a single GTK4/Libadwaita application.

Experience Packs let you snapshot a complete desktop configuration --- theme,
icons, cursor, extensions, and settings --- into a shareable TOML manifest,
turning 30-minute tutorials into 1-click installs.

## Features

- **Explore** --- Search and browse GNOME Shell extensions, GTK themes, Shell
  themes, icon packs, and cursor themes from extensions.gnome.org and
  gnome-look.org. The landing page surfaces popular and recently updated items
  across sources.

- **Detail View** --- Tap any listing to see a full-screen detail page with
  screenshot, description, and install/apply actions.

- **Installed** --- View all extensions on the system with enable/disable
  switches. Tap a row for the detail view.

- **Customize** --- Install themes, icons, and cursors, then apply them
  instantly via GSettings.

- **Experience Packs** --- Snapshot your current desktop (GTK theme, Shell
  theme, icon pack, cursor, wallpaper, enabled extensions) into a portable
  `.gnomex-pack.tar.gz` archive. Import packs others have shared to reproduce
  their setup in one click. Screenshots can be bundled with exports.

- **Settings** --- Disable version validation to allow installing extensions
  not marked compatible with the running Shell version.

- **Native Look & Feel** --- Follows the [GNOME Human Interface Guidelines][hig].
  Libadwaita adaptive layout, `AdwViewSwitcher` navigation, `AdwToastOverlay`
  notifications, and full dark/light/accent colour scheme support.

[hig]: https://developer.gnome.org/hig/

## Screenshots

> *Screenshots will be added after the first public release.*

## Architecture

GNOME X uses a layered hexagonal architecture ("Tree Architecture") with strict
dependency boundaries enforced at the crate level:

```
crates/
  domain/   L0  Pure domain types, value objects, entities (zero deps)
  app/      L1  Use cases and port (trait) definitions
  infra/    L3  HTTP clients, D-Bus proxy, GSettings, filesystem
  ui/       L2  GTK4 / Libadwaita / Relm4 application
```

| Port (trait)          | Adapter (impl)           | Purpose                            |
|-----------------------|--------------------------|------------------------------------|
| `ExtensionRepository` | `EgoClient`              | extensions.gnome.org HTTP API      |
| `ContentRepository`   | `OcsClient`              | gnome-look.org OCS API             |
| `ShellProxy`          | `DbusShellProxy`         | GNOME Shell D-Bus interface        |
| `LocalInstaller`      | `FilesystemInstaller`    | Install/uninstall to XDG dirs      |
| `AppearanceSettings`  | `GSettingsAppearance`    | Read/write desktop theme settings  |
| `PackStorage`         | `PackTomlStorage`        | Persist Experience Packs as TOML   |

## Requirements

### Runtime

- GNOME Shell 45 or later (for D-Bus extension management)
- GTK 4.14+
- Libadwaita 1.5+

### Build

- Rust 2024 edition (stable toolchain)
- GTK 4 development headers
- Libadwaita development headers
- `pkg-config`

#### Fedora / RHEL

```sh
sudo dnf install gtk4-devel libadwaita-devel gcc pkg-config
```

#### Ubuntu / Debian

```sh
sudo apt install libgtk-4-dev libadwaita-1-dev build-essential pkg-config
```

#### Arch Linux

```sh
sudo pacman -S gtk4 libadwaita rust pkgconf
```

## Building

```sh
git clone https://github.com/gnomex/gnome-x.git
cd gnome-x
cargo build --release -p gnomex-ui
```

The binary is produced at `target/release/gnomex-ui`.

### Running (development)

```sh
cargo run -p gnomex-ui
```

The application registers a custom icon theme search path at startup so the app
icon loads without installation.

## Installing

### System install

```sh
cargo build --release -p gnomex-ui
sudo ./install.sh
```

This installs the binary to `/usr/local/bin/gnome-x`, the desktop file, AppStream
metadata, GSettings schema, and hicolor icons. Override the prefix with
`PREFIX=/usr sudo ./install.sh`.

### Cargo install

```sh
cargo install --path crates/ui
```

Note: this installs the binary only. Desktop integration files (icon, `.desktop`,
schema) must be placed manually or via `install.sh`.

### Flatpak

A Flatpak manifest is provided at `build-aux/flatpak/io.github.gnomex.GnomeX.json`.
To build locally with `flatpak-builder`:

```sh
flatpak-builder --user --install --force-clean \
    build-dir build-aux/flatpak/io.github.gnomex.GnomeX.json
```

## Experience Packs

An Experience Pack is a TOML manifest that captures a complete desktop
configuration:

```toml
[pack]
id = "nordic-elegance"
name = "Nordic Elegance"
description = "Cool-toned minimal desktop"
author = "Ada"
shell_version = "47.0"
pack_format = 1

[theme.gtk]
name = "Nordic"
source = "gnome-look.org"
content_id = 1267246
file_id = 1

[icons]
name = "Papirus-Dark"
source = "gnome-look.org"
content_id = 1166289
file_id = 1

[[extensions]]
uuid = "blur-my-shell@aunetx"
name = "Blur my Shell"
required = true
```

### Export

Click the save icon on any pack row to export a `.gnomex-pack.tar.gz` archive
containing the manifest and an optional screenshot.

### Import

Click **Import** in the Packs toolbar to load a `.gnomex-pack.tar.gz` and add
it to your collection.

### Apply

Click **Apply** on any pack to download missing content, install it, set your
themes/icons/cursor via GSettings, and enable listed extensions --- all in one
step.

## Project Structure

```
gnome-x/
  Cargo.toml                        Workspace root
  crates/
    domain/                          Domain types and business rules
    app/                             Use cases and port traits
    infra/                           Infrastructure adapters
    ui/                              GTK4/Relm4 application binary
  data/
    io.github.gnomex.GnomeX.desktop.in
    io.github.gnomex.GnomeX.gschema.xml
    io.github.gnomex.GnomeX.metainfo.xml.in
    icons/                           Hicolor icon theme
    resources/                       GResource bundle (CSS)
  build-aux/
    flatpak/                         Flatpak build manifest
  install.sh                         System install script
```

## Known limitations

Some rendering concerns live below GNOME X's theming and GSettings
surface (HiDPI/fractional-scaling blurriness, Flatpak sandbox
visibility, browser theming fidelity, GDM login-screen theming). The
[Known limitations](docs/known-limitations.md) document explains the
causes, mitigations, and which tracker items track further work.

## Contributing

Contributions are welcome. See [CONTRIBUTING.md](CONTRIBUTING.md) for the full
guide covering architecture rules, code style, GNOME HIG compliance, commit
conventions, and the review checklist.

## License

GNOME X is licensed under the [Apache License 2.0](LICENSE).

```
Copyright 2026 GNOME X Contributors
SPDX-License-Identifier: Apache-2.0
```

## Acknowledgements

GNOME X is built with:

- [GTK 4](https://gtk.org/) and [Libadwaita](https://gnome.pages.gitlab.gnome.org/libadwaita/) --- the GNOME UI toolkit
- [Relm4](https://relm4.org/) --- Elm-inspired Rust framework for GTK4
- [extensions.gnome.org](https://extensions.gnome.org/) --- GNOME Shell extension repository
- [gnome-look.org](https://www.gnome-look.org/) --- Community themes, icons, and cursors
- [zbus](https://github.com/dbus2/zbus) --- Pure Rust D-Bus implementation
