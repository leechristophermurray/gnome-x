// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

// GNOME 47 / 48 / 49 adapter — pass-through. The majority of users
// target this range today; when a key method drifts it lands here
// first.

import { ShellCustomizerBase } from './shell_customizer.js';

export class Gnome47Customizer extends ShellCustomizerBase {
    versionLabel() {
        return 'GNOME 47+';
    }
}
