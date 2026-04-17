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
use gio::prelude::*;
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

    let app = adw::Application::builder()
        .application_id(APP_ID)
        .build();

    // Register bundled icons after GTK display is available.
    app.connect_startup(|_| {
        if let Some(display) = gtk::gdk::Display::default() {
            let icon_theme = gtk::IconTheme::for_display(&display);
            let icons_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../data/icons");
            if let Some(path_str) = icons_path
                .canonicalize()
                .ok()
                .and_then(|p| p.to_str().map(String::from))
            {
                icon_theme.add_search_path(&path_str);
                tracing::debug!("added icon search path: {path_str}");
            }
        }
    });

    let app = relm4::RelmApp::from_app(app);
    app.run::<AppModel>(services);

    // The glib main loop has exited. Force an immediate tokio shutdown
    // so background tasks (HTTP clients, D-Bus connections) don't keep
    // the process alive as an orphan.
    runtime.shutdown_timeout(std::time::Duration::from_millis(100));
    std::process::exit(0);
}
