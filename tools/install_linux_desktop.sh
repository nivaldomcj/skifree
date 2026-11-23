#!/bin/sh
# Install (or remove) a desktop entry and icon for SkiFree, for this user.
#
#   tools/install_linux_desktop.sh [path/to/skifree]   # default: target/release/skifree
#   tools/install_linux_desktop.sh --uninstall
#
# The game's window class / Wayland app id is "skifree", so desktops match
# it to skifree.desktop: native Wayland windows then get the SkiFree icon
# (Wayland takes icons from the desktop entry, not from the program), and
# the game shows up in the application menu. The icon is copied from the
# assets extracted from your ski32.exe (assets/icons/icon_3.png, the
# original's ICONSKI face); nothing is downloaded or committed.
set -e
cd "$(dirname "$0")/.."
DATA=${XDG_DATA_HOME:-$HOME/.local/share}
DESKTOP="$DATA/applications/skifree.desktop"
ICON="$DATA/icons/hicolor/32x32/apps/skifree.png"

if [ "$1" = "--uninstall" ]; then
    rm -f "$DESKTOP" "$ICON"
    update-desktop-database "$DATA/applications" 2>/dev/null || true
    echo "removed $DESKTOP and $ICON"
    exit 0
fi

BIN=$(realpath "${1:-target/release/skifree}")
[ -x "$BIN" ] || { echo "no executable at $BIN (cargo build --release first)" >&2; exit 1; }
[ -f assets/icons/icon_3.png ] || { echo "assets missing: run tools/extract_assets.py first" >&2; exit 1; }

mkdir -p "$(dirname "$ICON")" "$(dirname "$DESKTOP")"
cp assets/icons/icon_3.png "$ICON"
cat > "$DESKTOP" <<ENTRY
[Desktop Entry]
Type=Application
Name=SkiFree
Comment=A faithful port of SkiFree
Exec="$BIN"
Path=$(pwd)
Icon=skifree
Terminal=false
Categories=Game;
StartupWMClass=skifree
ENTRY
update-desktop-database "$DATA/applications" 2>/dev/null || true
gtk-update-icon-cache -q "$DATA/icons/hicolor" 2>/dev/null || true
echo "installed $DESKTOP and $ICON"
