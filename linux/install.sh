#!/bin/sh
# Install MO4 for the current user: binary to ~/.local/bin, a menu entry and icons.
#   ./install.sh          install (from a source checkout: builds first if needed)
#   ./install.sh --udev   also install the udev rule for the USB probe (asks for sudo)
set -eu
here=$(cd "$(dirname "$0")" && pwd)
if [ -x "$here/../mo4" ]; then
    # The release package: the binary and icon sit beside this folder.
    bin="$here/../mo4"
    icon="$here/../mo4.png"
else
    # A source checkout: build if needed.
    bin="$here/../target/release/mo4"
    icon="$here/../assets/mo4.png"
    if [ ! -x "$bin" ]; then
        echo "Building..."
        (cd "$here/.." && cargo build --release)
    fi
fi

icons="$HOME/.local/share/icons/hicolor"
mkdir -p "$HOME/.local/bin" "$HOME/.local/share/applications" "$icons/256x256/apps"
install -m 755 "$bin" "$HOME/.local/bin/mo4"
install -m 644 "$icon" "$icons/256x256/apps/mo4.png"
# The flat icon for menu and panel sizes (the machined one blurs below 64 px).
for s in 16 32 48; do
    if [ -f "$here/icons/mo4-$s.png" ]; then
        mkdir -p "$icons/${s}x$s/apps"
        install -m 644 "$here/icons/mo4-$s.png" "$icons/${s}x$s/apps/mo4.png"
    fi
done
touch "$icons"
command -v gtk-update-icon-cache >/dev/null && gtk-update-icon-cache -q -t -f "$icons" 2>/dev/null || true
sed "s|^Exec=.*|Exec=$HOME/.local/bin/mo4|" "$here/mo4.desktop" > "$HOME/.local/share/applications/mo4.desktop"
command -v update-desktop-database >/dev/null && update-desktop-database "$HOME/.local/share/applications" || true
command -v kbuildsycoca6 >/dev/null && kbuildsycoca6 >/dev/null 2>&1 || true
echo "Installed $HOME/.local/bin/mo4 and a menu entry."

if [ "${1:-}" = "--udev" ]; then
    sudo install -m 644 "$here/70-mo4.rules" /etc/udev/rules.d/70-mo4.rules
    sudo udevadm control --reload
    sudo udevadm trigger --subsystem-match=hidraw --action=change
    echo "Installed the udev rule. If the headphones were already plugged in, replug the cable."
fi
