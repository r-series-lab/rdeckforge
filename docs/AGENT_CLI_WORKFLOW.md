# Agent CLI Workflow

This is the stable rDeckForge loop for skills, external agents, and UI automation. Use the CLI as the source of truth; do not automate the desktop UI when a CLI command exists.

## 1. Check The CLI

```bash
rdeckforge info --json
rdeckforge capabilities --json
```

Stop if either command fails or returns non-JSON output.

## 2. Prepare The AI Handoff

```bash
rdeckforge workflow prepare \
  --prompt-pack /absolute/prompt-pack \
  --brief /absolute/brief.md \
  --template /absolute/template-pack \
  --prompt-out /absolute/ai-handoff-prompt.md \
  --contract-out /absolute/template-contract.json \
  --skeleton-out /absolute/content.skeleton.json \
  --json
```

Give the generated prompt to the external AI. The AI must return only content files, such as JSON, Markdown, and referenced local assets. It must not generate Office XML, slide coordinates, template files, or final Office documents.

## 3. Validate And Render In One Step

Choose the template by document role before rendering. Related templates can share one `familyId`,
but a PPTX `slides` template should render slides, a DOCX `lesson_plan` or `document` template
should render documents, and an XLSX `assessment` or `workbook` template should render spreadsheets.
Do not treat template families as cross-format conversion.

If the AI output is loosely shaped, let `workflow run` normalize it before validation. The normalized
JSON is saved beside the output by default, or to `--normalize-out` when supplied.

```bash
rdeckforge workflow run \
  --template /absolute/template-pack \
  --content /absolute/ai-output/content.json \
  --recipe teaching_deck \
  --normalize \
  --out /absolute/output.pptx \
  --validation-out /absolute/validation-report.json \
  --json
```

Use the standalone `content normalize` command when you want to inspect or edit the normalized JSON
before rendering.

`--input` and `--ai-output` are accepted aliases for `--content`.

On success, read:

- `data.stage`: `rendered`
- `data.validation.acceptanceSummary`
- `data.repairHints`
- `data.render.outputFile`
- `data.render.outputIntegrity`

On validation failure, the command exits non-zero with:

- `error.code`: `content_validation_failed`
- `data.stage`: `validate`
- `data.validation.acceptanceSummary.canRender`: `false`
- `data.repairHints`: actionable agent records

## 4. Route Repair Hints

Each `repairHints[]` item is stable JSON:

```json
{
  "code": "asset_file_not_found",
  "target": "asset",
  "severity": "error",
  "blocking": true,
  "path": "binding:hero_image",
  "sourcePath": "binding:hero_image",
  "message": "page 1 binding 'hero_image' image file not found: missing.png",
  "suggestedAction": "fix_asset_reference"
}
```

Route by `target` first:

- `content`: edit or regenerate the AI output content.
- `template`: repair the external template pack or its manifest.
- `binding`: align template binding paths with the content shape.
- `asset`: copy, relink, or rewrite local asset references.
- `output`: ask the caller for a writable output path.

Use `blocking=true` as the stop signal. Warnings can be reported to the user while still allowing render when `acceptanceSummary.canRender=true`.

Use `code` and `suggestedAction` for automation; do not parse `message` except for display.

## 5. Stop Conditions

The agent may finish when:

- `ok=true`
- `data.stage=rendered`
- `data.validation.acceptanceSummary.canRender=true`
- `data.render.outputFile` points to an existing Office file
- `data.render.outputIntegrity.status=passed`

If three repair attempts produce the same blocking `code` and `sourcePath`, stop and report the repeated blocker instead of looping.

## 6. Template Authoring Skill Notes

For `rdeckforge-template-author`, keep private packs outside the repository and skill directory. A template pack is not ready until `workflow run` succeeds against at least one representative sample and the output passes package integrity checks.
