// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

// GNOME 46 adapter — pass-through. Libadwaita 1.5 brought the native
// accent-color plumbing but didn't touch the overview/dash internals
// this extension hooks into, so nothing to override yet.

import { ShellCustomizerBase } from './shell_customizer.js';

export class Gnome46Customizer extends ShellCustomizerBase {
    versionLabel() {
        return 'GNOME 46';
    }
}
