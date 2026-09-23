#!/usr/bin/env sh
# Installs SparkGauge for the current user: binary in ~/.local/bin, icon and
# launcher entry under ~/.local/share. Works from a release archive (uses the
# prebuilt binary next to this script) or from the source tree (builds it).
set -eu
cd "$(dirname "$0")"

if [ -x ./sparkgauge ] && [ ! -f Cargo.toml ]; then
    src=./sparkgauge
else
    cargo build --release --locked
    src=target/release/sparkgauge
fi

bin="$HOME/.local/bin/sparkgauge"
install -Dm755 "$src" "$bin"
install -Dm644 assets/sparkgauge.svg "$HOME/.local/share/icons/hicolor/scalable/apps/sparkgauge.svg"
# Absolute path: ~/.local/bin is not always on the PATH of the desktop session.
sed "s|^Exec=.*|Exec=$bin|" packaging/sparkgauge.desktop > sparkgauge.desktop.tmp
install -Dm644 sparkgauge.desktop.tmp "$HOME/.local/share/applications/sparkgauge.desktop"
rm -f sparkgauge.desktop.tmp
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$HOME/.local/share/applications" || true

echo "SparkGauge installed: $bin"
echo "Look for \"SparkGauge\" in your applications, or run: sparkgauge"
