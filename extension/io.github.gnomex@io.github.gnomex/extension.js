// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

// Composition root for the GNOME X Integration extension.
//
// Mirrors the Rust workspace's Tree Architecture: this file selects
// the right *version adapter* at enable() time and wires it to our
// GSettings schema (io.github.gnomex.GnomeX). The adapter does all
// the actual shell-side work; this file is ~50 lines of glue.
//
// Dependency direction:
//
//   extension.js  (Yellow — composition)
//       ↓
//   ShellCustomizerBase  (Blue — the "port" interface + shared impl)
//       ↓
//   Gnome{N}Customizer   (Red — version-specific overrides)
//
// When a GNOME release renames an internal actor or changes a Clutter
// API, the fix lives in exactly one per-version file — everything
// else stays untouched, exactly like the Rust `ThemeCssGenerator`
// pattern.

import { Extension } from 'resource:///org/gnome/shell/extensions/extension.js';
import * as Config from 'resource:///org/gnome/shell/misc/config.js';

import { Gnome45Customizer } from './lib/gnome45_customizer.js';
import { Gnome46Customizer } from './lib/gnome46_customizer.js';
import { Gnome47Customizer } from './lib/gnome47_customizer.js';
import { Gnome50Customizer } from './lib/gnome50_customizer.js';

const KEY_OVERVIEW_BLUR = 'tb-overview-blur';
const KEY_FLOATING_DOCK = 'tb-floating-dock-enabled';

export default class GnomeXIntegration extends Extension {
    enable() {
        this._settings = this.getSettings();
        this._customizer = this._createAdapter();
        this._customizer.enable();

        // React to live GSettings changes. The Rust side writes these
        // keys whenever the user flips a toggle in the Shell tab, so
        // the loop is Rust → GSettings → us → Clutter.
        this._blurHandlerId = this._settings.connect(
            `changed::${KEY_OVERVIEW_BLUR}`,
            () => this._customizer.setOverviewBlur(this._settings.get_boolean(KEY_OVERVIEW_BLUR)),
        );
        this._dockHandlerId = this._settings.connect(
            `changed::${KEY_FLOATING_DOCK}`,
            () => this._customizer.setFloatingDock(this._settings.get_boolean(KEY_FLOATING_DOCK)),
        );

        // Apply the current state on enable so we don't wait for the
        // next toggle to catch up.
        this._customizer.setOverviewBlur(this._settings.get_boolean(KEY_OVERVIEW_BLUR));
        this._customizer.setFloatingDock(this._settings.get_boolean(KEY_FLOATING_DOCK));

        console.log(`[gnome-x] enabled (${this._customizer.versionLabel()})`);
    }

    disable() {
        if (this._settings && this._blurHandlerId) {
            this._settings.disconnect(this._blurHandlerId);
            this._settings.disconnect(this._dockHandlerId);
        }
        this._blurHandlerId = null;
        this._dockHandlerId = null;
        this._settings = null;

        if (this._customizer) {
            this._customizer.disable();
            this._customizer = null;
        }

        console.log('[gnome-x] disabled');
    }

    _createAdapter() {
        const major = parseInt(Config.PACKAGE_VERSION.split('.')[0], 10);
        if (major >= 50) return new Gnome50Customizer();
        if (major >= 47) return new Gnome47Customizer();
        if (major >= 46) return new Gnome46Customizer();
        return new Gnome45Customizer();
    }
}
