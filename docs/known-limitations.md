# Known limitations

GNOME X drives GNOME's own theming and GSettings surfaces — it does
not rewrite the compositor, the toolkit, or the browser. A handful of
rendering issues live *under* those surfaces and therefore can't be
fixed from our side. This document is the honest list.

---

## HiDPI and fractional scaling

### Symptom

Buttons, text, and icons in some applications look *blurry* when the
desktop is set to a fractional scale factor (125 %, 150 %, 175 %). The
effect is most noticeable on 14" 2880×1800 panels where the only
practical choices are 175 % (slightly small UI) and 200 % (slightly
large UI).

### Cause

GNOME on Wayland computes fractional scale by rendering at the next
integer factor and downsampling. Apps that respect GTK's scale factor
(Nautilus, Files, Settings, native GNOME apps) render crisply because
they're drawn at the target resolution directly. Apps that apply
their *own* scaling on top of GTK's scaling compound the error:

- **Chromium-based apps** (Chrome, Chromium, Brave, Edge, Cursor,
  VS Code, Antigravity, Electron apps) apply their internal zoom
  factor before the compositor scales them, so you end up with
  double-scaling artifacts.
- **X11 apps under XWayland** (any `--ozone-platform=x11`, legacy GTK
  3 apps run without Wayland backend, most Wine/Proton windows) are
  scaled as a texture by the compositor. The result is always
  bilinearly upsampled and looks soft.
- **Qt apps** (VLC, KeePassXC, OBS under older builds) ship their own
  scaling logic that often disagrees with GTK's.

### Mitigations

GNOME X can't fix the underlying compositor/toolkit interaction, but
these are the best workarounds:

1. **Prefer integer scales (100 %, 200 %, 300 %) when practical.**
   They always render crisply on every app category because no
   downsampling step is introduced.

2. **For 14"/15" panels around 2.5–2.8 K**, 150 % tends to look
   better than 175 % — the upstream scale produces cleaner integer
   pixel boundaries for most app framebuffers.

3. **For Chromium-family browsers**, launch with
   `--ozone-platform=wayland --enable-features=WaylandWindowDecorations`
   so they render at the compositor's native scale instead of
   applying their own. You can make this permanent by editing the
   `.desktop` file's `Exec=` line, or set the same flags via
   `~/.config/chromium-flags.conf`.

4. **For VS Code-family editors**, the same trick applies —
   `"window.titleBarStyle": "native"` in `settings.json` + an
   `--ozone-platform=wayland` flag via the launch shortcut.

5. **For X11 apps you can't move off XWayland**, accept soft
   rendering or run at integer scale.

### Font size vs scaling

"Just turn up the font size" works for text but leaves buttons,
icons, and padding unchanged — apps end up visually unbalanced
(large labels, tiny click targets). This is a separate concern from
DPI and GNOME X does not conflate them. See
[GXF-053](https://github.com/leechristophermurray/gnome-x/issues/23)
for the tracker on per-context font adjustment.

### Related tracker items

- [GXF-050 — Fractional scaling fine-tuning](https://github.com/leechristophermurray/gnome-x/issues/20)
  (exposing intermediate scales like 115 % / 133 % via Mutter)
- [GXF-051 — Per-app scaling overrides](https://github.com/leechristophermurray/gnome-x/issues/21)
  (GDK_SCALE / --force-device-scale-factor per `.desktop` entry)
- [GXF-052 — Scaling-induced blurriness documentation](https://github.com/leechristophermurray/gnome-x/issues/22)
  (this document)
- [GXF-053 — Font size vs desktop scaling decoupling](https://github.com/leechristophermurray/gnome-x/issues/23)

---

## GDM (login screen) theming

GDM runs under its own user (`gdm`) with its own GSettings database
and cannot read user CSS. Applying a GNOME X theme does not, by
itself, change the login screen — a user-level write to
`~/.config/gtk-4.0/gtk.css` or `~/.local/share/themes/` is not
visible to the `gdm` user.

### What GNOME X can do now (GXF-003)

GNOME X ships an **optional** pathway that elevates through polkit
and writes a small dconf snippet to the GDM database:

- **Toggle** — `apply-gdm-on-theme-apply` in our GSettings schema
  (Preferences → *Also apply to login screen*). Off by default.
- **Polkit action** — `org.gnomex.gdm-theme.apply` and
  `org.gnomex.gdm-theme.reset`, installed to
  `/usr/share/polkit-1/actions/org.gnomex.gdm-theme.policy`. Both
  require `auth_admin_keep` (admin password, cached for the session).
- **Elevated helper** — the hidden subcommands
  `experiencectl gdm-apply --theme <NAME> --accent <#rrggbb>` and
  `experiencectl gdm-reset`. These run as root via `pkexec` and
  write `/etc/dconf/db/gdm.d/90-gnome-x` before calling
  `dconf update` (and `restorecon` on SELinux systems, best-effort).

### Scope, v1

- We forward the **shell theme NAME** and the **accent colour name**
  (mapped to the nearest stock GNOME accent swatch). That's it. No
  CSS, no paths, no arbitrary hex — GDM's Shell expects the stock
  palette and rejects unknown names.
- Files we author always contain the literal substring `"GNOME X"`
  so `gdm-reset` can refuse to delete a file that isn't ours.
- On Fedora / RHEL, the helper runs `restorecon -R
  /etc/dconf/db/gdm.d/` so SELinux file contexts stay correct
  after the write. If `restorecon` is missing (Ubuntu / Arch
  defaults), that step is a silent no-op.

### What's still out of scope

- **Login-screen background images.** Controllable in theory via
  the same dconf surface, but the image itself lives in a path
  readable by the `gdm` user — a different problem and a larger
  attack surface. Not in v1.
- **Arbitrary distro GDM paths.** We target the Fedora / Ubuntu /
  Arch convention of `/etc/dconf/db/gdm.d/`. Distros with a
  different GDM config mechanism (e.g. non-dconf) need a separate
  adapter.
- **Full visual parity.** GDM's GNOME Shell runs a vendor-default
  GTK theme and will not pick up a `GNOME-X-Custom` shell theme
  bundle the way a logged-in session does. What the user sees at
  the login screen is the stock GNOME Shell with our theme NAME +
  accent applied on top — strongly consistent, but not byte-for-byte
  identical to the logged-in view.

### Tracker

- [GXF-003](https://github.com/leechristophermurray/gnome-x/issues/4)
  — this feature

---

## Flatpak is not a supported distribution target

GNOME X is **not published as a Flatpak** and there are no plans to
do so. The reasons are structural, not cosmetic:

- **Host-level writes.** GNOME X writes GTK CSS overrides to
  `~/.config/gtk-4.0/gtk.css` and `~/.config/gtk-3.0/gtk.css`, shell
  themes to `~/.local/share/themes/<Name>/gnome-shell/`, and icons
  to `~/.local/share/icons/`. The Flatpak sandbox blocks these paths
  by default; exposing them requires `--filesystem=home` (or
  near-equivalents), which defeats the point of the sandbox.
- **systemd user service install.** The theme-sync service is
  installed as a user unit under `~/.config/systemd/user/` and
  started through the user systemd bus. Flatpak does not expose
  host systemd, so this path stops working without additional
  portal holes that don't exist today.
- **Theme visibility inside other Flatpaks.** Even if GNOME X ran
  inside a sandbox, themes it wrote would not be visible to
  *other* Flatpak'd apps unless each of those apps had
  `--filesystem=~/.local/share/themes` overridden. That's not
  something GNOME X can do from inside its own sandbox.
- **D-Bus reach.** Managing extensions via
  `org.gnome.Shell.Extensions` requires unconditional access to
  the session bus name, which again requires loosening the
  sandbox to the point where it adds no safety.

Together these mean a Flatpak build of GNOME X would either be
**broken** (missing features) or **effectively unsandboxed**
(`--filesystem=home`, full session bus, systemd portal). Neither is
a useful product.

**Use one of these instead:**

- `.deb` / `.rpm` packages from the
  [Releases page](https://github.com/leechristophermurray/gnome-x/releases)
- `install.sh` from a source checkout (see
  [Installing](../README.md#installing) in the README)
- `cargo install --path crates/ui` for the binary only

---

## AppImage icon overrides

AppImages are self-contained `.AppImage` binaries that bundle their own
icon set (PNG/SVG files in the AppImage squashfs). When the user
launches one, several things can happen to its icons depending on how
the AppImage is integrated:

1. **Unintegrated AppImage (chmod +x, run directly).** The icon inside
   the binary is only visible while the app is running — the taskbar
   picks it up from `WM_ICON` / `_NET_WM_ICON` hints sent by the
   window itself. No icon theme is consulted, so GNOME X's icon pack
   selection has *no effect* on the running window icon.
2. **Integrated via AppImageLauncher, Gear Lever, or similar.** These
   tools extract the AppImage's `.desktop` file and icon into
   `~/.local/share/applications/` and
   `~/.local/share/icons/hicolor/*/apps/` at install time. From that
   point the `.desktop` file references an icon name, and GNOME looks
   that name up through the normal icon-theme lookup — **so a GNOME X
   icon pack *can* theme it**, provided the icon pack includes an
   entry with that exact name.
3. **Extracted manually (`--appimage-extract`).** Same behaviour as
   case 2 once the user wires the `.desktop` file into
   `~/.local/share/applications/`.

In practice cases (2) and (3) usually lose to the bundled icon because
AppImage authors install their icon into *the user's hicolor theme
directly* (`~/.local/share/icons/hicolor/256x256/apps/<app>.png`),
which takes precedence over any named-icon theme GNOME X activates.
This is a freedesktop.org icon-theme spec behaviour, not a GNOME X
bug: hicolor is the last-resort fallback *and* the place apps are
expected to drop their own icons, so a bundled icon there always wins.

### Workarounds for AppImage icons

- **Prefer icon packs that explicitly override common AppImage names.**
  Packs like Papirus, Tela, and WhiteSur do this for the top-50-ish
  AppImage apps (Cursor, OBS, Balena Etcher, Krita portable, etc.).
  GNOME X's Explore view surfaces these packs first.
- **For niche AppImages, the user can delete the bundled icon** from
  `~/.local/share/icons/hicolor/<size>/apps/` after installing — once
  it's gone the named-icon fallback chain reaches the active theme
  and the pack's icon applies.
- **Rebuilding the hicolor cache** (`gtk-update-icon-cache -f -t
  ~/.local/share/icons/hicolor`) is sometimes needed after either
  mitigation; GNOME Shell caches icons aggressively across restarts.

### What GNOME X can and can't do

GNOME X has no way to *detect* that a given icon was shipped inside an
AppImage rather than a regular `.desktop` install — by the time the
icon is in `~/.local/share/icons/hicolor/`, its provenance is gone.
The best we can offer is a future "Shadowed Resources" panel (tracker
item [GXF-012](https://github.com/leechristophermurray/gnome-x/issues/7))
that lists icons installed into hicolor with a user-writable flag, so
the user can see at a glance which icons are blocking their active
pack from taking effect. That panel is the planned follow-up to this
documentation.

---

## Browser / Electron theme fidelity

Even with our Chromium theme extension enabled, browsers cannot
respect the full GNOME X palette the way native GTK apps do. The
theme API is colors-only — there's no border-radius, no shadow-depth,
and no accent on scrollbars. Matching the GNOME frame requires
disabling Chrome's custom window frame entirely
([GXF-020](https://github.com/leechristophermurray/gnome-x/issues/8)),
which in turn loses Chrome's tab-strip-in-titlebar.

VS Code-family editors have similar constraints. See the
[external-app themers](../extension/io.github.gnomex@io.github.gnomex/)
for the scope of what's achievable via officially-supported APIs.

---

## Future port/adapter support — a note on cadence

GNOME X's Tree Architecture makes it *mechanically* easy to add new
external-app themers: each target (Chromium family, VS Code family,
Qt, Electron, etc.) becomes a new adapter implementing the
`ExternalAppThemer` port. So new targets *can* land — the plumbing
isn't the blocker.

What **is** the blocker is **maintenance latency**. Each of those
app families bumps its internal theme format on its own cadence,
often without a deprecation window:

- Chromium's theme manifest schema has silently renamed keys across
  major Chrome releases. A GNOME X Chromium adapter that shipped
  today can break on a Chrome update 30 days from now until someone
  tracks the new schema and pushes a fix.
- VS Code's `workbench.colorCustomizations` has added and removed
  keys across minor version bumps — we mitigate this by owning a
  delimited region, but the keys *within* that region still need to
  be kept in sync.
- Qt theming under GNOME is governed by the platform theme plugin
  (`qt5ct`, `qt6ct`, `qgnomeplatform`), each of which has its own
  release cadence. Any adapter we ship touches whichever plugin the
  user has installed, and that plugin may change its config format
  before we notice.

**Practical implication:** if you plan to rely on a future adapter,
expect a small window after every upstream release where the adapter
misbehaves until a catch-up fix lands. This isn't unique to GNOME X
— it's the cost of bridging between apps that don't coordinate their
theming surfaces with the desktop. We try to keep the window short,
but we're honest that it exists.

When an adapter breaks, the fix is localized to that adapter's file
— it won't cascade through the app. Users not using that adapter
aren't affected. And adapters with no upstream drift in a release
cycle need zero attention.
