# Contributing

Thank you for helping improve rDeckForge. Read the [public repository boundary](PUBLIC_REPOSITORY.md) before opening a pull request.

## Development setup

```bash
npm run web:install
npm run rust-check
npm run web:build
npm run pptx:build
npm run test:scripts
npm --prefix web run test
cargo test --workspace
```

These checks do not require private template packs or business documents. Keep pull requests focused and update the relevant documentation or changelog when a public CLI, schema, template, or rendering contract changes.

Never commit real company, hospital, client, course, or personal templates; source documents; logos; credentials; private paths; generated Office files; or local visual QA captures. Use synthetic examples and `example.com`-style placeholders.

Release packaging and tags are maintained according to [RELEASE.md](RELEASE.md).
