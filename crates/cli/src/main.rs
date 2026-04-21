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
use gnomex_app::ports::{ShellCustomizer, ShellProxy, ThemeRenderTrigger};
use gnomex_app::use_cases::gdm_theme;
use gnomex_app::use_cases::{ApplyThemeUseCase, CustomizeShellUseCase, PacksUseCase};
use gnomex_domain::ThemeSpec;
use gnomex_infra::{
    ChromiumThemer, DbusShellProxy, DesktopAppLauncherOverrides, EgoClient, FilesystemInstaller,
    FilesystemThemeWriter, FsThemeRenderTrigger, GSettingsAppSettings, GSettingsAppearance,
    GSettingsBlurMyShell, GSettingsFloatingDock, GSettingsMutter, OcsClient, PackTomlStorage,
    PkexecGdmThemer, VscodeThemer,
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
    /// Apply a GNOME X theme NAME + accent to the GDM login-screen
    /// dconf DB. Requires root — normally invoked via pkexec through
    /// the `org.gnomex.gdm-theme.apply` polkit action.
    #[command(name = "gdm-apply", hide = true)]
    GdmApply {
        /// Shell theme NAME (not path). Must match [A-Za-z0-9._-]+.
        #[arg(long)]
        theme: String,
        /// Accent colour as #rrggbb.
        #[arg(long)]
        accent: String,
    },
    /// Remove GNOME X-authored overrides from the GDM dconf DB.
    /// Requires root; normally invoked via pkexec through the
    /// `org.gnomex.gdm-theme.reset` polkit action.
    #[command(name = "gdm-reset", hide = true)]
    GdmReset,
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
        Commands::GdmApply { theme, accent } => handle_gdm_apply(&theme, &accent),
        Commands::GdmReset => handle_gdm_reset(),
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
    let app = gio::Settings::new(APP_SCHEMA);
    match action {
        ThemeAction::Apply => {
            let spec = build_spec_from_gsettings()?;
            apply_uc.apply(&spec).context("failed to apply theme")?;
            println!(
                "Theme applied ({}) — restart apps to see changes",
                apply_uc.detected_version()
            );

            // GXF-003: opt-in extend to GDM. Off by default because it
            // prompts for an admin password via polkit. We forward only
            // the theme NAME + accent — the elevated helper hard-codes
            // the write path and re-validates both inputs.
            if app.boolean("apply-gdm-on-theme-apply") {
                match apply_uc.apply_to_gdm(gnomex_app::use_cases::CUSTOM_THEME_NAME, &spec.tint.accent_hex) {
                    Ok(()) => println!("Theme applied to GDM login screen as well."),
                    Err(e) => eprintln!(
                        "Warning: could not apply theme to GDM login screen: {e}"
                    ),
                }
            }
        }
        ThemeAction::Reset => {
            apply_uc.reset().context("failed to reset theme")?;
            println!("Theme overrides removed");
            if app.boolean("apply-gdm-on-theme-apply") {
                if let Err(e) = apply_uc.reset_gdm() {
                    eprintln!("Warning: could not reset GDM theme overrides: {e}");
                }
            }
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
            // The use case's `ThemeRenderTrigger` (wired in
            // `build_packs_use_case`) takes care of regenerating
            // GTK 4 + GTK 3 CSS from the just-restored tb-* values
            // (GXF-061).
            runtime
                .block_on(packs_uc.apply_pack(&id))
                .context("failed to apply pack")?;
            println!("Pack '{id}' applied (theme re-rendered).");
        }
    }
    Ok(())
}

/// Root-side implementation of the polkit-elevated "apply a theme to
/// GDM" action.
///
/// Trust boundary: assume the caller may be hostile. The only inputs
/// we honour are `theme` (re-validated here against the same
/// shell-identifier rules the unprivileged side uses) and `accent`
/// (re-parsed as a hex colour). No paths are ever read from the
/// caller; our write target is hard-coded to
/// `/etc/dconf/db/gdm.d/90-gnome-x`, and the dconf snippet we emit
/// is generated from the validated inputs only.
fn handle_gdm_apply(theme_name_raw: &str, accent_raw: &str) -> Result<()> {
    let theme = gdm_theme::validate_theme_name(theme_name_raw)
        .map_err(|e| anyhow::anyhow!("invalid theme name: {e}"))?;
    let accent = gdm_theme::validate_accent_hex(accent_raw)
        .map_err(|e| anyhow::anyhow!("invalid accent hex: {e}"))?;

    let path = gdm_theme::gdm_dconf_snippet_path(None);
    let snippet = gdm_theme::render_gdm_dconf_snippet(theme, &accent);
    tracing::info!("writing GDM dconf snippet to {}", path.display());

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    std::fs::write(&path, &snippet)
        .with_context(|| format!("writing {}", path.display()))?;

    // Re-build the gdm dconf DB. Best-effort: log but do not fail
    // the apply if the tool is missing — the file is already on disk
    // and the user's next reboot will trigger a rebuild via the
    // systemd unit that runs `dconf update` for GDM on package
    // upgrade anyway.
    run_best_effort("dconf", &["update"]);
    // SELinux: on Fedora/RHEL the file context has to match etc_t so
    // the gdm user can read it post-reboot. No-op if restorecon isn't
    // present (Ubuntu/Arch defaults).
    run_best_effort("restorecon", &["-R", gdm_theme::GDM_DCONF_DB_DIR]);

    println!(
        "GDM dconf snippet written to {} (theme='{theme}', accent='{}')",
        path.display(),
        accent.as_str()
    );
    Ok(())
}

fn handle_gdm_reset() -> Result<()> {
    let path = gdm_theme::gdm_dconf_snippet_path(None);
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            if !gdm_theme::looks_like_managed_snippet(&content) {
                anyhow::bail!(
                    "refusing to remove {}: missing the GNOME X managed-region marker \
                     (looks like this file was authored by someone else)",
                    path.display()
                );
            }
            std::fs::remove_file(&path)
                .with_context(|| format!("removing {}", path.display()))?;
            tracing::info!("removed {}", path.display());
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::info!("{} does not exist; nothing to reset", path.display());
        }
        Err(e) => {
            return Err(anyhow::anyhow!(
                "reading {}: {e}",
                path.display()
            ));
        }
    }
    run_best_effort("dconf", &["update"]);
    run_best_effort("restorecon", &["-R", gdm_theme::GDM_DCONF_DB_DIR]);
    println!("GDM theme overrides removed.");
    Ok(())
}

/// Best-effort subprocess spawn. Used for `dconf update` and
/// `restorecon`, both of which may be absent on some distros.
/// Absence is logged at debug level and treated as a no-op; a
/// non-zero exit is logged at warn level but not propagated.
fn run_best_effort(bin: &str, args: &[&str]) {
    match std::process::Command::new(bin).args(args).status() {
        Ok(s) if s.success() => {
            tracing::info!("{bin} {:?} ok", args);
        }
        Ok(s) => {
            tracing::warn!("{bin} {:?} exited with {s}", args);
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            tracing::debug!("{bin} not installed — skipping");
        }
        Err(e) => {
            tracing::warn!("spawning {bin}: {e}");
        }
    }
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
    println!(
        "Text scaling:          {:.2}x",
        app.double("tb-scaling-text-factor")
    );
    let mutter_fb = app.boolean("tb-scaling-monitor-framebuffer");
    let mutter_x11 = app.boolean("tb-scaling-x11-fractional");
    println!(
        "Fractional scaling:    framebuffer={} x11={}",
        if mutter_fb { "on" } else { "off" },
        if mutter_x11 { "on" } else { "off" }
    );
    let per_app = app.strv("tb-scaling-per-app-overrides");
    if !per_app.is_empty() {
        println!("Per-app scale overrides:");
        for entry in per_app.iter() {
            println!("  {}", entry.as_str());
        }
    }
    Ok(())
}

// --- Composition helpers ---

fn build_apply_theme_use_case() -> Result<ApplyThemeUseCase> {
    let shell_version = detect_shell_version();
    let generator: Arc<dyn gnomex_app::ports::ThemeCssGenerator> =
        Arc::from(gnomex_infra::theme_css::create_css_generator(&shell_version));
    let writer: Arc<FilesystemThemeWriter> = Arc::new(FilesystemThemeWriter::new());
    let appearance: Arc<GSettingsAppearance> = Arc::new(GSettingsAppearance::new());
    let mutter: Arc<GSettingsMutter> = Arc::new(GSettingsMutter::new());
    let launcher: Arc<DesktopAppLauncherOverrides> =
        Arc::new(DesktopAppLauncherOverrides::new());
    Ok(ApplyThemeUseCase::new(generator, writer, appearance)
        .with_external_themer(Arc::new(VscodeThemer::new()))
        .with_external_themer(Arc::new(ChromiumThemer::new()))
        .with_mutter_settings(mutter)
        .with_app_launcher_overrides(launcher)
        .with_gdm_themer(Arc::new(PkexecGdmThemer::new())))
}

fn build_ext_controllers() -> gnomex_infra::shell_customizer::ExtensionControllers {
    gnomex_infra::shell_customizer::ExtensionControllers {
        floating_dock: Arc::new(GSettingsFloatingDock::new()),
        blur_my_shell: Arc::new(GSettingsBlurMyShell::new()),
    }
}

#[allow(dead_code)]
fn build_customize_shell_use_case() -> Result<CustomizeShellUseCase> {
    let shell_version = detect_shell_version();
    let customizer: Arc<dyn ShellCustomizer> =
        Arc::from(gnomex_infra::shell_customizer::create_shell_customizer(
            &shell_version,
            build_ext_controllers(),
        ));
    Ok(CustomizeShellUseCase::new(customizer, &shell_version))
}

fn build_packs_use_case(handle: tokio::runtime::Handle) -> Result<PacksUseCase> {
    let ego_client: Arc<EgoClient> = Arc::new(EgoClient::new());
    let ocs_client: Arc<OcsClient> = Arc::new(OcsClient::new());
    let installer: Arc<FilesystemInstaller> = Arc::new(FilesystemInstaller::new());
    let appearance: Arc<GSettingsAppearance> = Arc::new(GSettingsAppearance::new());
    let pack_storage: Arc<PackTomlStorage> = Arc::new(PackTomlStorage::new());
    let app_settings: Arc<GSettingsAppSettings> = Arc::new(GSettingsAppSettings::new());
    let shell_version = detect_shell_version();
    let shell_customizer: Arc<dyn ShellCustomizer> =
        Arc::from(gnomex_infra::shell_customizer::create_shell_customizer(
            &shell_version,
            build_ext_controllers(),
        ));
    let shell_proxy: Arc<DbusShellProxy> = Arc::new(
        handle
            .block_on(DbusShellProxy::new())
            .context("failed to connect to GNOME Shell D-Bus")?,
    );
    let _ = ego_client; // reserved for future pack operations that need EGO

    // Wire a theme-render trigger so `apply_pack` regenerates both
    // `gtk-4.0/gtk.css` and `gtk-3.0/gtk.css` from the pack's
    // restored tb-* values (GXF-061).
    let apply_theme = Arc::new(build_apply_theme_use_case()?);
    let trigger: Arc<dyn ThemeRenderTrigger> =
        Arc::new(FsThemeRenderTrigger::new(apply_theme));

    Ok(PacksUseCase::new(
        pack_storage,
        appearance,
        shell_proxy,
        installer,
        ocs_client,
        app_settings,
        shell_customizer,
    )
    .with_theme_render_trigger(trigger))
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
///
/// Delegates to `gnomex_infra::gsettings_theme_spec::build_theme_spec_from_gsettings`
/// so the CLI and the pack-apply `FsThemeRenderTrigger` emit
/// byte-identical CSS for the same settings — the tb-* parsing logic
/// lives in exactly one place.
fn build_spec_from_gsettings() -> Result<ThemeSpec> {
    gnomex_infra::gsettings_theme_spec::build_theme_spec_from_gsettings()
        .context("reading tb-* gsettings")
}
