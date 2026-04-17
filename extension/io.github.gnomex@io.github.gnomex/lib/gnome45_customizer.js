// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

// GNOME 45 adapter — pass-through to the base class for now.
// When a shell 45-specific API rename appears, override the affected
// method here so the other version files stay untouched.

import { ShellCustomizerBase } from './shell_customizer.js';

export class Gnome45Customizer extends ShellCustomizerBase {
    versionLabel() {
        return 'GNOME 45';
    }
}
