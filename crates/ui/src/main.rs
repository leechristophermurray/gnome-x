// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! # GNOME X
//!
//! Composition Root — wires infrastructure adapters to application ports
//! and launches the Relm4 UI.
//!
//! This is the only place where concrete infrastructure types are instantiated.
//! All other code depends on abstractions (ports).

mod app;
mod components;
mod services;

use app::AppModel;
use services::AppServices;

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

    // Build the tokio runtime for async I/O (HTTP, D-Bus, filesystem).
    // Relm4 uses the glib main loop, so we bridge via Handle + sender.emit().
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    let services = AppServices::new(runtime.handle().clone());

    let app = relm4::RelmApp::new(APP_ID);
    app.run::<AppModel>(services);
}
