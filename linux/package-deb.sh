#!/bin/sh
# Build a .deb from the release build: the program, its menu entry and icons.
#   cargo build --release && linux/package-deb.sh [output-dir]
set -eu
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
bin="$root/target/release/mo4"
if [ ! -x "$bin" ]; then
    echo "Build first: cargo build --release" >&2
    exit 1
fi
out=${1:-"$root/target"}
version=$(grep -m1 '^version' "$root/Cargo.toml" | cut -d'"' -f2)
arch=$(dpkg --print-architecture)
# The newest glibc symbol the program asks for is the oldest glibc it runs on.
glibc=$(objdump -T "$bin" | grep -o 'GLIBC_[0-9.]*' | cut -d_ -f2 | sort -uV | tail -1)

stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
install -Dm755 "$bin" "$stage/usr/bin/mo4"
install -Dm644 "$here/mo4.desktop" "$stage/usr/share/applications/mo4.desktop"
install -Dm644 "$root/assets/mo4.png" "$stage/usr/share/icons/hicolor/256x256/apps/mo4.png"
for s in 16 32 48; do
    install -Dm644 "$here/icons/mo4-$s.png" "$stage/usr/share/icons/hicolor/${s}x$s/apps/mo4.png"
done
install -Dm644 "$root/README.md" "$stage/usr/share/doc/mo4/README.md"
install -Dm644 "$root/docs/PROTOCOL.md" "$stage/usr/share/doc/mo4/PROTOCOL.md"
size=$(du -sk "$stage" | cut -f1)

mkdir -p "$stage/DEBIAN"
cat > "$stage/DEBIAN/control" <<CONTROL
Package: mo4
Version: $version
Section: sound
Priority: optional
Architecture: $arch
Maintainer: innerpulse <noreply@innerpulse.net>
Installed-Size: $size
Depends: libc6 (>= $glibc), libgcc-s1
Recommends: bluez, libxkbcommon0, libwayland-client0, libegl1, libgl1
Homepage: https://github.com/innerpulsenet/mo4
Description: Control for Sennheiser MOMENTUM 4 headphones
 Noise control, the five-band equalizer and the settings that otherwise
 only the phone app reaches, over Bluetooth. A window and a command line.
CONTROL

mkdir -p "$out"
deb="$out/mo4_${version}_${arch}.deb"
dpkg-deb --root-owner-group --build "$stage" "$deb" >/dev/null
echo "$deb"
