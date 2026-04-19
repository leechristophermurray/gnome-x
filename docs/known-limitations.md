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

GDM runs under its own user with its own GSettings database and
cannot read user CSS. Applying a GNOME X theme does not change the
login screen. See
[GXF-003](https://github.com/leechristophermurray/gnome-x/issues/4)
for the plan to support GDM via polkit-elevated writes.

---

## Flatpak sandbox visibility

Flatpak apps run with a restricted filesystem view and don't see
`~/.local/share/themes/` by default. Themes installed by GNOME X will
not show up inside the flatpak'd app until one of:

- `flatpak override --filesystem=~/.local/share/themes` is run (we
  plan to automate this — see
  [GXF-011](https://github.com/leechristophermurray/gnome-x/issues/6))
- The theme is also copied to `~/.themes/` (the legacy path, which
  flatpak reads by default)

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
