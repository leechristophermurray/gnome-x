---
title: Why no Flatpak
description: Why GNOME X is not — and will not be — distributed as a Flatpak. The reasons are structural, not cosmetic.
---

# Why no Flatpak

GNOME X is **not published as a Flatpak** and there are no plans to do so.
This page explains why, in detail, so you can evaluate whether GNOME X is
the right tool for your situation.

The short version: a Flatpak GNOME X would either be **broken** (missing
core features) or **effectively unsandboxed** (`--filesystem=home`, full
session bus, systemd portal). Neither is a useful product.

## What GNOME X actually does

To make sense of the Flatpak rejection, you need to know what surfaces
GNOME X writes to. The honest list:

| What GNOME X writes              | Where it goes                                       |
|----------------------------------|-----------------------------------------------------|
| GTK 4 CSS overrides              | `~/.config/gtk-4.0/gtk.css`                         |
| GTK 3 CSS overrides              | `~/.config/gtk-3.0/gtk.css`                         |
| Shell themes                     | `~/.local/share/themes/<Name>/gnome-shell/`         |
| GTK themes (downloaded)          | `~/.local/share/themes/<Name>/gtk-{3,4}.0/`         |
| Icon packs (downloaded)          | `~/.local/share/icons/<Name>/`                      |
| Cursor themes (downloaded)       | `~/.local/share/icons/<Name>/cursors/`              |
| Extensions (downloaded)          | `~/.local/share/gnome-shell/extensions/<uuid>/`     |
| GSettings reads/writes           | `org.gnome.desktop.interface`, `org.gnome.shell.extensions.*`, etc. |
| Theme-sync systemd user unit     | `~/.config/systemd/user/gnome-x-theme-sync.service` (when scheduled accent is enabled) |
| Shell extension management       | D-Bus calls to `org.gnome.Shell.Extensions`         |

Every one of those touches a host path or a host bus name that the Flatpak
sandbox blocks by default.

## What it would cost to make a Flatpak work

To deliver the same feature set inside a Flatpak, the manifest would need:

```yaml
finish-args:
  - --filesystem=home               # GTK CSS, themes, icons, extensions
  - --filesystem=xdg-config/systemd # theme-sync unit
  - --talk-name=org.gnome.Shell.Extensions
  - --talk-name=org.gnome.Shell
  - --talk-name=org.freedesktop.systemd1
  - --socket=session-bus            # broad bus access
  - --share=network                 # downloads from EGO + gnome-look
```

The only commonly-cited finish-arg *not* on that list is `--device=all`.
At which point: you have a sandbox that can read every file in the user's
home, write themes that affect every other Flatpak'd app, install systemd
user units, and own arbitrary bus names. That is not a sandbox.

The point of Flatpak is to constrain what an app can reach so users can
install software with reduced trust. A "sandbox" that lets the app touch
everything the user can touch is theatre, not security. We'd rather not
ship security theatre.

## The four specific structural blockers

Even if a user accepts the loose finish-args above, four issues remain
where the Flatpak wrapping breaks GNOME X's behaviour in ways the manifest
*can't* fix:

### 1. Theme visibility inside *other* Flatpaks

Flatpak'd applications (Inkscape, Blender, Bottles, etc.) read GTK themes
from their **own sandbox**. When GNOME X writes a theme to
`~/.local/share/themes/`, that path is not visible inside the other
Flatpak unless that other Flatpak has its own override:

```sh
flatpak override --filesystem=~/.local/share/themes <app>
```

We can't issue overrides from inside our own sandbox. So a Flatpak'd
GNOME X would *install* themes, *apply* them to non-Flatpak GTK apps, and
**fail to theme other Flatpak'd apps** — the most-visible "doesn't work"
case for any user with a mix of native and Flatpak'd apps.

### 2. Host systemd reach

GNOME X's optional **scheduled accent** feature installs a systemd user
unit and starts it via the user's systemd. Flatpak's `--talk-name` rules
don't expose host systemd cleanly, and there's no portal for installing
units. We'd have to drop the scheduled-accent feature inside a Flatpak,
which means feature parity with the native build is impossible.

### 3. Shell theme installation

Activating a Shell theme requires writing under
`~/.local/share/themes/<Name>/gnome-shell/` *and* setting
`org.gnome.shell.extensions.user-theme name`. The first half is a
filesystem write (which the loose finish-args allow); the second half is
fine through `talk-name`. But the **User Themes extension itself** must
be installed and enabled, and managing it requires the
`org.gnome.Shell.Extensions` bus name. Adding that name removes the
sandbox's ability to deny extension management — at which point we've
gained nothing over a native install.

### 4. Theme-sync inside other applications

Some apps (notably Chromium-family browsers and VS Code-family editors)
read GNOME X's theme via per-app config files we write to:

- `~/.config/google-chrome/Default/Preferences`
- `~/.config/Code/User/settings.json`

Those paths are inside the user's home and are managed by other apps —
each of which might also be Flatpak'd, with its own sandbox. The
permission story for cross-Flatpak config writes is "don't do that".

## Honest comparison

| Distribution mode             | Works fully | Sandbox is meaningful | Maintenance |
|-------------------------------|-------------|------------------------|-------------|
| `.deb` / `.rpm` from Releases | ✅          | n/a                    | Low (CI handles it) |
| `install.sh` from source      | ✅          | n/a                    | Low          |
| `cargo install`               | ✅ (bin only — no desktop file) | n/a | Low |
| **Flatpak** (hypothetical, loose finish-args) | ⚠️ partial — Other-Flatpak themes, systemd unit, several extensions silently broken | ❌ no  | High — every Flathub release cycle plus per-feature sandbox debugging |

There's no row where Flatpak comes out ahead.

## What you should use instead

- **For most users:** install from the
  [Releases page][releases] using the `.deb` or `.rpm` for your distro.
- **For developers and power users:** clone the repo and run
  `sudo ./install.sh` (see [Install](../install.md#from-source)).
- **For CI / scripts:** `cargo install --path crates/ui` then add desktop
  integration files manually.

[releases]: https://github.com/leechristophermurray/gnome-x/releases

## What if I really want a Flatpak

You can write one — the source is Apache-2.0 and we won't object. But
we won't accept a Flatpak manifest into the upstream repo for the reasons
above. You'd be maintaining it on your own, and your users would hit the
"works on the docs but not for me" failures every time the sandbox blocks
something that the docs assume works.

If you build one anyway and want to share it, please make the limitations
**very loud** in your README so users know what to expect.

## Where to go next

- [Install](../install.md) — the supported install paths.
- [Architecture](architecture.md) — why the port/adapter design *enables*
  the host-write features that make Flatpak hostile.
- [Known limitations](../known-limitations.md) — the full honest list of
  things GNOME X can't fix.
