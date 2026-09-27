# Implementation Notes

## Current Scope

- Rust core and CLI contract are implemented.
- Portable `.rdeckpack` export/import is implemented in the shared Rust core, CLI, and Tauri command
  surface. Imported packs are validated before being moved into the external template root and linked.
- Per-template runtime readiness diagnostics are implemented across the shared Rust core, CLI, and Tauri
  command surface. They check declarative engines, sidecars, script/pipeline entries, runtime commands,
  and template health without executing the renderer.
- Template manifests can declare local dependency files, external command probes, platform filters, and
  required environment variables. Readiness evaluates them without installing packages or returning
  environment-variable values.
- Release bundles include the native `rdeckforge` CLI beside the standalone PPTX sidecar. The CLI
  resolves that sibling renderer without Node.js, and `scripts/install-cli.mjs` installs both into a
  user-local bin directory, with a `~/.local/bin/rdeckforge` command link on macOS/Linux and a
  `%LOCALAPPDATA%\rDeckForge\bin` installation on Windows.
- The release wrapper preserves Tauri's normal macOS DMG flow and recovers a fully prepared read/write
  image when the upstream Finder layout script exits during volume detach. Recovery retries detach,
  converts to the same compressed UDZO format, verifies the checksum, and still fails for compile,
  signing, or incomplete-bundle errors.
- CLI JSON mode now wraps parser errors, help, version, validation failures, and successful commands
  in the same stable `ok + command + data/error` envelope with family-standard exit codes.
- Tauri shell command wiring is implemented.
- React workbench shell is implemented with local UI primitives.
- The main Generate workflow is format-aware: linked PPTX, DOCX, and XLSX templates share one
  four-step template/content/output/preflight flow and dispatch through accepted Office rendering.
- Prompt packs can build AI handoff prompts from a brief, prompt pack, template manifest, and effective output schema. The app does not run inference.
- PPTX Node renderer can render a real `.pptx` from `plannedPages` with named text/list bindings.
- Template manifest supports `pageTemplates + deckRecipes` for PPTX, `blockTemplates + documentRecipes` for DOCX, and `sheetTemplates + workbookRecipes` for XLSX.
- Template manifest recognizes `templateType` values `declarative`, `script`, and `hybrid`. Script and
  hybrid packs can declare `renderer` and `pipeline` metadata, pure script packs do not require an
  Office `entry`, and Python, Node.js, Shell, PowerShell, or explicit command adapters can run through
  `render office`.
- Legacy `layouts` is read only for compatibility warnings.
- Template manifest supports `input.formats`, `input.schema`, `input.schemaId`, and `input.mdProfile` so each private template pack can own its content protocol.
- Template manifest supports portable `tests[]` cases with pack-local inputs and Office package,
  output-size, slide-count, sheet-count, and required-ZIP-entry assertions. The shared executor runs
  through CLI or Tauri without adding render history, and CLI assertion failures retain the complete
  report with exit code 1.
- Template self-tests support whitespace-insensitive `requiredText` and `forbiddenText` assertions
  over visible PPTX slide/chart text, DOCX document/header/footer text, and XLSX worksheet/shared-string
  text. Image OCR is intentionally outside the deterministic test protocol.
- Template manifest supports template-local `input.authoring.instructions` and `input.authoring.examples`; validation keeps paths inside the external pack, while contracts and Prompt builds embed the declared text for external AI handoff.
- Template manifest supports `input.profile` as a built-in protocol shortcut for `teaching_deck_v1`, `design_doc_v1`, and `feature_assessment_v1`; template-local `input.schema` still takes priority.
- Template validation includes a read-only health check for Office template targets: PPTX source slide shape names, DOCX placeholders, and XLSX workbook sheets, named ranges, and A1 cell references.
- Office inspection can generate draft manifests from existing files: PPTX source slides and named shapes, DOCX `{{placeholder}}` tokens, and XLSX worksheets plus named ranges.
- PPTX/DOCX/XLSX draft-pack creation copies a private Office file into an external template-pack directory, preserves its source name as manifest identity, generates `template.manifest.json`, validates it, and exposes the same flow through Tauri.
- Content inputs can be single JSON/Markdown files or content pack directories containing `content.json`, `content.md`, or `outline.md` plus local resources.
- Markdown profiles include `teaching_outline_v1` for teaching decks and `design_doc_v1` for engineering/design DOCX documents.
- Rust expands `deckRecipes` into `plannedPages` before renderer execution, including `repeat.chunk` pagination for repeated collections, `variants` selection based on the current page data, and `overflow.split` page splitting for long nested lists.
- Content workspace validation accepts JSON/MD content, optional template pack, and optional recipe, then returns input metadata, `renderFormat`, `selectedRecipe`, PPTX `plannedPages`, binding warnings, asset warnings, and an `acceptanceSummary` before rendering. DOCX/XLSX validation resolves their document/workbook recipe without requiring PPTX planned pages.
- Accepted Office rendering re-runs content workspace validation for declarative packs, requires `acceptanceSummary.canRender`, and dispatches to PPTX/DOCX/XLSX from the template format through one app-facing command. Script and hybrid-renderer packs use the same command but validate the source file/format/schema before invoking their declared renderer and pipeline scripts. Declarative and hybrid packs without `renderer` can still run `pipeline` after built-in Office rendering.
- Direct and accepted Office rendering stage output in a same-directory transaction, validate every ZIP/XML part plus content-type and relationship references, and only then replace the destination. Failed generation preserves any existing destination, history is written after commit, and successful results expose `outputIntegrity`.
- Content workspace validation can also run without a template by passing a built-in `profile`, which enables JSON/MD protocol checks before choosing an external template.
- Non-dry-run CLI rendering writes a RenderJob, invokes the local Node sidecar, and normalizes PPTX package relationships after generation.
- PPTX rendering supports named text/list placeholders, local image resource replacement through content-pack paths, SVG-to-PNG rasterization for PowerPoint compatibility, table placeholder filling with layout/style options, generated PNG chart images for `chartMode: "image"`, and editable PowerPoint chart replacement for `chartMode: "native"`.
- DOCX export writes a minimal OpenXML teaching handout from the shared content structure when no template is supplied.
- DOCX export can convert standalone Markdown design documents into DOCX without a template.
- DOCX template rendering can replace manifest-declared text, list, table, image, and semantic `documentBlocks` placeholders in linked `.docx` template packs. Block replacements preserve placeholder paragraph/run styling, semantic design-document blocks map to template-owned Word style IDs, and table placeholders inside prototype Word tables copy table/row/cell styling while expanding data rows.
- XLSX export can convert standalone assessment JSON with `detailed_rows` and `summary` into a styled feature assessment workbook without a template.
- XLSX template rendering can write manifest-declared cells or named ranges, expand list/table bindings from a starting cell, preserve declared object-column order and labels, clone separate header/body prototype styles, and handle default or prefixed SpreadsheetML namespaces in linked `.xlsx` template packs.
- Diagnostic XLSX export writes a two-sheet content and validation report for review workflows.

## Template Model

`pageTemplates` describe reusable single-page PPTX templates. A page template points at one source slide and declares bindings to named shapes.

`deckRecipes` describe complete PPT structures. A recipe is an ordered list of steps that either use one data object or repeat over a collection.

`blockTemplates` and `documentRecipes` reserve the same structure for DOCX: a document recipe composes reusable placeholder/bookmark/content-control blocks.

`sheetTemplates` and `workbookRecipes` reserve the same structure for XLSX: a workbook recipe composes reusable sheet templates with cell or named-range bindings.

Example:

```json
{
  "pageTemplates": {
    "chapter_content": {
      "sourceSlide": 4,
      "bindings": {
        "title": {
          "shapeName": "ph:title",
          "dataPath": "$.contentTitle",
          "type": "text"
        }
      }
    }
  },
  "deckRecipes": {
    "teaching_deck": [
      { "use": "cover", "data": "$" },
      { "use": "chapter_content", "repeat": "$.chapters" }
    ]
  }
}
```

## Private Template Rule

Do not place real customer, hospital, company, or friend-provided template files in this repository.

Use private linked folders outside the repo, for example:

```text
~/Documents/rdeckforge-templates/my-template/
```

The app stores paths and metadata only.

## Demo Renderer Smoke Test

The demo template is generated from neutral shapes and does not contain private company assets. It includes text/list/image/table/chart-image/native-chart placeholders:

```bash
npm --prefix ./renderers/pptx-node run build
npm --prefix ./renderers/pptx-node run create-demo-template
cargo run -- --json render pptx --template examples/templates/demo-medical-teaching-v1 --recipe teaching_deck --input examples/content-packs/demo-teaching-with-assets --out target/demo-output.pptx
```

The sidecar returns `cleanup.visibleSlideCount` and `cleanup.slidePartCount`; both should match the planned page count for the demo deck.

The cleanup step removes unused source slide relationships, removes orphan notes parts, drops invalid top-level slide layout relationships, removes stale SVG extension nodes after SVG assets are rasterized to PNG, renames generated `slideN.xml` / `notesSlideN.xml` parts back to contiguous `1..N` names, updates `docProps/app.xml` slide/note/title counts, and removes stale chart/workbook parts that are no longer referenced after native chart replacement. This avoids Office repair prompts caused by stale OpenXML package relationships or mismatched presentation metadata.

`template validate` warns for any template pack path under the app repo, including the neutral demo pack. Private template packs should live outside the repo.
