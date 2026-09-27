# rDeckForge

rDeckForge is a local Tauri document workbench that validates structured content and renders it into Office files. It connects private template packs with an external AI handoff, local acceptance checks, and safe PPTX, DOCX, and XLSX generation.

The app does not run model inference. It builds a prompt from a template contract and brief, then validates the returned `content.json` or `content.md` locally before rendering.

## Highlights

- Link and inspect private template packs without bundling them in the app.
- Build handoff prompts from template contracts, content skeletons, and briefs.
- Validate fields, assets, bindings, and profiles before rendering.
- Render PPTX, DOCX, XLSX, or script-driven outputs through a transactional path.
- Keep task stages and render history locally, with a stable JSON CLI.

## Safe sample data

The public screenshot uses `Demo Template`, `Example Brief`, and `sample-output`. It contains no real templates, logos, customer documents, or credentials.

## CLI

```sh
rdeckforge --json info
rdeckforge --json capabilities
rdeckforge --json template list
rdeckforge --json content validate --input /absolute/content.json --template /absolute/template-pack --recipe teaching_deck --out /absolute/report.json
```

Keep real template packs and business content outside the repository and public issue attachments.

## Development

```sh
npm run test:scripts
npm run web:build
npm run rust-check
```
