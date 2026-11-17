#!/usr/bin/env bash
# Builds an AppImage with linuxdeploy and the GTK plugin. The tools are pinned
# and downloaded on first use, so this script needs network access. It is not
# executed in the offline verification environment; CI can run it on demand.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
TOOLS="$ROOT/target/appimage-tools"
APPDIR="$ROOT/target/UniDB.AppDir"
LINUXDEPLOY_VERSION="1-alpha-20250213-2"
GTK_PLUGIN_VERSION="1-alpha-20240109-1"

mkdir -p "$TOOLS"
if [ ! -x "$TOOLS/linuxdeploy" ]; then
    curl -fL -o "$TOOLS/linuxdeploy" \
        "https://github.com/linuxdeploy/linuxdeploy/releases/download/${LINUXDEPLOY_VERSION}/linuxdeploy-x86_64.AppImage"
    chmod +x "$TOOLS/linuxdeploy"
fi
if [ ! -x "$TOOLS/linuxdeploy-plugin-gtk.sh" ]; then
    curl -fL -o "$TOOLS/linuxdeploy-plugin-gtk.sh" \
        "https://raw.githubusercontent.com/linuxdeploy/linuxdeploy-plugin-gtk/${GTK_PLUGIN_VERSION}/linuxdeploy-plugin-gtk.sh"
    chmod +x "$TOOLS/linuxdeploy-plugin-gtk.sh"
fi

./scripts/gtk-cargo.sh build --release --features gui

rm -rf "$APPDIR"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/usr/share/applications" \
         "$APPDIR/usr/share/icons/hicolor/scalable/apps" \
         "$APPDIR/usr/share/metainfo"
install -m 0755 target/release/unidb "$APPDIR/usr/bin/unidb"
install -m 0644 assets/io.github.sakib669.UniDB.desktop "$APPDIR/usr/share/applications/"
install -m 0644 assets/io.github.sakib669.UniDB.svg "$APPDIR/usr/share/icons/hicolor/scalable/apps/"
install -m 0644 assets/io.github.sakib669.UniDB.metainfo.xml "$APPDIR/usr/share/metainfo/"

export DEPLOY_GTK_VERSION=4
export OUTPUT="UniDB-${VERSION}-x86_64.AppImage"
"$TOOLS/linuxdeploy" --appdir "$APPDIR" --output appimage \
    --desktop-file "$APPDIR/usr/share/applications/io.github.sakib669.UniDB.desktop" \
    --icon-file "$APPDIR/usr/share/icons/hicolor/scalable/apps/io.github.sakib669.UniDB.svg" \
    --plugin gtk

mv "$OUTPUT" "$ROOT/target/$OUTPUT" 2>/dev/null || true
echo "AppImage written: $ROOT/target/${OUTPUT}"
