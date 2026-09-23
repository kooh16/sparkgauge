#!/usr/bin/env sh
# Builds SparkGauge and installs it for the current user: binary in
# ~/.local/bin, icon and launcher entry under ~/.local/share.
set -eu
cd "$(dirname "$0")"

cargo build --release --locked

bin="$HOME/.local/bin/sparkgauge"
install -Dm755 target/release/sparkgauge "$bin"
install -Dm644 assets/sparkgauge.svg "$HOME/.local/share/icons/hicolor/scalable/apps/sparkgauge.svg"
# Absolute path: ~/.local/bin is not always on the PATH of the desktop session.
sed "s|^Exec=.*|Exec=$bin|" packaging/sparkgauge.desktop > sparkgauge.desktop.tmp
install -Dm644 sparkgauge.desktop.tmp "$HOME/.local/share/applications/sparkgauge.desktop"
rm -f sparkgauge.desktop.tmp
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database -q "$HOME/.local/share/applications" || true

echo "SparkGauge installed: $bin"
echo "Look for \"SparkGauge\" in your applications, or run: sparkgauge"
