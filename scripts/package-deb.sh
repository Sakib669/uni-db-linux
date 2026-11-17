#!/usr/bin/env bash
# Builds the release binary in the pinned GTK container and packages a .deb.
# The package is verified by reading its metadata, listing its contents,
# extracting it, and running the extracted binary with --version.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
if [ -z "$VERSION" ]; then
    echo "FAIL: could not read the workspace version from Cargo.toml"
    exit 1
fi

if [ "${UNIDB_BUILD_LOCAL:-0}" = "1" ]; then
    echo "Building UniDB $VERSION release binary with the local toolchain..."
    cargo build --release --features gui
else
    echo "Building UniDB $VERSION release binary in the GTK container..."
    ./scripts/gtk-cargo.sh build --release --features gui
fi

STAGE="$ROOT/target/deb-root"
rm -rf "$STAGE"
mkdir -p "$STAGE/usr/bin" \
         "$STAGE/usr/share/applications" \
         "$STAGE/usr/share/icons/hicolor/scalable/apps" \
         "$STAGE/usr/share/metainfo" \
         "$STAGE/DEBIAN"

install -m 0755 "$ROOT/target/release/unidb" "$STAGE/usr/bin/unidb"
install -m 0644 "$ROOT/assets/io.github.sakib669.UniDB.desktop" "$STAGE/usr/share/applications/"
install -m 0644 "$ROOT/assets/io.github.sakib669.UniDB.svg" "$STAGE/usr/share/icons/hicolor/scalable/apps/"
install -m 0644 "$ROOT/assets/io.github.sakib669.UniDB.metainfo.xml" "$STAGE/usr/share/metainfo/"

INSTALLED_SIZE="$(du -sk "$STAGE/usr" | cut -f1)"
cat > "$STAGE/DEBIAN/control" <<CONTROL
Package: unidb
Version: $VERSION
Section: database
Priority: optional
Architecture: amd64
Maintainer: Sakib <sakib@example.com>
Installed-Size: $INSTALLED_SIZE
Depends: libc6, libgtk-4-1 (>= 4.14), libadwaita-1-0 (>= 1.5), libgtksourceview-5-0, libglib2.0-0
Recommends: xdg-utils
Homepage: https://github.com/Sakib669/uni-db-linux
Description: Native database studio for Linux
 UniDB is a GTK4 database studio for SQLite, PostgreSQL, CockroachDB,
 MySQL, MariaDB, Redis, MongoDB, Cassandra, and MSSQL. It includes a
 navigator, a SQL editor with results, a table designer, CSV and JSON
 transfer, history, saved queries, and a security report.
CONTROL

DEB="$ROOT/target/release/unidb_${VERSION}_amd64.deb"
dpkg-deb --build --root-owner-group -Zxz "$STAGE" "$DEB" >/dev/null

EVIDENCE="$ROOT/docs/evidence/package-deb.txt"
{
    echo "UniDB $VERSION .deb verification"
    echo "date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "package: $(basename "$DEB")"
    echo
    echo "== dpkg-deb -I =="
    dpkg-deb -I "$DEB"
    echo
    echo "== dpkg-deb -c =="
    dpkg-deb -c "$DEB"
    echo
    echo "== extracted binary =="
    EXTRACT="$ROOT/target/deb-extract"
    rm -rf "$EXTRACT"
    dpkg-deb -x "$DEB" "$EXTRACT"
    if [ "${UNIDB_BUILD_LOCAL:-0}" = "1" ]; then
        "$EXTRACT/usr/bin/unidb" --version
        echo
        echo "== shared libraries =="
        ldd "$EXTRACT/usr/bin/unidb" | head -20
    else
        # The host has the GTK runtime but not the development libraries the
        # binary links against, so run it inside the pinned container.
        docker run --rm -v "$ROOT":/app unidb-gtk-base \
            /app/target/deb-extract/usr/bin/unidb --version
        echo
        echo "== shared libraries =="
        docker run --rm -v "$ROOT":/app unidb-gtk-base \
            bash -lc 'ldd /app/target/deb-extract/usr/bin/unidb | head -20'
    fi
} | tee "$EVIDENCE"

echo
echo "Package written: $DEB"
echo "Evidence written: $EVIDENCE"
