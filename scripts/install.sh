#!/bin/sh
# Builds opal and installs it for the current user: the program, a launcher
# entry (application menus, fuzzel, rofi…) and an icon.
# Usage: scripts/install.sh [PREFIX]    (default: ~/.local)
# Remove: rm PREFIX/bin/opal PREFIX/share/applications/opal.Editor.desktop \
#            PREFIX/share/icons/hicolor/scalable/apps/opal.Editor.svg
set -eu

prefix=${1:-$HOME/.local}
cd "$(dirname "$0")/.."

cargo build --release
mkdir -p "$prefix/bin" "$prefix/share/applications" "$prefix/share/icons/hicolor/scalable/apps"
install -m 755 target/release/opal "$prefix/bin/opal"
# The launcher gets the full path: it may not share the shell's PATH.
sed "s|@BINDIR@|$prefix/bin|" data/opal.Editor.desktop.in > "$prefix/share/applications/opal.Editor.desktop"
install -m 644 data/opal.Editor.svg "$prefix/share/icons/hicolor/scalable/apps/opal.Editor.svg"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q "$prefix/share/applications" || true
fi
echo "opal установлен в $prefix"
