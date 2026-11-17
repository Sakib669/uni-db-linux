# Packaging, signing, and updates

UniDB ships as a native Linux application. This page covers how a release is
built, how the update notice works, and what a maintainer needs for a signed
release. The feature decision is in
[spec 0010](specs/0010-packaging-updates.md).

## Build an installable package

The `.deb` is built from the same pinned container used for GUI verification, so
release and verification builds cannot drift.

```
./scripts/package-deb.sh
```

The script:

1. builds `target/release/unidb` with the `gui` feature inside `unidb-gtk-base`,
2. stages the binary, desktop entry, scalable icon, and AppStream metadata,
3. writes `target/release/unidb_<version>_amd64.deb` with
   `dpkg-deb --build --root-owner-group`,
4. verifies the package by printing its metadata, listing its contents,
   extracting it, and running the extracted binary with `--version` inside the
   container, and
5. writes the proof to `docs/evidence/package-deb.txt`.

In CI, set `UNIDB_BUILD_LOCAL=1` to build with the runner toolchain instead of
Docker.

Install and remove it with the normal tools:

```
sudo apt install ./target/release/unidb_0.1.0_amd64.deb
sudo apt remove unidb
```

Runtime dependencies are declared in `DEBIAN/control`: GTK 4.14, libadwaita
1.5, GtkSourceView 5, glib, and libc. The package targets amd64 in this version.

## Other formats

- AppImage: `scripts/package-appimage.sh` builds an AppImage with pinned
  linuxdeploy and the GTK plugin. It downloads tools on first use, so it needs
  network access and is not part of the offline verification run.
- Flatpak: `flatpak/io.github.sakib669.UniDB.yml` is a manifest skeleton for
  the GNOME 47 runtime. It builds the app from this checkout and is ready to
  submit to Flathub once signed releases exist.

## Software bill of materials

```
./scripts/generate-sbom.sh
```

This writes `target/sbom.cdx.json`, a CycloneDX 1.5 document built from
`cargo metadata`, so it works offline and lists every workspace and dependency
component with a purl and version. The current workspace produces 502
components. Release jobs attach the SBOM next to the package.

## Signing and release automation

`.github/workflows/release.yml` runs on `v*` tags:

1. installs the GTK development packages on Ubuntu 24.04,
2. runs formatting, clippy, and the test suite,
3. builds and verifies the `.deb`,
4. generates the SBOM and `SHA256SUMS`,
5. imports `RELEASE_GPG_PRIVATE_KEY` when the secret exists and writes detached
   ASCII armored signatures next to the artifacts,
6. creates the GitHub release with the package, SBOM, checksums, and
   signatures.

Without the signing secret the release is still produced, but unsigned. Verify
a download with:

```
sha256sum -c SHA256SUMS
gpg --verify unidb_0.1.0_amd64.deb.asc unidb_0.1.0_amd64.deb
```

The releases page is the single source of truth for the update notice.

## Update notice

Settings has a Check for updates action. It is disabled unless
`UNIDB_UPDATE_FEED` points at a release feed, for example a GitHub releases API
URL:

```
UNIDB_UPDATE_FEED=https://api.github.com/repos/Sakib669/uni-db-linux/releases/latest unidb
```

UniDB fetches the feed over HTTP on a blocking thread, parses `tag_name`,
`html_url`, and `published_at`, and compares the version with the running build.
If a newer release exists, the Settings row and a toast show its version and
link. The check never downloads or installs anything, and no request is made
when the variable is unset.

## Rollback

Keep the previous package file and checksum from the release page. Roll back
with:

```
sudo apt install ./unidb_<previous>_amd64.deb
```

The app store database is independent of the package version. Backups are
written next to `unidb.db` under the user data directory before migrations, so a
downgrade can restore a backup copy if a future migration is incompatible.

## What is not covered yet

- arm64 packages and a multi architecture release matrix.
- A signed apt repository instead of a single `.deb` download.
- In app downloading and installing of updates.
- Distribution packages beyond the Flatpak manifest skeleton.
