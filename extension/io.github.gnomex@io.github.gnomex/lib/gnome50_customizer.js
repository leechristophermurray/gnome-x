// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

// GNOME 50 / Libadwaita 1.9 adapter — pass-through.
// When the hypothesised `_bgManagers` → `_backgroundGroup` rename or
// the new BlurMode constant lands, override `_backgroundActors` or
// the BlurEffect construction here. The rest of the extension stays
// untouched.

import { ShellCustomizerBase } from './shell_customizer.js';

export class Gnome50Customizer extends ShellCustomizerBase {
    versionLabel() {
        return 'GNOME 50 (Libadwaita 1.9)';
    }
}
