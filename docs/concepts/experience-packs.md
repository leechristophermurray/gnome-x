---
title: Experience Packs
description: What Experience Packs are, what's inside one, what they replace, and what they don't try to do.
---

# Experience Packs

An **Experience Pack** is a single file that describes a complete GNOME
desktop look — theme, icons, cursor, wallpaper, enabled extensions, fine
appearance values — in a way that's portable, version-controllable, and
applied with one click. This page explains the model behind them.

For the file format, see [Pack TOML reference](../reference/pack-toml.md).
For walk-throughs, see
[Build a pack](../tutorials/build-a-pack.md) and
[Apply a pack](../tutorials/apply-a-pack.md).

## The problem they solve

Recreating a GNOME setup from scratch is brittle:

- A theme on gnome-look.org has a name, a content ID, a *file ID* per
  download, and a version. Sharing "I use Nordic" doesn't pin any of those.
- Extensions on extensions.gnome.org are matched by UUID, but compatibility
  by Shell version. Sharing "install Blur My Shell" doesn't pin which
  version.
- GTK CSS overrides in `~/.config/gtk-4.0/gtk.css` are an opaque blob most
  users won't share.
- The seven or eight Theme-Builder values (radius, panel opacity, headerbar
  height, etc.) live in GSettings, which doesn't export cleanly.

A pack pins **all of the above** by reference (content + file IDs, UUIDs,
GSettings key/value pairs) into one file that another machine can replay
deterministically.

## What's in a pack

A pack is a `.tar.gz` archive containing a TOML manifest and an optional
screenshot:

```
<id>.gnomex-pack.tar.gz
├── pack.toml          # the manifest — see Pack TOML reference
└── screenshot.webp    # optional cover image
```

The manifest sections, at a glance:

| Section          | Captures                                                |
|------------------|---------------------------------------------------------|
| `[pack]`         | ID, name, description, author, target Shell version, wallpaper path, `pack_format` version, optional screenshot |
| `[theme.gtk]`    | GTK theme name + gnome-look.org content/file IDs        |
| `[theme.shell]`  | Shell theme name + gnome-look.org content/file IDs      |
| `[icons]`        | Icon pack name + gnome-look.org content/file IDs        |
| `[cursor]`       | Cursor theme name + gnome-look.org content/file IDs     |
| `[[extensions]]` | One block per extension — UUID, display name, `required` flag |
| `[[settings]]`   | Arbitrary GSettings overrides — schema-qualified key + value (string) |

The TOML schema is enforced by serde DTOs in
`crates/infra/src/pack_toml_storage.rs`, validated against the
`pack_format` field on import.

## What packs reference, not bundle

Packs are designed to stay small and stable, so they reference downloadable
content rather than bundling it:

- **Themes / icons / cursors** are referenced by gnome-look.org content
  + file ID. On `Apply`, GNOME X downloads the file at apply-time using
  the same path as a manual install.
- **Extensions** are referenced by UUID. On `Apply`, GNOME X installs each
  one through the GNOME Shell D-Bus interface, which fetches from
  extensions.gnome.org.
- **Wallpapers** are referenced by absolute path (`file:///...` or
  `https://...`). The image itself is not bundled.

This means a pack file is typically **under 100 KB** including the
screenshot. It also means a pack is **only as reproducible as the upstream
URLs**: if a gnome-look.org maintainer deletes a theme or bumps its file ID
without updating the content ID, the pack will fail to apply until you
re-snapshot or hand-edit the IDs.

We accept that tradeoff because:

- Bundling theme tarballs would push pack files into multi-megabyte
  territory — at which point sharing them via Slack / email / a wiki gets
  awkward.
- Re-distributing third-party themes inside our archives raises licensing
  questions we'd rather not invent answers to.
- The breakage mode (a single dead URL) is **easy to diagnose and fix**
  through the UI's apply-progress toast.

## What packs *don't* try to do

| Not in scope                                         | Why                                  |
|------------------------------------------------------|--------------------------------------|
| **Per-extension settings**                           | Extensions store settings under their own GSettings paths, in formats they own. We can't reliably round-trip every extension's config. Use the extension's own export feature if it has one. |
| **Application-specific themes** (Firefox, Code, …)   | Each app has its own theming system. Some are covered by the [external app themers](../known-limitations.md#browser--electron-theme-fidelity), but those are separate features, not pack contents. |
| **Fonts**                                            | We don't bundle, install, or activate font files. The font name in a `[[settings]]` block (`org.gnome.desktop.interface font-name`) is preserved, but the font itself must already be installed on the target machine. |
| **Keyboard shortcuts**                               | Not captured. Out of scope — would balloon the manifest with content most users don't want pinned. |
| **GDM (login screen) theming**                       | Lives outside the user session. See [Known limitations](../known-limitations.md#gdm-login-screen-theming). |

If a pack-able item isn't covered here and you think it should be, open an
issue on the [tracker][issues].

[issues]: https://github.com/leechristophermurray/gnome-x/issues

## Pack format versioning

The `pack.pack_format` integer pins the schema. The current version is
**`1`**. When the schema changes:

- The version number is bumped (`pack_format = 2`, etc.)
- A migration function in `pack_file_to_domain()` (see
  `crates/infra/src/pack_toml_storage.rs`) handles the previous version
- Older packs continue to import — they get migrated in-memory at load time
- The README example in the [Pack TOML reference](../reference/pack-toml.md)
  is updated

Packs **never** silently downgrade. If a pack uses a `pack_format` newer
than the running GNOME X knows, it refuses to import with an explicit
"please update GNOME X" message.

## Pack lifecycle on disk

Imported and snapshotted packs live at:

```
~/.local/share/gnome-x/packs/
└── <id>/
    ├── pack.toml
    └── screenshot.webp     (optional)
```

The Packs tab scans this directory at open time and builds a row per
subdirectory. You can hand-edit `pack.toml` and the changes appear next
time the tab opens. You can copy a `<id>/` directory between machines as
an alternative to exporting + importing.

`Delete` removes the `<id>/` directory entirely.

## Why TOML and not JSON / YAML

TOML reads cleanly when hand-edited (which we expect users to do), supports
arrays of tables (which we use heavily for `[[extensions]]` and
`[[settings]]`), and has a single, unambiguous spec. JSON requires too much
quoting noise for hand editing. YAML's whitespace sensitivity is too easy
to get wrong when copy-pasting fragments.

The cost is that TOML lacks anchors / references — but packs are flat
enough that this hasn't mattered.

## Where to go next

- [Pack TOML reference](../reference/pack-toml.md) — the full schema with
  every field's type and default.
- [Build a pack](../tutorials/build-a-pack.md) — practical walk-through.
- [Architecture](architecture.md) — where pack storage fits in the
  port/adapter model.
