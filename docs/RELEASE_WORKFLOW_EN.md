# rDeckForge release workflow

Use only sanitized test templates and content for a public release. Never commit real customer documents, logos, credentials, or generated business files.

1. Check that `README.md`, `docs/`, the template protocol, and the CLI contract agree.
2. Run script tests, the web build, and Rust checks; validate template rendering locally when needed.
3. Run `npm run release:preflight` to check version, platform, and output directories before a distribution build.
4. Inspect installers, CLI and PPTX renderer sidecars, checksums, and release notes.
5. Create a Draft Release first, then review filenames, version, screenshots, and test data before publishing.

The release workflow never uploads private template packs or copies business documents into the app or GitHub Release.
