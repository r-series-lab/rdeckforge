# Release

rDeckForge releases are built by GitHub Actions from version tags. A tagged build runs the source checks, creates platform packages, uploads them as workflow artifacts, generates `SHA256SUMS.txt`, and creates a Draft prerelease.

## Targets

- macOS Apple Silicon: `aarch64-apple-darwin`
- macOS Intel: `x86_64-apple-darwin`
- Windows x64: `x86_64-pc-windows-msvc`

## Maintainer flow

1. Keep the application versions in the root package, Rust packages, Tauri configuration, lockfiles, and changelog aligned.
2. Run `npm run release:preflight` and the repository checks locally. Do not include private templates or generated Office files.
3. Push a new `vX.Y.Z` tag only after `main` contains the intended source commit and the tag does not already exist.
4. Review the GitHub Actions checks, package signatures, checksum manifest, release notes, and installation behavior.
5. Keep preview builds as Draft prereleases until signing, notarization, updater metadata, and distribution hosting are ready.

The release workflow reads signing credentials only from GitHub Actions secrets. They must never be written to the repository, logs, or local configuration files.
