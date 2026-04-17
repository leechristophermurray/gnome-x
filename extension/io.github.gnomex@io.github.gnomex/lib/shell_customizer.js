// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

// ShellCustomizerBase — the shared feature implementation that most
// GNOME versions will consume unchanged.
//
// The Rust port of the same name lives in `crates/app/src/ports/`.
// This class is its GJS twin: `setOverviewBlur` / `setFloatingDock`
// are the methods the extension.js composition root calls.
//
// Per-version subclasses (gnome45_customizer.js, etc.) inherit this
// and override only the specific hooks that drift between GNOME
// releases. Today every version uses the base implementation verbatim
// — the subclasses exist so drift can be absorbed in exactly one file
// when it appears.

import Shell from 'gi://Shell';

import * as Main from 'resource:///org/gnome/shell/ui/main.js';

import { FloatingDock } from './floating_dock.js';

// Visible identifier for the blur effect we attach, so we can find
// and detach it again cleanly on disable.
const BLUR_EFFECT_NAME = 'gnomex-overview-blur';

// Tunables. Conservative values that look decent across both light
// and dark GNOME defaults. Future iteration: surface these as their
// own GSettings keys.
const BLUR_SIGMA = 32;
const BLUR_BRIGHTNESS = 0.6; // < 1.0 darkens; adapts to dark mode naturally

export class ShellCustomizerBase {
    constructor() {
        // State flags — cached to avoid duplicate effect
        // attachments on repeated setOverviewBlur(true) calls.
        this._overviewBlurActive = false;
        this._blurredActors = [];
        this._overviewShownId = 0;
        this._overviewHidingId = 0;

        this._floatingDockActive = false;
        this._floatingDock = null;
    }

    versionLabel() {
        return 'GNOME Shell (base)';
    }

    enable() {
        // Subclasses may override to connect per-version signals.
    }

    disable() {
        // Make sure both effects are torn down even if the extension
        // is disabled while both are active.
        this.setOverviewBlur(false);
        this.setFloatingDock(false);
    }

    // --- Overview background blur ----------------------------------------

    setOverviewBlur(enabled) {
        if (enabled === this._overviewBlurActive) return;
        this._overviewBlurActive = enabled;
        if (enabled) {
            this._enableOverviewBlur();
        } else {
            this._disableOverviewBlur();
        }
    }

    _enableOverviewBlur() {
        // Blur is applied to wallpaper actors only while the overview
        // is visible — that's the native GNOME expectation. We install
        // show/hide hooks now and attach/detach effects from them.
        this._overviewShownId = Main.overview.connect('showing', () => {
            this._attachBlurToBackgrounds();
        });
        this._overviewHidingId = Main.overview.connect('hiding', () => {
            this._detachBlurFromBackgrounds();
        });

        // If the overview is already visible at toggle time, attach now.
        if (Main.overview.visible) {
            this._attachBlurToBackgrounds();
        }
    }

    _disableOverviewBlur() {
        if (this._overviewShownId) {
            Main.overview.disconnect(this._overviewShownId);
            this._overviewShownId = 0;
        }
        if (this._overviewHidingId) {
            Main.overview.disconnect(this._overviewHidingId);
            this._overviewHidingId = 0;
        }
        this._detachBlurFromBackgrounds();
    }

    _attachBlurToBackgrounds() {
        // GNOME Shell tracks one background actor per monitor. The
        // list is rebuilt on monitor changes — we re-fetch on each
        // overview show so hotplug events don't leave stale refs.
        //
        // `Shell.BlurEffect` exposes GObject *properties*, not setter
        // methods, on the versions we support. Use `set_property`
        // which is uniform across every GObject subclass.
        //
        // `BlurMode.ACTOR` blurs the actor's *own* pixels, which is
        // what we want for wallpaper actors at the bottom of the
        // stack. `BACKGROUND` mode reads whatever is rendered
        // behind — for a wallpaper that's nothing, so it produces
        // no visible effect.
        const actors = this._backgroundActors();
        for (const actor of actors) {
            if (actor.get_effect(BLUR_EFFECT_NAME)) continue;
            const effect = new Shell.BlurEffect();
            try {
                effect.set_property('sigma', BLUR_SIGMA);
                effect.set_property('brightness', BLUR_BRIGHTNESS);
                if (Shell.BlurMode && 'ACTOR' in Shell.BlurMode) {
                    effect.set_property('mode', Shell.BlurMode.ACTOR);
                }
            } catch (e) {
                console.log(`[gnome-x] blur configuration failed: ${e}`);
            }
            actor.add_effect_with_name(BLUR_EFFECT_NAME, effect);
            this._blurredActors.push(actor);
        }
        console.log(
            `[gnome-x] overview blur attached to ${this._blurredActors.length} background actor(s)`
        );
    }

    _detachBlurFromBackgrounds() {
        for (const actor of this._blurredActors) {
            // Silently skip actors that were reaped by the shell while
            // blur was attached — happens on monitor disconnect.
            try {
                actor.remove_effect_by_name(BLUR_EFFECT_NAME);
            } catch (_) {
                // ignore
            }
        }
        this._blurredActors = [];
    }

    /**
     * Find every current wallpaper actor. Default implementation reads
     * from the BackgroundManager list — overridden by per-version
     * subclasses when the internal attribute name drifts.
     */
    _backgroundActors() {
        const managers = Main.layoutManager._bgManagers;
        if (!managers || managers.length === 0) return [];
        return managers
            .map(m => m.backgroundActor)
            .filter(a => a !== null && a !== undefined);
    }

    // --- Floating dock ---------------------------------------------------

    setFloatingDock(enabled) {
        if (enabled === this._floatingDockActive) return;
        this._floatingDockActive = enabled;
        if (enabled) {
            this._enableFloatingDock();
        } else {
            this._disableFloatingDock();
        }
    }

    _enableFloatingDock() {
        if (this._floatingDock) return;
        try {
            this._floatingDock = new FloatingDock();
            console.log('[gnome-x] floating dock: enabled');
        } catch (e) {
            console.log(`[gnome-x] floating dock failed to start: ${e}`);
            this._floatingDock = null;
        }
    }

    _disableFloatingDock() {
        if (this._floatingDock) {
            this._floatingDock.destroy();
            this._floatingDock = null;
            console.log('[gnome-x] floating dock: disabled');
        }
    }
}

