// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

// Hand-rolled always-visible dock with autohide.
//
// Deliberately does NOT reuse `imports.ui.dash.Dash`: that class
// assumes it lives inside the overview's layout tree and asserts its
// allocation against stage views that don't exist for top-level chrome
// actors.
//
// Visibility model:
//   - Starts *visible* so the user sees it on first enable.
//   - After the cursor leaves, a short timer fires and the dock
//     slides down + fades out.
//   - A thin hot-zone actor pinned to the bottom edge catches
//     cursor-enter events and slides the dock back in.
//   - While the Activities overview is showing, the dock is
//     force-hidden and the hot-zone is disarmed — GNOME already
//     presents an app launcher there.

import Clutter from 'gi://Clutter';
import GLib from 'gi://GLib';
import Shell from 'gi://Shell';
import St from 'gi://St';

import * as AppFavorites from 'resource:///org/gnome/shell/ui/appFavorites.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const DOCK_MARGIN_BOTTOM = 16;
const ICON_SIZE = 40;
const HOTZONE_HEIGHT = 3; // pixels at the screen edge that trigger reveal
const HOTZONE_WIDTH_FRAC = 0.5; // 50% of monitor width, centred
const SHOW_DURATION_MS = 180;
const HIDE_DURATION_MS = 220;
const HIDE_DELAY_MS = 400;
const SLIDE_OFFSET = 24; // px the dock drops while hiding

export class FloatingDock {
    constructor() {
        this._container = new St.Bin({
            style_class: 'gnomex-floating-dock',
            reactive: true,
            track_hover: true,
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
        });

        this._box = new St.BoxLayout({
            vertical: false,
            style_class: 'gnomex-floating-dock-box',
        });
        this._container.set_child(this._box);

        // A short, wide, transparent strip at the bottom-center of
        // the primary monitor. Reactive so it receives enter-event,
        // but otherwise invisible and non-click-blocking in a 3px strip.
        this._hotZone = new Clutter.Actor({
            name: 'gnomex-dock-hotzone',
            reactive: true,
            opacity: 0,
        });

        this._favorites = AppFavorites.getAppFavorites();
        this._appSystem = Shell.AppSystem.get_default();

        // --- App-content signals ---
        this._favoritesId = this._favorites.connect('changed', () => this._queueRebuild());
        this._installedId = this._appSystem.connect('installed-changed', () => this._queueRebuild());
        this._appStateId = this._appSystem.connect('app-state-changed', () => this._queueRebuild());

        // --- Layout signals ---
        this._monitorsId = Main.layoutManager.connect('monitors-changed', () => this._reposition());

        // --- Overview signals: hide during overview, don't re-show after ---
        this._overviewShowingId = Main.overview.connect('showing', () => this._hide(/* immediate */ true));
        this._overviewHiddenId = Main.overview.connect('hidden', () => {
            // Intentional no-op: stay hidden until the user hovers
            // the hot-zone. Re-appearing the moment the overview
            // dismisses feels clingy.
        });

        // --- Reveal / autohide signals ---
        this._hotZoneEnterId = this._hotZone.connect('enter-event', () => this._show());
        this._enterId = this._container.connect('enter-event', () => this._cancelScheduledHide());
        this._leaveId = this._container.connect('leave-event', () => this._scheduleHide());

        this._rebuildPending = false;
        this._hideTimerId = 0;
        this._hidden = false;

        Main.layoutManager.addChrome(this._container, {
            affectsInputRegion: true,
            affectsStruts: false,
            trackFullscreen: true,
        });
        Main.layoutManager.addChrome(this._hotZone, {
            affectsInputRegion: true,
            affectsStruts: false,
            trackFullscreen: true,
        });

        this._rebuild();
    }

    _queueRebuild() {
        if (this._rebuildPending) return;
        this._rebuildPending = true;
        GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._rebuildPending = false;
            this._rebuild();
            return GLib.SOURCE_REMOVE;
        });
    }

    _rebuild() {
        if (!this._box) return;
        this._box.remove_all_children();

        const seen = new Set();
        const favorites = this._favorites.getFavorites();
        for (const app of favorites) {
            this._box.add_child(this._makeIcon(app, true));
            seen.add(app.get_id());
        }

        const running = this._appSystem.get_running().filter(a => !seen.has(a.get_id()));
        if (favorites.length > 0 && running.length > 0) {
            this._box.add_child(this._makeSeparator());
        }
        for (const app of running) {
            this._box.add_child(this._makeIcon(app, false));
        }

        if (this._box.get_n_children() === 0) {
            const placeholder = new St.Label({
                text: 'GNOME X Dock',
                style_class: 'gnomex-dock-placeholder',
            });
            this._box.add_child(placeholder);
        }

        this._container.queue_relayout();
        GLib.idle_add(GLib.PRIORITY_DEFAULT, () => {
            this._reposition();
            return GLib.SOURCE_REMOVE;
        });
    }

    _makeIcon(app, isFavorite) {
        const btn = new St.Button({
            style_class: isFavorite ? 'gnomex-dock-icon favorite' : 'gnomex-dock-icon running',
            reactive: true,
            can_focus: true,
            track_hover: true,
            x_align: Clutter.ActorAlign.CENTER,
            y_align: Clutter.ActorAlign.CENTER,
        });
        const icon = app.create_icon_texture(ICON_SIZE);
        btn.set_child(icon);
        btn.set_tooltip_text?.(app.get_name());
        btn.connect('clicked', () => this._activateApp(app));
        return btn;
    }

    _makeSeparator() {
        return new St.Widget({
            style_class: 'gnomex-dock-separator',
            y_expand: true,
        });
    }

    _activateApp(app) {
        const windows = app.get_windows();
        if (windows.length === 0) {
            app.activate();
            return;
        }
        Main.activateWindow(windows[0]);
    }

    // --- Positioning ------------------------------------------------------

    _reposition() {
        if (!this._container) return;
        const monitor = Main.layoutManager.primaryMonitor;
        if (!monitor) return;

        const [, natWidth] = this._container.get_preferred_width(-1);
        const [, natHeight] = this._container.get_preferred_height(natWidth);

        const dockX = monitor.x + Math.floor((monitor.width - natWidth) / 2);
        const dockY = monitor.y + monitor.height - natHeight - DOCK_MARGIN_BOTTOM;

        this._container.set_position(dockX, dockY);

        // Hot-zone sits pinned to the very bottom of the monitor,
        // spanning the middle band. Arrow navigating into the bottom
        // edge anywhere in that band reveals the dock.
        const hzWidth = Math.floor(monitor.width * HOTZONE_WIDTH_FRAC);
        const hzX = monitor.x + Math.floor((monitor.width - hzWidth) / 2);
        const hzY = monitor.y + monitor.height - HOTZONE_HEIGHT;
        this._hotZone.set_position(hzX, hzY);
        this._hotZone.set_size(hzWidth, HOTZONE_HEIGHT);
    }

    // --- Visibility state machine ---------------------------------------

    _show() {
        if (Main.overview.visible) return;
        this._cancelScheduledHide();
        if (!this._hidden) return;
        this._hidden = false;
        this._container.show();
        this._container.remove_all_transitions();
        // Clutter properties are hyphenated / snake_case (`translation-y`
        // / `translation_y`). camelCase keys in an `ease()` payload get
        // silently ignored, which leaves the dock clipped 24px below
        // its intended position after the first hide animation.
        this._container.ease({
            opacity: 255,
            translation_y: 0,
            duration: SHOW_DURATION_MS,
            mode: Clutter.AnimationMode.EASE_OUT_QUAD,
        });
    }

    _hide(immediate = false) {
        this._cancelScheduledHide();
        if (this._hidden) return;
        this._hidden = true;

        if (immediate) {
            this._container.remove_all_transitions();
            this._container.set_opacity(0);
            this._container.set_translation(0, SLIDE_OFFSET, 0);
            this._container.hide();
            return;
        }

        this._container.remove_all_transitions();
        this._container.ease({
            opacity: 0,
            translation_y: SLIDE_OFFSET,
            duration: HIDE_DURATION_MS,
            mode: Clutter.AnimationMode.EASE_IN_QUAD,
            onComplete: () => {
                if (this._hidden && this._container) {
                    this._container.hide();
                }
            },
        });
    }

    _scheduleHide() {
        this._cancelScheduledHide();
        this._hideTimerId = GLib.timeout_add(
            GLib.PRIORITY_DEFAULT,
            HIDE_DELAY_MS,
            () => {
                this._hideTimerId = 0;
                this._hide();
                return GLib.SOURCE_REMOVE;
            },
        );
    }

    _cancelScheduledHide() {
        if (this._hideTimerId) {
            GLib.source_remove(this._hideTimerId);
            this._hideTimerId = 0;
        }
    }

    // --- Lifecycle -------------------------------------------------------

    destroy() {
        this._cancelScheduledHide();

        const disc = (obj, id) => { if (obj && id) obj.disconnect(id); };
        disc(this._favorites, this._favoritesId);
        disc(this._appSystem, this._installedId);
        disc(this._appSystem, this._appStateId);
        disc(Main.layoutManager, this._monitorsId);
        disc(Main.overview, this._overviewShowingId);
        disc(Main.overview, this._overviewHiddenId);
        disc(this._hotZone, this._hotZoneEnterId);
        disc(this._container, this._enterId);
        disc(this._container, this._leaveId);
        this._favoritesId = this._installedId = this._appStateId = 0;
        this._monitorsId = this._overviewShowingId = this._overviewHiddenId = 0;
        this._hotZoneEnterId = this._enterId = this._leaveId = 0;

        if (this._hotZone) {
            Main.layoutManager.removeChrome(this._hotZone);
            this._hotZone.destroy();
            this._hotZone = null;
        }
        if (this._container) {
            Main.layoutManager.removeChrome(this._container);
            this._container.destroy();
            this._container = null;
        }
        this._box = null;
        this._favorites = null;
        this._appSystem = null;
    }
}
