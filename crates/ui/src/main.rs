// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! # GNOME X
//!
//! Composition Root — wires infrastructure adapters to application ports
//! and launches the Relm4 UI.

mod app;
mod components;

use app::AppModel;

const APP_ID: &str = "io.github.gnomex.GnomeX";

fn main() {
    // Structured logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("gnomex=info")),
        )
        .init();

    tracing::info!("starting GNOME X");

    let app = relm4::RelmApp::new(APP_ID);
    app.run::<AppModel>(());
}
