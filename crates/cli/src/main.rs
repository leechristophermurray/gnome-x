// Copyright 2026 GNOME X Contributors
// SPDX-License-Identifier: Apache-2.0

//! experiencectl — command-line interface for GNOME X.
//!
//! Yellow L2 adapter: parses CLI args, delegates to use cases, formats output.
//! No business logic. Each command wires the same domain/app/infra stack that
//! the GUI uses, just without GTK.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use gio::prelude::*;
use gnomex_app::ports::ShellProxy;
use gnomex_app::use_cases::{ApplyThemeUseCase, PacksUseCase};
use gnomex_domain::{
    DashSpec, ForegroundSpec, HeaderbarSpec, HexColor, InsetSpec, NotificationSpec, Opacity,
    PanelSpec, Radius, StatusColorSpec, ThemeSpec, TintSpec, WindowFrameSpec,
};
use gnomex_infra::{
    DbusShellProxy, EgoClient, FilesystemInstaller, FilesystemThemeWriter, GSettingsAppearance,
    OcsClient, PackTomlStorage,
};
use std::sync::Arc;

const APP_SCHEMA: &str = "io.github.gnomex.GnomeX";
const IFACE_SCHEMA: &str = "org.gnome.desktop.interface";

#[derive(Parser)]
#[command(name = "experiencectl")]
#[command(about = "GNOME X command-line control", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Set or get the system accent color
    Accent {
        #[command(subcommand)]
        action: AccentAction,
    },
    /// Apply or reset theme settings
    Theme {
        #[command(subcommand)]
        action: ThemeAction,
    },
    /// Manage Experience Packs
    Pack {
        #[command(subcommand)]
        action: PackAction,
    },
    /// Show current state and detected environment
    Status,
}

#[derive(Subcommand)]
enum AccentAction {
    /// Set the accent color (blue, teal, green, yellow, orange, red, pink, purple, slate)
    Set { color: String },
    /// Print the current accent color
    Get,
}

#[derive(Subcommand)]
enum ThemeAction {
    /// Apply the current theme builder settings (writes GTK + Shell CSS)
    Apply,
    /// Reset all custom theme overrides
    Reset,
}

#[derive(Subcommand)]
enum PackAction {
    /// List saved Experience Packs
    List,
    /// Apply an Experience Pack by id
    Apply { id: String },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("experiencectl=info")),
        )
        .without_time()
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Accent { action } => handle_accent(action),
        Commands::Theme { action } => handle_theme(action),
        Commands::Pack { action } => handle_pack(action),
        Commands::Status => handle_status(),
    }
}

fn handle_accent(action: AccentAction) -> Result<()> {
    let iface = gio::Settings::new(IFACE_SCHEMA);
    match action {
        AccentAction::Set { color } => {
            iface
                .set_string("accent-color", &color)
                .context("failed to set accent color")?;
            println!("Accent color set to: {color}");
        }
        AccentAction::Get => {
            let current = iface.string("accent-color");
            println!("{current}");
        }
    }
    Ok(())
}

fn handle_theme(action: ThemeAction) -> Result<()> {
    let apply_uc = build_apply_theme_use_case()?;
    match action {
        ThemeAction::Apply => {
            let spec = build_spec_from_gsettings()?;
            apply_uc.apply(&spec).context("failed to apply theme")?;
            println!(
                "Theme applied ({}) — restart apps to see changes",
                apply_uc.detected_version()
            );
        }
        ThemeAction::Reset => {
            apply_uc.reset().context("failed to reset theme")?;
            println!("Theme overrides removed");
        }
    }
    Ok(())
}

fn handle_pack(action: PackAction) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let packs_uc = build_packs_use_case(runtime.handle().clone())?;

    match action {
        PackAction::List => {
            let summaries = packs_uc.list_packs().context("failed to list packs")?;
            if summaries.is_empty() {
                println!("No Experience Packs saved.");
            } else {
                for s in summaries {
                    println!("{}  —  {}  (by {})", s.id, s.name, s.author);
                }
            }
        }
        PackAction::Apply { id } => {
            runtime
                .block_on(packs_uc.apply_pack(&id))
                .context("failed to apply pack")?;
            println!("Pack '{id}' applied.");
        }
    }
    Ok(())
}

fn handle_status() -> Result<()> {
    // Detected GNOME version
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let shell_version = {
        let handle = runtime.handle().clone();
        let shell_proxy = handle
            .block_on(DbusShellProxy::new())
            .context("failed to connect to GNOME Shell D-Bus")?;
        handle.block_on(shell_proxy.get_shell_version()).ok()
    };

    let iface = gio::Settings::new(IFACE_SCHEMA);
    let app = gio::Settings::new(APP_SCHEMA);

    println!("=== GNOME X Status ===");
    if let Some(v) = shell_version {
        println!("Shell version:         {v}");
    }
    println!("Accent color:          {}", iface.string("accent-color"));
    println!("Color scheme:          {}", iface.string("color-scheme"));
    println!(
        "Scheduled accent:      {}",
        if app.boolean("scheduled-accent-enabled") {
            "ON"
        } else {
            "off"
        }
    );
    if app.boolean("scheduled-accent-enabled") {
        println!(
            "  Day accent:          {}",
            app.string("day-accent-color")
        );
        println!(
            "  Night accent:        {}",
            app.string("night-accent-color")
        );
        let nl = gio::Settings::new("org.gnome.settings-daemon.plugins.color");
        let from_h = nl.double("night-light-schedule-from");
        let to_h = nl.double("night-light-schedule-to");
        let auto = nl.boolean("night-light-schedule-automatic");
        println!(
            "  Schedule:            {:.0}:00 → {:.0}:00 ({})",
            from_h,
            to_h,
            if auto { "sunrise/sunset" } else { "fixed" }
        );
    }
    println!("Shared tinting:        {}", app.boolean("shared-tinting-enabled"));
    println!(
        "Window radius:         {:.0}px",
        app.double("tb-window-radius")
    );
    println!(
        "Element radius:        {:.0}px",
        app.double("tb-element-radius")
    );
    println!("Panel opacity:         {:.0}%", app.double("tb-panel-opacity"));
    println!("Tint intensity:        {:.0}%", app.double("tb-tint-intensity"));
    Ok(())
}

// --- Composition helpers ---

fn build_apply_theme_use_case() -> Result<ApplyThemeUseCase> {
    let shell_version = detect_shell_version();
    let generator: Arc<dyn gnomex_app::ports::ThemeCssGenerator> =
        Arc::from(gnomex_infra::theme_css::create_css_generator(&shell_version));
    let writer: Arc<FilesystemThemeWriter> = Arc::new(FilesystemThemeWriter::new());
    let appearance: Arc<GSettingsAppearance> = Arc::new(GSettingsAppearance::new());
    Ok(ApplyThemeUseCase::new(generator, writer, appearance))
}

fn build_packs_use_case(handle: tokio::runtime::Handle) -> Result<PacksUseCase> {
    let ego_client: Arc<EgoClient> = Arc::new(EgoClient::new());
    let ocs_client: Arc<OcsClient> = Arc::new(OcsClient::new());
    let installer: Arc<FilesystemInstaller> = Arc::new(FilesystemInstaller::new());
    let appearance: Arc<GSettingsAppearance> = Arc::new(GSettingsAppearance::new());
    let pack_storage: Arc<PackTomlStorage> = Arc::new(PackTomlStorage::new());
    let shell_proxy: Arc<DbusShellProxy> = Arc::new(
        handle
            .block_on(DbusShellProxy::new())
            .context("failed to connect to GNOME Shell D-Bus")?,
    );
    let _ = ego_client; // reserved for future pack operations that need EGO
    Ok(PacksUseCase::new(
        pack_storage,
        appearance,
        shell_proxy,
        installer,
        ocs_client,
    ))
}

/// Detect the running GNOME Shell major version via D-Bus, falling back to 47.
fn detect_shell_version() -> gnomex_domain::ShellVersion {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");
    runtime
        .block_on(async {
            let proxy = DbusShellProxy::new().await.ok()?;
            proxy.get_shell_version().await.ok()
        })
        .unwrap_or_else(|| gnomex_domain::ShellVersion::new(47, 0))
}

/// Build a ThemeSpec from the current GSettings tb-* keys.
fn build_spec_from_gsettings() -> Result<ThemeSpec> {
    let app = gio::Settings::new(APP_SCHEMA);
    let iface = gio::Settings::new(IFACE_SCHEMA);
    let accent_name = iface.string("accent-color").to_string();
    let accent_hex = accent_name_to_hex(&accent_name);

    Ok(ThemeSpec {
        window_radius: Radius::new(app.double("tb-window-radius"))?,
        element_radius: Radius::new(app.double("tb-element-radius"))?,
        panel: PanelSpec {
            radius: Radius::new(app.double("tb-panel-radius"))?,
            opacity: Opacity::from_percent(app.double("tb-panel-opacity"))?,
            tint: HexColor::new(&app.string("tb-panel-tint"))?,
        },
        dash: DashSpec {
            opacity: Opacity::from_percent(app.double("tb-dash-opacity"))?,
        },
        tint: TintSpec {
            accent_hex: HexColor::new(&accent_hex)?,
            intensity: Opacity::from_percent(app.double("tb-tint-intensity"))?,
        },
        headerbar: HeaderbarSpec {
            min_height: Radius::new(app.double("tb-headerbar-height"))?,
            shadow_intensity: Opacity::from_fraction(app.double("tb-headerbar-shadow"))?,
            circular_buttons: app.boolean("tb-circular-buttons"),
        },
        window_frame: WindowFrameSpec {
            show_shadow: app.boolean("tb-show-window-shadow"),
            inset_border: Radius::new(app.double("tb-inset-border"))?,
        },
        insets: InsetSpec {
            card_border_width: Radius::new(app.double("tb-card-border-width"))?,
            separator_opacity: Opacity::from_percent(app.double("tb-separator-opacity"))?,
            focus_ring_width: Radius::new(app.double("tb-focus-ring-width"))?,
            combo_inset: app.boolean("tb-combo-inset"),
        },
        foreground: ForegroundSpec::default(),
        status_colors: StatusColorSpec::default(),
        notifications: NotificationSpec {
            radius: Radius::new(app.double("tb-notification-radius"))?,
            opacity: Opacity::from_fraction(app.double("tb-notification-opacity"))?,
        },
        overview_blur: app.boolean("tb-overview-blur"),
    })
}

fn accent_name_to_hex(name: &str) -> String {
    match name {
        "blue" => "#3584e4",
        "teal" => "#2190a4",
        "green" => "#3a944a",
        "yellow" => "#c88800",
        "orange" => "#ed5b00",
        "red" => "#e62d42",
        "pink" => "#d56199",
        "purple" => "#9141ac",
        "slate" => "#6f8396",
        _ => "#3584e4",
    }
    .to_owned()
}
