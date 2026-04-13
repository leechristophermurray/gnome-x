# Contributing to GNOME X

Thank you for your interest in contributing to GNOME X. This document explains
how to set up your environment, how the codebase is organized, and what we
expect from contributions.

## Code of Conduct

This project follows the [GNOME Code of Conduct][coc]. Please read it before
participating. We are committed to providing a welcoming and inclusive
experience for everyone.

[coc]: https://conduct.gnome.org/

## Getting Started

### Prerequisites

- Rust stable toolchain (2024 edition)
- GTK 4.14+ and Libadwaita 1.5+ development headers
- A running GNOME Shell session (for D-Bus extension management)
- `pkg-config`

See the [README](README.md#requirements) for platform-specific package names.

### Building

```sh
git clone https://github.com/gnomex/gnome-x.git
cd gnome-x
cargo build -p gnomex-ui
cargo run -p gnomex-ui
```

### Running Tests

```sh
cargo test --workspace
```

Domain and application layer tests run without a display server. UI tests
require a running GTK environment.

## Architecture Overview

GNOME X uses a layered hexagonal architecture with four crates:

```
 L0  crates/domain/    Pure types, value objects, business rules (zero deps)
 L1  crates/app/       Use cases and port trait definitions
 L2  crates/ui/        GTK4 / Libadwaita / Relm4 application
 L3  crates/infra/     HTTP clients, D-Bus, GSettings, filesystem
```

**The fundamental rule**: dependencies flow inward only.

- `domain` depends on nothing external.
- `app` depends only on `domain`.
- `infra` implements `app` port traits using concrete adapters.
- `ui` depends on `app`, `domain`, and `infra` (for the composition root in
  `services.rs`).

Never import `infra` types from `app` or `domain`. Never import `ui` types
from anywhere else.

### Ports and Adapters

Every external dependency is abstracted behind a port trait in `crates/app/src/ports/`:

| Port                  | Adapter                | What it does                       |
|-----------------------|------------------------|------------------------------------|
| `ExtensionRepository` | `EgoClient`            | extensions.gnome.org HTTP API      |
| `ContentRepository`   | `OcsClient`            | gnome-look.org OCS API             |
| `ShellProxy`          | `DbusShellProxy`       | GNOME Shell D-Bus interface        |
| `LocalInstaller`      | `FilesystemInstaller`  | Install/uninstall to XDG dirs      |
| `AppearanceSettings`  | `GSettingsAppearance`  | Read/write desktop theme settings  |
| `PackStorage`         | `PackTomlStorage`      | Persist Experience Packs as TOML   |

To add a new external integration, define a port trait first, then implement
the adapter in `infra`.

## How to Contribute

### Reporting Issues

- Check [existing issues][issues] to avoid duplicates.
- Include your GNOME Shell version, GTK version, and distribution.
- For UI issues, attach a screenshot.
- For crashes, include the full backtrace (`RUST_BACKTRACE=1 cargo run`).

[issues]: https://github.com/gnomex/gnome-x/issues

### Proposing Changes

1. Open an issue describing what you want to change and why.
2. Fork the repository and create a feature branch from `main`.
3. Make your changes following the guidelines below.
4. Open a pull request against `main`.

Small fixes (typos, single-line bugs) can skip the issue step.

### Commit Messages

Use [Conventional Commits][cc] style:

```
feat: add wallpaper preview in detail view
fix: OCS search returning empty results for cursor category
refactor: extract shared landing list builder
docs: add Flatpak build instructions to README
```

- `feat:` --- new functionality
- `fix:` --- bug fix
- `refactor:` --- restructuring without behavior change
- `docs:` --- documentation only
- `chore:` --- build, CI, or tooling changes

Keep the subject line under 72 characters. Use the body for context on *why*,
not *what* (the diff shows the what).

[cc]: https://www.conventionalcommits.org/

### Branching

- `main` is the stable development branch.
- Feature branches: `feat/short-description`
- Bug fixes: `fix/short-description`

## Code Guidelines

### Rust Style

- **Edition**: 2024. Use current idioms.
- **Formatting**: `cargo fmt` before every commit.
- **Linting**: `cargo clippy --workspace` must pass with zero warnings.
- **Comments**: Only where the logic is not self-evident. Do not add
  doc-comments to private items that have clear names.
- **Error handling**: Use `AppError` variants for port failures. Use `?` for
  propagation. Do not `unwrap()` in library code.
- **Dependencies**: Check if the standard library or an existing workspace
  dependency solves the problem before adding a new crate.

### UI Guidelines

All UI changes must comply with the [GNOME Human Interface Guidelines][hig]:

- Use `Adw` widgets (`AdwActionRow`, `AdwStatusPage`, `AdwAlertDialog`, etc.)
  instead of raw GTK equivalents where an Adwaita variant exists.
- Use Libadwaita [CSS variables][css-vars] and built-in style classes
  (`boxed-list`, `suggested-action`, `dim-label`, etc.) instead of hardcoded
  colours or custom CSS.
- Support adaptive layouts. Test at narrow (360px) and wide (900px+) widths.
- All interactive elements must be keyboard-accessible.
- Provide tooltip text for icon-only buttons.
- Use `AdwToastOverlay` for transient feedback, not dialogs.

[hig]: https://developer.gnome.org/hig/
[css-vars]: https://gnome.pages.gitlab.gnome.org/libadwaita/doc/1-latest/css-variables.html

### Adding a New View

1. Create the component in `crates/ui/src/components/`.
2. Register it in `components/mod.rs`.
3. If it needs async data, add a use case method in `crates/app/src/use_cases/`.
4. If it needs a new external source, define a port in `crates/app/src/ports/`
   and implement the adapter in `crates/infra/src/`.
5. Wire the use case into `AppServices` in `crates/ui/src/services.rs`.
6. Add the tab or navigation entry in `crates/ui/src/app.rs`.

### Adding a New Infrastructure Adapter

1. Define the port trait in `crates/app/src/ports/`.
2. Re-export it from `crates/app/src/ports/mod.rs`.
3. Implement the adapter in `crates/infra/src/`.
4. Export it from `crates/infra/src/lib.rs`.
5. Construct it in `AppServices::new()` and inject it into the relevant
   use case.

### Experience Pack Format

The pack TOML schema is defined by the serde DTOs in
`crates/infra/src/pack_toml_storage.rs`. If you change the schema:

- Bump `pack_format` in the domain type.
- Add migration logic in `pack_file_to_domain()` for the previous format.
- Update the example in the README.

## Desktop Integration Files

Files under `data/` are installed system-wide and follow
[freedesktop.org][fd] and [GNOME][gnome-infra] specifications:

| File                                   | Specification                 |
|----------------------------------------|-------------------------------|
| `io.github.gnomex.GnomeX.desktop.in`  | [Desktop Entry][de-spec]      |
| `io.github.gnomex.GnomeX.metainfo.xml.in` | [AppStream][as-spec]      |
| `io.github.gnomex.GnomeX.gschema.xml` | [GSettings Schema][gs-spec]   |
| `icons/`                               | [Icon Theme][icon-spec]       |

When modifying these files, validate them:

```sh
# AppStream
appstreamcli validate data/io.github.gnomex.GnomeX.metainfo.xml.in

# Desktop file
desktop-file-validate data/io.github.gnomex.GnomeX.desktop.in

# GSettings schema
glib-compile-schemas --strict data/
```

[fd]: https://www.freedesktop.org/wiki/
[gnome-infra]: https://developer.gnome.org/documentation/guidelines/maintainer/integrating.html
[de-spec]: https://specifications.freedesktop.org/desktop-entry-spec/latest/
[as-spec]: https://www.freedesktop.org/software/appstream/docs/
[gs-spec]: https://docs.gtk.org/gio/class.Settings.html
[icon-spec]: https://specifications.freedesktop.org/icon-theme-spec/latest/

## Review Checklist

Before requesting review, verify:

- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --workspace` has zero warnings
- [ ] `cargo test --workspace` passes
- [ ] `cargo build --release -p gnomex-ui` succeeds
- [ ] The app launches and the changed feature works visually
- [ ] No new `unwrap()` calls in library code
- [ ] New port traits have corresponding adapter implementations
- [ ] UI follows GNOME HIG (adaptive, keyboard-accessible, Adwaita widgets)
- [ ] Commit messages follow Conventional Commits

## License

By contributing to GNOME X, you agree that your contributions will be licensed
under the [Apache License 2.0](LICENSE).

```
SPDX-License-Identifier: Apache-2.0
```
