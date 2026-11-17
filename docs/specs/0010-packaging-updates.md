# 0010. Packaging, signing, updates, and release automation

**Date**: 2026-11-17
**Status**: In Progress

## Summary

This feature makes UniDB shippable. A reproducible `.deb` builds from the same container used for GUI verification, with a desktop entry, icon, AppStream metadata, and documented runtime dependencies. An AppImage script and a Flatpak manifest skeleton are provided for other distributions. A GitHub Actions workflow builds, tests, packages, generates an SBOM and checksums, signs when a key secret exists, and creates a release on a tag. The application gains an update notice: a Settings row that fetches a release feed, compares versions, and tells the user when a newer build exists, without downloading or installing anything by itself.

## Context

The app runs from `cargo run` in the verification container and has no installable artifact, no release process, and no way to tell a user that a newer build exists. Spec 0001 commits to `.deb`, AppImage, signed releases, an update notice, and an SBOM. The repository already has CI for fmt, clippy, tests, and the GUI build, plus a pinned container, so packaging can reuse the same toolchain instead of inventing a second build path.

## Requirements

**User stories**
- As a maintainer, I build an installable `.deb` with one command from a clean checkout.
- As a distribution maintainer, I can read exact runtime dependencies and install files from the package metadata.
- As a user, I install the package, launch UniDB from the application grid, and see the right icon and name.
- As a user, I open Settings and check for updates; the app tells me if a newer release exists.
- As a maintainer, tagging a release builds, tests, packages, checksums, and signs artifacts automatically.

**Acceptance criteria**
- **AC-1**: `scripts/package-deb.sh` builds a release binary in the pinned container and writes `target/release/unidb_<version>_amd64.deb`.
- **AC-2**: the package installs `/usr/bin/unidb`, a `.desktop` entry, a scalable icon, and AppStream metainfo, with `Depends` naming GTK4, libadwaita, and GtkSourceView 5.
- **AC-3**: the built package is verified in the script: `dpkg-deb -I`, `dpkg-deb -c`, extraction, and `usr/bin/unidb --version` printing the package version.
- **AC-4**: `scripts/generate-sbom.sh` produces a CycloneDX JSON SBOM listing every workspace and dependency component with versions.
- **AC-5**: `.github/workflows/release.yml` runs on `v*` tags: gates, release build, package, SBOM, SHA-256 checksums, optional detached signing when a key secret is present, and a GitHub release with the artifacts.
- **AC-6**: an AppImage build script and a Flatpak manifest skeleton exist with pinned tool versions and documented network requirements, marked unverified in this environment.
- **AC-7**: the Settings page shows the running version, a Check for updates action enabled by `UNIDB_UPDATE_FEED`, and a result line; no update is downloaded or installed.
- **AC-8**: update feed parsing and version comparison are pure functions with tests: newer, equal, older, malformed feed, and a local HTTP fetch.
- **AC-9**: packaging and update behavior is documented in `docs/packaging.md`, including signing and rollback.
- **AC-10**: the package verification output and the Settings updates surface are captured as evidence.

## Decision

- **`.deb`**: built with host `dpkg-deb --build --root-owner-group` from a staged tree, using the release binary from `scripts/gtk-cargo.sh build --release --features gui`. A hand written `DEBIAN/control` is the whole packaging metadata; no new build tool is introduced. Version comes from `[workspace.package] version`.
- **Assets**: an editable SVG icon, a desktop entry with `StartupWMClass` and database/SQL keywords, and AppStream metainfo with a 0.1.0 release entry live under `assets/` and install into the standard hicolor, applications, and metainfo directories.
- **AppImage and Flatpak**: scripts with pinned linuxdeploy and linuxdeploy-plugin-gtk versions and a manifest skeleton. They need network access to fetch tools and the runtime, so they are documented and not executed in this environment; CI can run the AppImage job on demand.
- **Signing**: releases produce SHA-256 checksums always, and a detached GPG signature when `RELEASE_GPG_PRIVATE_KEY` is configured. `.deb` signing is documented with `dpkg-sig` as an optional step rather than a hard dependency.
- **SBOM**: `scripts/generate-sbom.py` reads `cargo metadata` and emits CycloneDX 1.5 JSON, so it works offline and in CI without extra tools. Licenses and purls are filled where the metadata has them.
- **Updates**: `unidb-app::update` parses a GitHub style release payload (`tag_name`, `html_url`, `published_at`), compares dotted numeric versions without a semver dependency, and fetches over HTTP with `ureq` on a blocking thread. The feed URL comes from `UNIDB_UPDATE_FEED`; without it the button is disabled and explains the setting. The check only reports; downloading stays a manual, documented action.
- **Release workflow**: GitHub Actions on Ubuntu 24.04 installs the GTK development packages, runs the same gates as CI, builds the package with the script, generates the SBOM and checksums, optionally signs, and creates the release with `gh release create` using the tag and generated notes. No secrets are required for an unsigned release.
- **Evidence**: the package script prints metadata, contents, and the extracted version into `docs/evidence/package-deb.txt`, and the Settings updates group is captured in the container screenshot flow.

## Build plan

1. Add the icon, desktop entry, AppStream metainfo, and package staging script, satisfies **AC-1**, **AC-2**
2. Add package verification output and build the `.deb`, satisfies **AC-3**
3. Add the CycloneDX SBOM generator, satisfies **AC-4**
4. Add the AppImage script and Flatpak manifest skeleton, satisfies **AC-6**
5. Add the release GitHub Actions workflow with signing hook, satisfies **AC-5**
6. Add the update feed module with parsing, comparison, and fetch plus tests, satisfies **AC-8**
7. Add the Settings updates group and wire the check action, satisfies **AC-7**
8. Document packaging, signing, updates, and rollback, satisfies **AC-9**
9. Capture package and update evidence, satisfies **AC-10**

## Consequences

**Positive**
- The project becomes installable and releasable with one command and one tag.
- Dependencies are explicit in the package metadata, so distro integration is honest.
- The SBOM and checksums make each release auditable.

**Negative / tradeoffs**
- The `.deb` targets amd64 only in this version; arm64 needs a matrix entry.
- AppImage and Flatpak artifacts are scripts and skeletons here, not produced artifacts in this environment.
- The update check is a notice only; there is no in-app download or signature verification of the feed beyond TLS.

**Neutral**
- Packaging reuses the pinned container, so release and verification builds cannot drift.
- The desktop entry and icon also improve the development run when installed by a packager.

## Follow-up

- [ ] arm64 and universal builds in the release matrix.
- [ ] Automatic `.deb` repository publishing with signed `Release` files.
- [ ] In-app download with signature verification and rollback.
- [ ] Flatpak submission to Flathub.

## References

**Project sources**
- Spec `0001`, release and update commitments, pinned GTK floor.
- Spec `0004`, Settings surface this feature extends.
- Spec `0009`, settings persistence used for the update preference.
- Installed skill `.agents/skills/developing-gtk-apps/`, Flatpak and packaging guidance.
- `docs/development.md`, the container and gate contract reused for release builds.

**Practices & standards**
- One build path for development, verification, and release.
- Checksums and SBOMs produced by the release job, signatures optional over a documented key.
- Update feed consumed read only, with an explicit environment gate.
