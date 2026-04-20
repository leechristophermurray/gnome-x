---
title: Pack TOML reference
description: Complete schema for the .gnomex-pack.tar.gz manifest file (pack.toml). Every section, every field, every type, every default.
---

# Pack TOML reference

Every `.gnomex-pack.tar.gz` archive contains exactly one `pack.toml`
manifest. This page is the complete schema reference. For the conceptual
overview, see [Experience Packs](../concepts/experience-packs.md).

The schema is defined in
[`crates/infra/src/pack_toml_storage.rs`][src] (the serde DTOs) and
validated on every import. Unrecognised fields are silently ignored.

[src]: https://github.com/leechristophermurray/gnome-x/blob/main/crates/infra/src/pack_toml_storage.rs

## Top-level structure

```toml
[pack]                  # required — manifest metadata
[theme]                 # optional — wraps theme.gtk and theme.shell
[theme.gtk]             # optional — GTK theme content reference
[theme.shell]           # optional — Shell theme content reference
[icons]                 # optional — icon pack content reference
[cursor]                # optional — cursor theme content reference
[[extensions]]          # optional, repeatable — one block per extension
[[settings]]            # optional, repeatable — one block per arbitrary GSetting
[[shell_tweaks]]        # optional, repeatable — pack_format ≥ 2 only
```

`[pack]` is the only required table. A pack with just `[pack]` and nothing
else is valid (it just won't *do* anything when applied).

## `[pack]` — manifest metadata

```toml
[pack]
id = "nordic-elegance"
name = "Nordic Elegance"
description = "A clean, dark Nordic-inspired desktop with blur effects"
author = "Ada Lovelace"
created = "2026-04-13T12:00:00Z"
shell_version = "47"
pack_format = 1
wallpaper = "file:///usr/share/backgrounds/gnome/adwaita-d.webp"  # optional
```

| Field           | Type   | Required | Description                                                              |
|-----------------|--------|----------|--------------------------------------------------------------------------|
| `id`            | string | yes      | Kebab-case slug. Used as filename and pack key. Must be unique on the target machine. |
| `name`          | string | yes      | Display name. Free-form.                                                 |
| `description`   | string | yes      | One-or-two-sentence description.                                         |
| `author`        | string | yes      | Author name or handle.                                                   |
| `created`       | string | yes      | ISO 8601 UTC timestamp of when the pack was snapshotted.                 |
| `shell_version` | string | yes      | Major (or major.minor) version of GNOME Shell the pack was snapshotted on. Used for compatibility warnings on apply. |
| `pack_format`   | int    | yes      | Schema version. Currently `1`. Bumped when the schema changes; older packs are migrated in-memory. |
| `wallpaper`     | string | no       | Absolute `file://` or `https://` URL of the wallpaper. **Not bundled** — see [Concepts → Experience Packs](../concepts/experience-packs.md#what-packs-reference-not-bundle). |

## `[theme.gtk]` — GTK theme reference

```toml
[theme.gtk]
name = "Nordic-darker"
source = "gnome-look"
content_id = 1267246
file_id = 1654321
```

| Field        | Type   | Required | Description                                                       |
|--------------|--------|----------|-------------------------------------------------------------------|
| `name`       | string | yes      | The theme name as it should appear in `~/.local/share/themes/`.   |
| `source`     | string | yes      | Currently always `"gnome-look"`. Future: `"local"`, `"github"`.   |
| `content_id` | int    | yes      | The gnome-look.org content ID (the page-level identifier).        |
| `file_id`    | int    | yes      | The gnome-look.org file ID (the per-download identifier; bumps when the author uploads a new version). |

## `[theme.shell]` — Shell theme reference

Same fields as `[theme.gtk]`. The Shell theme is applied through the
**User Themes** extension, which GNOME X will install if missing.

## `[icons]` — icon pack reference

Same fields as `[theme.gtk]`. Installed under `~/.local/share/icons/<name>/`
and applied via `org.gnome.desktop.interface icon-theme`.

## `[cursor]` — cursor theme reference

Same fields as `[theme.gtk]`. Installed under
`~/.local/share/icons/<name>/cursors/` and applied via
`org.gnome.desktop.interface cursor-theme`.

## `[[extensions]]` — GNOME Shell extensions

Repeatable. One block per extension.

```toml
[[extensions]]
uuid = "blur-my-shell@aunetx"
name = "Blur my Shell"
required = true                # default: true

[[extensions]]
uuid = "arcmenu@arcmenu.com"
name = "ArcMenu"
required = false
```

| Field      | Type    | Required | Default | Description                                                |
|------------|---------|----------|---------|------------------------------------------------------------|
| `uuid`     | string  | yes      | —       | The extension UUID as it appears on extensions.gnome.org and in `~/.local/share/gnome-shell/extensions/`. |
| `name`     | string  | yes      | —       | Human-readable display name. Shown in the apply-progress UI. |
| `required` | boolean | no       | `true`  | If `true`, the apply step fails if installation fails. If `false`, failures are logged but applying continues. Use `false` for nice-to-haves. |

On apply, GNOME X:

1. Checks if the UUID is already installed locally.
2. If not, calls `org.gnome.Shell.Extensions.InstallRemoteExtension` over
   D-Bus, which fetches from extensions.gnome.org.
3. After install, calls `EnableExtension` on the same UUID.

## `[[settings]]` — arbitrary GSettings overrides

Repeatable. One block per key. Use sparingly — the schema/key pair must
match a GSettings schema actually present on the target system.

```toml
[[settings]]
key = "org.gnome.desktop.interface.color-scheme"
value = "prefer-dark"

[[settings]]
key = "org.gnome.desktop.interface.font-name"
value = "Inter 11"

[[settings]]
key = "org.gnome.shell.app-switcher.current-workspace-only"
value = "true"
```

| Field   | Type   | Required | Description                                                                 |
|---------|--------|----------|-----------------------------------------------------------------------------|
| `key`   | string | yes      | Fully-qualified GSettings key — `<schema>.<key>` joined with a dot. Last segment is the key; everything before is the schema id. |
| `value` | string | yes      | The value as a string. GSettings type coercion happens at apply-time based on the key's declared type. Booleans use `"true"` / `"false"`. Integers use decimal. Strings need no extra quoting. |

!!! warning "Keep this list small"
    `[[settings]]` is a power-user surface. The more keys a pack writes,
    the more brittle it gets across machines (different GNOME versions
    have different default values, deprecated keys, etc.). Prefer the
    typed sections (`[theme.*]`, `[icons]`, `[cursor]`, `[[extensions]]`)
    when they cover what you need. Use `[[settings]]` only for things
    those don't.

## `[[shell_tweaks]]` — typed Shell tweaks (`pack_format ≥ 2`)

Repeatable. One block per tweak. Slimmer than `[[settings]]` because the
`id` is a stable, GNOME-X-internal slug rather than a GSettings key — so
the meaning is preserved across GNOME versions even when the underlying
key changes.

```toml
[[shell_tweaks]]
id = "panel_position"
value = "bottom"

[[shell_tweaks]]
id = "blur_my_shell_panel"
value = "true"
```

| Field   | Type   | Required | Description                                                            |
|---------|--------|----------|------------------------------------------------------------------------|
| `id`    | string | yes      | Snake-case GNOME X tweak id. Stable across GNOME versions.             |
| `value` | string | yes      | String form of the value. Type depends on the `id` (bool/int/enum/position). |

This section is **only present in `pack_format ≥ 2` packs**. In v1 packs
it is absent; on import, v1 packs get an empty `shell_tweaks` list and
behave as before. See [`PackFile`][packfile] for the deserialiser logic.

[packfile]: https://github.com/leechristophermurray/gnome-x/blob/main/crates/infra/src/pack_toml_storage.rs#L42

## Complete example

```toml
[pack]
id = "nordic-elegance"
name = "Nordic Elegance"
description = "A clean, dark Nordic-inspired desktop with blur effects, a dock, and an application menu"
author = "Ada Lovelace"
created = "2026-04-13T12:00:00Z"
shell_version = "47"
pack_format = 1
wallpaper = "file:///usr/share/backgrounds/gnome/adwaita-d.webp"

[theme.gtk]
name = "Nordic-darker"
source = "gnome-look"
content_id = 1267246
file_id = 1654321

[theme.shell]
name = "Nordic-darker"
source = "gnome-look"
content_id = 1267246
file_id = 1654322

[icons]
name = "Papirus-Dark"
source = "gnome-look"
content_id = 1166289
file_id = 9876543

[cursor]
name = "Bibata-Modern-Classic"
source = "gnome-look"
content_id = 1197198
file_id = 5555555

[[extensions]]
uuid = "user-theme@gnome-shell-extensions.gcampax.github.com"
name = "User Themes"
required = true

[[extensions]]
uuid = "blur-my-shell@aunetx"
name = "Blur my Shell"
required = true

[[extensions]]
uuid = "dash-to-dock@micxgx.gmail.com"
name = "Dash to Dock"
required = true

[[extensions]]
uuid = "arcmenu@arcmenu.com"
name = "ArcMenu"
required = false

[[settings]]
key = "org.gnome.desktop.interface.color-scheme"
value = "prefer-dark"

[[settings]]
key = "org.gnome.desktop.interface.font-name"
value = "Inter 11"
```

A live copy of this example lives at
[`CONTEXT/sample-nordic-elegance.gnomex-pack.toml`][sample] in the repo.

[sample]: https://github.com/leechristophermurray/gnome-x/blob/main/CONTEXT/sample-nordic-elegance.gnomex-pack.toml

## Validation rules

On import, GNOME X enforces:

- `[pack]` is present and contains all required fields.
- `pack_format` is a known version (currently `1` or `2`).
- Every `[[extensions]].uuid` is a non-empty string.
- Every `[[settings]].key` contains at least one dot (it's a schema-qualified key).
- Every `content_id` and `file_id` is a positive integer.

A pack that fails any of these is **rejected at import** with a descriptive
error in the toast. Partially-valid packs are not silently truncated.

## Hand-editing a pack

Edit `~/.local/share/gnome-x/packs/<id>/pack.toml` directly. GNOME X
re-reads the file each time the **Packs** tab opens — no daemon to
restart, no cache to invalidate. Common edits:

- **Add an extension** to an existing pack: append a `[[extensions]]` block.
- **Remove a forced extension**: change `required = true` to
  `required = false`, or delete the block entirely.
- **Update a stale `file_id`**: open the gnome-look.org page in a browser,
  copy the new file ID from the download link, paste it in.
- **Change the screenshot**: replace `screenshot.webp` in the same
  directory.

Re-export the pack (Packs tab → row → **Export**) to share the edited
version.

## Where to go next

- [Build a pack](../tutorials/build-a-pack.md) — generate a manifest
  through the UI.
- [Apply a pack](../tutorials/apply-a-pack.md) — see how each section is
  applied in practice.
- [GSettings reference](gsettings.md) — for the keys you might put in
  `[[settings]]`.
