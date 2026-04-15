#!/usr/bin/env bash
# Install GNOME X desktop integration files.
# Usage: sudo ./install.sh         (install to /usr/local)
#        PREFIX=/usr ./install.sh   (install to /usr)

set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
DATADIR="${PREFIX}/share"
BINARY="target/release/gnomex-ui"

# Build release binary if missing
if [ ! -f "${BINARY}" ]; then
    echo "Building release binary..."
    cargo build --release -p gnomex-ui
fi

echo "Installing GNOME X to ${PREFIX}..."

# Binary
install -Dm755 "${BINARY}" \
    "${PREFIX}/bin/gnome-x"

# Desktop file
install -Dm644 data/io.github.gnomex.GnomeX.desktop.in \
    "${DATADIR}/applications/io.github.gnomex.GnomeX.desktop"

# AppStream metadata
install -Dm644 data/io.github.gnomex.GnomeX.metainfo.xml.in \
    "${DATADIR}/metainfo/io.github.gnomex.GnomeX.metainfo.xml"

# GSettings schema
install -Dm644 data/io.github.gnomex.GnomeX.gschema.xml \
    "${DATADIR}/glib-2.0/schemas/io.github.gnomex.GnomeX.gschema.xml"

# Icons
install -Dm644 data/icons/scalable/apps/io.github.gnomex.GnomeX.svg \
    "${DATADIR}/icons/hicolor/scalable/apps/io.github.gnomex.GnomeX.svg"
install -Dm644 data/icons/symbolic/apps/io.github.gnomex.GnomeX-symbolic.svg \
    "${DATADIR}/icons/hicolor/symbolic/apps/io.github.gnomex.GnomeX-symbolic.svg"
install -Dm644 data/icons/128x128/apps/io.github.gnomex.GnomeX.png \
    "${DATADIR}/icons/hicolor/128x128/apps/io.github.gnomex.GnomeX.png"

# Systemd user timer for background accent scheduling
SYSTEMD_USER_DIR="${HOME}/.config/systemd/user"
mkdir -p "${SYSTEMD_USER_DIR}"
install -Dm644 data/systemd/gnome-x-accent-scheduler.service \
    "${SYSTEMD_USER_DIR}/gnome-x-accent-scheduler.service"
install -Dm644 data/systemd/gnome-x-accent-scheduler.timer \
    "${SYSTEMD_USER_DIR}/gnome-x-accent-scheduler.timer"
systemctl --user daemon-reload || true
systemctl --user enable --now gnome-x-accent-scheduler.timer || true
echo "Enabled accent scheduler timer (runs every 5 minutes)"

# Update caches
if command -v gtk-update-icon-cache &>/dev/null; then
    gtk-update-icon-cache -f -t "${DATADIR}/icons/hicolor" || true
fi
if command -v glib-compile-schemas &>/dev/null; then
    glib-compile-schemas "${DATADIR}/glib-2.0/schemas" || true
fi
if command -v update-desktop-database &>/dev/null; then
    update-desktop-database "${DATADIR}/applications" || true
fi

echo "Done. Run 'gnome-x' or find it in the app grid."
