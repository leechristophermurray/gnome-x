#!/usr/bin/env bash
# Install GNOME X desktop integration files.
# Usage: sudo ./install.sh         (install to /usr/local)
#        PREFIX=/usr ./install.sh   (install to /usr)

set -euo pipefail

PREFIX="${PREFIX:-/usr/local}"
DATADIR="${PREFIX}/share"
UI_BINARY="target/release/gnome-x"
DAEMON_BINARY="target/release/experienced"
CLI_BINARY="target/release/experiencectl"

# Build release binaries if missing
if [ ! -f "${UI_BINARY}" ] || [ ! -f "${DAEMON_BINARY}" ] || [ ! -f "${CLI_BINARY}" ]; then
    echo "Building release binaries..."
    cargo build --release -p gnomex-ui -p gnomex-daemon -p gnomex-cli
fi

echo "Installing GNOME X to ${PREFIX}..."

# Binaries
install -Dm755 "${UI_BINARY}" "${PREFIX}/bin/gnome-x"
install -Dm755 "${DAEMON_BINARY}" "${PREFIX}/bin/experienced"
install -Dm755 "${CLI_BINARY}" "${PREFIX}/bin/experiencectl"

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

# Systemd user service for the experienced daemon
# Install to the invoking user's config dir even when running under sudo
TARGET_USER="${SUDO_USER:-$(whoami)}"
TARGET_HOME=$(getent passwd "${TARGET_USER}" | cut -d: -f6)
SYSTEMD_USER_DIR="${TARGET_HOME}/.config/systemd/user"
mkdir -p "${SYSTEMD_USER_DIR}"
# Service ships with /usr/bin/experienced (the packaged path); when doing
# a local install under /usr/local, rewrite the path on the fly.
sed "s#/usr/bin/experienced#${PREFIX}/bin/experienced#" data/systemd/experienced.service \
    > "${SYSTEMD_USER_DIR}/experienced.service"
chmod 644 "${SYSTEMD_USER_DIR}/experienced.service"
chown -R "${TARGET_USER}":"${TARGET_USER}" "${SYSTEMD_USER_DIR}" 2>/dev/null || true

# Remove any older bash-timer-based units
rm -f "${SYSTEMD_USER_DIR}/gnome-x-accent-scheduler.service" \
      "${SYSTEMD_USER_DIR}/gnome-x-accent-scheduler.timer" 2>/dev/null || true

# Reload and enable, running as the target user
sudo -u "${TARGET_USER}" systemctl --user daemon-reload 2>/dev/null || true
sudo -u "${TARGET_USER}" systemctl --user disable --now gnome-x-accent-scheduler.timer 2>/dev/null || true
sudo -u "${TARGET_USER}" systemctl --user enable --now experienced.service 2>/dev/null || \
    echo "Note: couldn't auto-enable experienced.service — run as ${TARGET_USER}:"
echo "       systemctl --user enable --now experienced.service"

# GNOME Shell extension (io.github.gnomex@io.github.gnomex) — drives
# native overview blur + floating dock by listening to our schema.
EXT_UUID="io.github.gnomex@io.github.gnomex"
EXT_DIR="${DATADIR}/gnome-shell/extensions/${EXT_UUID}"
install -d "${EXT_DIR}/lib"
install -Dm644 "extension/${EXT_UUID}/metadata.json"             "${EXT_DIR}/metadata.json"
install -Dm644 "extension/${EXT_UUID}/extension.js"              "${EXT_DIR}/extension.js"
install -Dm644 "extension/${EXT_UUID}/stylesheet.css"            "${EXT_DIR}/stylesheet.css"
install -Dm644 "extension/${EXT_UUID}/lib/shell_customizer.js"   "${EXT_DIR}/lib/shell_customizer.js"
install -Dm644 "extension/${EXT_UUID}/lib/floating_dock.js"      "${EXT_DIR}/lib/floating_dock.js"
install -Dm644 "extension/${EXT_UUID}/lib/gnome45_customizer.js" "${EXT_DIR}/lib/gnome45_customizer.js"
install -Dm644 "extension/${EXT_UUID}/lib/gnome46_customizer.js" "${EXT_DIR}/lib/gnome46_customizer.js"
install -Dm644 "extension/${EXT_UUID}/lib/gnome47_customizer.js" "${EXT_DIR}/lib/gnome47_customizer.js"
install -Dm644 "extension/${EXT_UUID}/lib/gnome50_customizer.js" "${EXT_DIR}/lib/gnome50_customizer.js"
echo "Installed Shell extension to ${EXT_DIR}"

# Try to enable the extension for the invoking user. Requires a shell
# reload to pick up the new system-wide extension — we best-effort it.
sudo -u "${TARGET_USER}" gnome-extensions enable "${EXT_UUID}" 2>/dev/null || \
    echo "Note: couldn't auto-enable extension — run as ${TARGET_USER}:"
echo "       gnome-extensions enable ${EXT_UUID}"
echo "       (and log out + back in if GNOME Shell needs to re-scan extensions)"

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

echo ""
echo "Done. GUI: 'gnome-x' (or find it in the app grid)"
echo "      CLI: 'experiencectl --help'"
echo "      Daemon: running as systemd user service 'experienced.service'"
