# Template Manifest

`rDeckForge` template packs use structural composition. PPTX packs compose pages, DOCX packs
compose document blocks, and XLSX packs compose sheets. Packs can be `declarative`, `script`, or
`hybrid`.

## Concepts

- `pageTemplates`: reusable single-page PPTX templates. Each page template maps to one source slide and describes how named shapes receive data.
- `deckRecipes`: complete PPTX deck structures. Each recipe is an ordered list of page-template uses.
- `blockTemplates`: reusable DOCX document blocks. Each block describes placeholders, bookmarks, or content controls.
- `documentRecipes`: complete DOCX document structures composed from block templates.
- `sheetTemplates`: reusable XLSX sheet templates. Each sheet describes cells or named ranges.
- `workbookRecipes`: complete XLSX workbook structures composed from sheet templates.
- `PageInstance`: created at render time when a recipe step applies a page template to data.
- `familyId`: optional business-protocol family shared by related PPTX, DOCX, and XLSX templates.
- `role`: optional document role inside a family, such as `slides`, `document`, `lesson_plan`,
  `workbook`, `assessment`, or `report`.
- `input`: the template pack's accepted content formats, optional built-in profile, and schema.
- `preview`: optional cover and representative slide images for the desktop template library.
- `templateType`: `declarative` for built-in binding renderers, `script` for external template
  scripts, or `hybrid` when a pack combines declared structure with script steps.
- `renderer`: primary external renderer metadata, such as a Python script entry.
- `pipeline`: optional ordered steps for validation, rendering, and output validation.

`input.profile` points to a built-in content protocol. It can fill default `formats`, `schemaId`, built-in schema validation, and `mdProfile`. `input.schema` points to a template-local JSON Schema file and takes priority when present. `template validate` compiles the effective schema, `content validate` returns any content mismatch as `schemaErrors`, and render commands for PPTX/DOCX/XLSX block output when schema errors exist.

`familyId` and `role` describe what a template is for; they do not imply cross-format conversion.
Related templates can share one `familyId` and content protocol while keeping separate formats and
roles:

```json
{
  "familyId": "nursing_training_v1",
  "role": "slides",
  "format": "pptx",
  "input": { "schemaId": "nursing_training_v1" }
}
```

A PPTX template still renders PPTX, a DOCX template still renders DOCX, and an XLSX template still
renders XLSX. Agents should use `role` to choose the right output template for the user's goal.

`input.authoring` lets a private template carry its external-AI writing contract without running AI in the app:

```json
{
  "input": {
    "formats": ["md"],
    "schemaId": "chapter_markdown_v1",
    "authoring": {
      "instructions": ["prompts/content-authoring.md", "references/structure.md"],
      "examples": ["examples/content-example.md"]
    }
  }
}
```

All declared files must be UTF-8 text files addressed by relative paths inside the template pack.
`template validate` checks them, `template contract` embeds both path and content, and `prompt build`
adds dedicated authoring instructions/examples before the output boundary and contract sections.

Content can be supplied as a single file or as a content pack directory. A content pack directory is resolved by looking for `content.json`, `content.md`, then `outline.md`; relative asset paths are resolved from that directory.

`preview` gives the app safe visual hints for template selection without opening Office files:

```json
{
  "preview": {
    "cover": "previews/cover.png",
    "slides": [
      { "title": "封面", "image": "previews/cover.png" },
      { "title": "章节内容", "image": "previews/content.png" }
    ]
  }
}
```

Preview paths must stay relative to the template pack. Missing preview files produce warnings but do
not block rendering.

`template validate` also performs a read-only template health check:

- PPTX: verifies each `pageTemplate.sourceSlide` can be inspected and each declared `shapeName` exists on that source slide.
- DOCX: verifies declared `placeholder` values exist in `word/document.xml`.
- XLSX: verifies `sourceSheet`, `namedRange`, and single-cell A1 `cell` bindings against `xl/workbook.xml`.

`template contract` exports the AI-facing contract for a template pack:

```bash
rdeckforge template contract \
  --template /path/private-template-pack \
  --out /path/template-contract.json \
  --json
```

The contract is intentionally content-only. It tells an external AI or script which output shape to
produce, which PPTX page templates / DOCX blocks / XLSX sheets are available, and which data fields
each binding reads. For PPTX, the preferred output is explicit `pages[]`: the AI expands every final
slide itself, while `deckRecipes` remain available under optional template expansion for packs that
want built-in low-code pagination or conditional pages.
Each binding also includes `acceptedShape`, `sampleValue`, and `authorHint`, so UI panels and CLI
agents can present a usable field guide without hard-coding template-specific help text.

`template content-skeleton` turns that contract into a starter content file:

```bash
rdeckforge template content-skeleton \
  --template /path/private-template-pack \
  --mode minimal \
  --out /path/content.skeleton.json \
  --json
```

Use `--mode minimal` for a small renderable sample with required fields only. Use `--mode all` when
the file is mainly for AI handoff and should show every PPTX `pageTemplate` or every selected
DOCX/XLSX recipe binding. Optional image bindings are omitted unless required because they need real
content-pack assets.

`template create-script-adapter` creates a small external template pack for an existing script
renderer:

```bash
rdeckforge template create-script-adapter \
  --script /path/external/build.py \
  --out-dir /path/outside-repo/script-template-pack \
  --id my-script-template \
  --name "My Script Template" \
  --format pptx \
  --input-format md \
  --json
```

The adapter copies the selected script into `renderer/` and passes `{input} --output {output}`. This
keeps exported `.rdeckpack` files portable without copying private assets into the app bundle. Pass
`--link-script` when the adapter should retain an absolute reference to a separately managed script.

`template inspect-pptx` can bootstrap a manifest from an existing PPTX:

```bash
rdeckforge template inspect-pptx \
  --input /path/template.pptx \
  --out /path/template.manifest.json \
  --json
```

It extracts slide parts, shape names, and likely binding types, then writes `pageTemplates` plus a
simple `draft_deck` recipe. The result is a draft: review `input.profile`, `dataPath`, repeated page
structure, and binding types before using it as a private template pack.

For app-style onboarding from a private Office file, use the matching `create-*-draft` command. It
copies the source into an external draft pack as `template.<format>`, writes
`template.manifest.json`, and runs the same template validation:

```bash
rdeckforge template create-pptx-draft \
  --input /path/private-template.pptx \
  --out-dir /path/outside-repo/private-template-pack \
  --json
```

DOCX and XLSX use the same workflow:

```bash
rdeckforge template create-docx-draft --input /path/template.docx --out-dir /path/docx-pack --json
rdeckforge template create-xlsx-draft --input /path/template.xlsx --out-dir /path/xlsx-pack --json
```

## Portable Template Archives

A complete template pack can be transported as a `.rdeckpack` archive. The archive is a standard ZIP
whose pack root contains exactly one `template.manifest.json`; all paths referenced by the manifest must
remain inside that root.

```bash
rdeckforge template export \
  --template /path/external-template-pack \
  --out /path/external-template-pack.rdeckpack \
  --json

rdeckforge template import \
  --archive /path/external-template-pack.rdeckpack \
  --out-dir /path/rdeckforge-templates \
  --json
```

Import is transactional: files are extracted to a temporary directory, archive paths and limits are
checked, the resulting template pack is validated, and only then is it moved into the external template
root and linked. Use `--force` only when an existing directory with the same `templateId` should be
replaced.

## Runtime Readiness

Templates can declare non-renderer dependencies without asking the app to install them:

```json
{
  "dependencies": {
    "files": [
      {
        "path": "requirements.txt",
        "kind": "python_requirements",
        "required": true
      },
      {
        "path": "package.json",
        "kind": "node_package",
        "required": false
      }
    ],
    "commands": [
      {
        "id": "libreoffice",
        "command": "soffice",
        "args": ["--version"],
        "required": false,
        "platforms": ["macos", "windows", "linux"]
      }
    ],
    "environment": [
      {
        "name": "TEMPLATE_DATA_TOKEN",
        "required": true,
        "description": "Template-specific data source token"
      }
    ]
  }
}
```

Dependency file paths must stay inside the template pack. Supported file kinds are
`python_requirements`, `node_package`, `config`, `resource`, and `other`. Platform filters accept
`macos`, `windows`, and `linux`. A dependency is required by default; optional missing dependencies
produce warnings instead of blocking rendering.

Environment checks expose only whether a variable is set and non-empty. Values are never serialized,
logged, or copied into template archives. rDeckForge does not run `pip install`, `npm install`, or any
other dependency installation command.

Manifest validation proves that the pack structure can be parsed. Runtime readiness additionally proves
that the current machine has the files and executables required to render it:

```bash
rdeckforge template readiness --template /path/template-pack --json
```

The report checks the Office entry, declarative engine, PPTX sidecar, renderer and pipeline script types,
entry files, runtime commands, and Office structure health. `canRender: false` means a required dependency
is missing. A `warn` readiness with `canRender: true` means rendering is allowed but template warnings
should be reviewed.

The DOCX scanner auto-binds contiguous `{{placeholder}}` tokens and reports tokens split across Word
runs for manual cleanup. The XLSX scanner creates one `sheetTemplate` per worksheet and auto-binds
supported named ranges; a workbook without named ranges still receives a valid structural draft with
a warning to add `cell` or `namedRange` bindings.

Supported Markdown profiles:

- `teaching_outline_v1`: convert an outline-style teaching Markdown file into the teaching deck structure.
- `design_doc_v1`: convert an engineering/design Markdown document into a structured design document with heading, paragraph, list, quote, and code blocks.

Built-in content profiles:

- `teaching_deck_v1`: JSON or Markdown teaching content for PPTX/DOCX.
- `design_doc_v1`: JSON or Markdown design-document content for DOCX.
- `feature_assessment_v1`: JSON feature assessment rows and summary notes for XLSX.

## Render Plan

Before calling the PPTX renderer, Rust builds `plannedPages`. The primary AI-output path is explicit
`pages[]` content: each page item chooses a page template and provides the exact data for that page.

```json
{
  "schemaVersion": "1.0",
  "documentType": "pptx_pages",
  "pages": [
    { "use": "cover", "data": { "title": "封面标题" } },
    { "use": "chapter_content", "data": { "contentTitle": "章节标题", "items": [] } }
  ]
}
```

When no `--recipe` is supplied and the content has `pages[]`, each item becomes one `plannedPage`.
The optional `deckRecipe` layer remains available for templates that want built-in expansion.

For example, this recipe step:

```json
{ "use": "chapter_content", "repeat": "$.chapters" }
```

becomes one `PageInstance` per chapter:

```json
{
  "pageIndex": 4,
  "pageTemplate": "chapter_content",
  "sourceSlide": 4,
  "dataPath": "$.chapters[0]",
  "repeatIndex": 0
}
```

The Node PPTX renderer loops over `plannedPages`, copies each `sourceSlide`, resolves bindings from each page's `dataPath`, and writes the final PPTX.

For directory, agenda, or summary pages, a repeated collection can be chunked across multiple
instances of the same page template:

```json
{ "use": "toc_page", "repeat": "$.chapters", "chunk": { "size": 4, "as": "chapters" } }
```

With 6 chapters, this creates 2 planned pages. Each planned page receives a generated `dataValue`
instead of a single source object:

```json
{
  "chapters": [{ "title": "第一部分" }, { "title": "第二部分" }],
  "items": [{ "title": "第一部分" }, { "title": "第二部分" }],
  "pageIndex": 1,
  "pageCount": 2,
  "startNumber": 1,
  "endNumber": 2,
  "root": { "title": "课程标题" }
}
```

The page template can bind a list shape to `$.chapters`, a page label to `$.pageIndex`, and the
deck title to `$.root.title`. This is the first low-code layout primitive for "same template,
different data volume" PPT generation.

Recipe steps can be grouped with `steps`. A group step does not render a page itself; it creates a
new data scope for its child steps. This keeps related generated pages together:

```json
{
  "repeat": "$.chapters",
  "steps": [
    { "use": "section_divider", "data": "$" },
    {
      "use": "chapter_content",
      "data": "$",
      "overflow": { "strategy": "split", "path": "$.items", "maxItems": 4 }
    }
  ]
}
```

In this example, `$` inside each child step is the current chapter. The output order is chapter 1
divider, chapter 1 content pages, chapter 2 divider, chapter 2 content pages, and so on. This is the
declarative equivalent of a template script that loops through chapters and emits a small page group
per chapter.

Recipe steps can be conditional with `when`. A string condition is a truthy data path:

```json
{ "use": "assessment_table", "data": "$", "when": "$.assessmentTable" }
```

An object condition supports existence, truthiness, and count rules:

```json
{ "use": "summary", "data": "$", "when": { "path": "$.summary", "countMin": 1 } }
```

For repeated steps, `when` is checked against each repeated item or generated chunk. For group steps,
`when` is checked against the selected group data before child steps run.

Recipe steps can also choose a page-template variant from the current page data. Variants are
checked in order, and the default `use` template is used when none match:

```json
{
  "use": "toc_page",
  "repeat": "$.chapters",
  "chunk": { "size": 4, "as": "chapters" },
  "variants": [
    { "use": "toc_compact", "when": { "path": "$.chapters", "countMax": 2 } }
  ]
}
```

In this example, a 6-chapter deck creates 2 directory pages: the first page uses `toc_page` for 4
chapters, and the second page uses `toc_compact` for the remaining 2 chapters.

For long nested lists, a recipe step can split one data object across multiple page instances:

```json
{
  "use": "chapter_content",
  "repeat": "$.chapters",
  "overflow": { "strategy": "split", "path": "$.items", "maxItems": 4 }
}
```

If a chapter has 9 items, this creates 3 `chapter_content` pages. Each generated page keeps the
chapter title and receives only the current `items` slice. Metadata such as `overflowIndex` and
`overflowCount` is also available for optional page labels.

Variants are evaluated after `chunk` or `overflow` has created the current page data. That means a
long chapter can use the regular content layout for full 4-item pages and automatically switch to a
compact page template for the final 1-2 item overflow page.

## PPTX Shape

```json
{
  "schemaVersion": "1.0",
  "templateId": "my-private-template",
  "name": "My Private Template",
  "format": "pptx",
  "entry": "template.pptx",
  "input": {
    "profile": "teaching_deck_v1",
    "formats": ["json", "md"],
    "schema": "schemas/input.schema.json",
    "schemaId": "my_template_input_v1",
    "mdProfile": "outline_v1"
  },
  "contentSchema": "teaching_deck_v1",
  "renderMode": "pptx_automizer",
  "pageTemplates": {
    "cover": {
      "sourceSlide": 1,
      "bindings": {
        "title": {
          "shapeName": "ph:title",
          "dataPath": "$.title",
          "type": "text",
          "required": true
        },
        "coverImage": {
          "shapeName": "ph:coverImage",
          "dataPath": "$.coverImage",
          "type": "image",
          "fit": "cover"
        },
        "assessmentTable": {
          "shapeName": "ph:assessmentTable",
          "dataPath": "$.assessmentTable",
          "type": "table",
          "tableOptions": {
            "adjustWidth": true,
            "adjustHeight": true,
            "columnWidths": [1400000, 2200000, 2600000],
            "headerStyle": {
              "bold": true,
              "color": "FFFFFF",
              "background": "1E5B7A"
            },
            "bodyStyle": {
              "fontSize": 11,
              "color": "263238",
              "borderColor": "C8D7DF"
            }
          }
        },
        "scoreTrend": {
          "shapeName": "ph:scoreChart",
          "dataPath": "$.charts.scoreTrend",
          "type": "chart",
          "chartMode": "image"
        },
        "editableScoreTrend": {
          "shapeName": "ph:nativeScoreChart",
          "dataPath": "$.charts.scoreTrend",
          "type": "chart",
          "chartMode": "native",
          "chartOptions": {
            "title": true,
            "axisRange": { "min": 0, "max": 100, "majorUnit": 20 }
          }
        }
      }
    },
    "chapter_content": {
      "sourceSlide": 4,
      "bindings": {
        "title": {
          "shapeName": "ph:title",
          "dataPath": "$.contentTitle",
          "type": "text"
        },
        "items": {
          "shapeName": "ph:items",
          "dataPath": "$.items",
          "type": "list",
          "maxItems": 4
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

Image-like bindings use the same named-shape pattern as text. Put an image placeholder in PowerPoint, name it in the Selection Pane, then bind it:

```json
{
  "shapeName": "ph:coverImage",
  "dataPath": "$.coverImage",
  "type": "image",
  "fit": "cover"
}
```

The content value may be a path string or an object with `src`:

```json
{
  "coverImage": {
    "src": "assets/cover.svg",
    "alt": "Cover image"
  }
}
```

Table bindings target real PowerPoint table placeholders. Content may use `columns + rows`, a two-dimensional array, or `{ header, body }`:

```json
{
  "assessmentTable": {
    "columns": ["项目", "观察要点", "教学提示"],
    "rows": [
      ["意识状态", "观察意识水平", "用案例引导观察"],
      ["生命体征", "记录趋势变化", "强调异常上报"]
    ]
  }
}
```

`tableOptions` can tune generated table layout while preserving the template's overall placement. The first pass supports table width/height fitting, column widths, row heights, whole-table style ids/attributes, and header/body row style defaults.

Image-backed chart bindings can point to a local chart image or accept structured data that is rendered into a local PNG:

```json
{
  "charts": {
    "scoreTrend": {
      "type": "line",
      "labels": ["课前", "讲授后", "复盘后"],
      "series": [{ "name": "平均分", "values": [68, 78, 91] }]
    }
  }
}
```

Native editable chart bindings target a real PowerPoint chart placeholder. The renderer updates chart XML plus the embedded workbook so the result can still be edited in PowerPoint:

```json
{
  "shapeName": "ph:nativeScoreChart",
  "dataPath": "$.charts.scoreTrend",
  "type": "chart",
  "chartMode": "native",
  "chartOptions": {
    "title": true,
    "removeLegend": false,
    "axisRange": { "min": 0, "max": 100, "majorUnit": 20 }
  }
}
```

Currently supported PPTX binding types are `text`, `list`, `image`, `backgroundImage`, `table`, and `chart`. Chart bindings support `chartMode: "image"` for image/PNG output and `chartMode: "native"` for editable PowerPoint charts. SVG image assets are accepted in content packs, but they are rasterized to PNG during PPTX rendering for compatibility.

## DOCX Shape

DOCX template rendering starts with placeholder replacement. Use `blockTemplates` to bind data to
placeholders such as `{{title}}`, then compose those blocks with `documentRecipes`.

Supported placeholder binding types:

- `text`: replace the placeholder text in place.
- `list`: replace the placeholder paragraph with simple bullet paragraphs.
- `table`: replace the placeholder paragraph with a Word table.
- `image`: replace the placeholder paragraph with a drawing image and copy the asset into `word/media/`.
- `documentBlocks`: render semantic `design_doc_v1` blocks with Word styles owned by the template.

For `list`, `table`, and `image`, place the placeholder in a paragraph styled like the desired
output. The renderer preserves that paragraph's `<w:pPr>` properties and the placeholder run's
`<w:rPr>` properties when generating the replacement block.

For richer DOCX tables, place the `table` placeholder inside a prototype Word table instead of a
standalone paragraph. The renderer replaces the entire prototype table, preserves `<w:tblPr>`,
`<w:tblGrid>`, `<w:trPr>`, `<w:tcPr>`, and cell paragraph/run styling, then fills the content rows.
When two prototype rows are present, the first row styles the generated header row and the second
row styles generated body rows.

For image bindings, `dataPath` may point to a string path or an object with `src`. Relative paths are
resolved from the content pack directory. `widthInches` and `heightInches` can be declared on the
binding, or on the image object in the content.

For `documentBlocks`, set `dataPath` to an array such as `$.blocks`. The default style IDs are
`Heading1` through `Heading6`, `Normal`, `ListBullet`, `ListNumber`, `Quote`, and `Code`.
`blockStyles` can override roles such as `heading2`, `paragraph`, `listItem`, `orderedItem`, `quote`,
and `code`; nested list roles use keys such as `listItem2` and `orderedItem2`.
Blocks may use `text`, `content`, `body`, or `title` for textual content. `callout` uses Word's
`IntenseQuote` style by default, while `divider` and `pageBreak` create structural document breaks.

```json
{
  "schemaVersion": "1.0",
  "templateId": "my-private-docx-template",
  "name": "My Private DOCX Template",
  "format": "docx",
  "entry": "template.docx",
  "input": {
    "profile": "teaching_deck_v1",
    "formats": ["json", "md"],
    "schema": "schemas/input.schema.json",
    "schemaId": "doc_input_v1"
  },
  "blockTemplates": {
    "summary_block": {
      "bindings": {
        "title": {
          "placeholder": "{{title}}",
          "dataPath": "$.title",
          "type": "text",
          "required": true
        },
        "goals": {
          "placeholder": "{{goals}}",
          "dataPath": "$.goals",
          "type": "list"
        },
        "assessmentTable": {
          "placeholder": "{{assessmentTable}}",
          "dataPath": "$.assessmentTable",
          "type": "table"
        },
        "coverImage": {
          "placeholder": "{{coverImage}}",
          "dataPath": "$.coverImage",
          "type": "image",
          "widthInches": 4.5,
          "heightInches": 2.8
        },
        "blocks": {
          "placeholder": "{{blocks}}",
          "dataPath": "$.blocks",
          "type": "documentBlocks",
          "blockStyles": {
            "heading2": "DesignHeading2",
            "paragraph": "DesignBody",
            "listItem": "DesignBullet",
            "code": "DesignCode"
          }
        }
      }
    }
  },
  "documentRecipes": {
    "standard_document": [
      { "use": "summary_block", "data": "$" }
    ]
  }
}
```

## XLSX Shape

XLSX template rendering uses sheet-level templates and workbook recipes. Direct `cell` bindings or
Excel `namedRange` bindings write into the linked workbook while preserving untouched workbook parts.
When the target cell already exists, rDeckForge preserves its cell style attributes. When a `list`
or `table` expands into new rows, it clones row height/custom row attributes and cell style
attributes from the declared starting row/cells where possible.

Supported cell binding types:

- `text`: write one inline string cell.
- `list`: write array items vertically from the starting cell.
- `table`: write table rows from the starting cell, expanding right and down.

For arrays of objects, `tableColumns` fixes column order and supplies display headers independently
from JSON property names. Set `tableBodyRowOffset` to `1` when the starting row is a header
prototype and the following row is the body prototype; generated body rows then clone the second
row's height and cell styles.

```json
{
  "schemaVersion": "1.0",
  "templateId": "my-private-xlsx-template",
  "name": "My Private XLSX Template",
  "format": "xlsx",
  "entry": "template.xlsx",
  "input": {
    "profile": "feature_assessment_v1",
    "formats": ["json"],
    "schema": "schemas/input.schema.json",
    "schemaId": "workbook_input_v1"
  },
  "sheetTemplates": {
    "report_sheet": {
      "sourceSheet": "Report",
      "bindings": {
        "title": {
          "namedRange": "TitleCell",
          "dataPath": "$.title",
          "type": "text",
          "required": true
        },
        "goals": {
          "cell": "B4",
          "dataPath": "$.goals",
          "type": "list"
        },
        "assessmentTable": {
          "namedRange": "AssessmentStart",
          "dataPath": "$.assessmentTable",
          "type": "table",
          "tableBodyRowOffset": 1,
          "tableColumns": [
            { "key": "module", "header": "模块" },
            { "key": "feature", "header": "功能点" },
            { "key": "effort", "header": "工时(人天)" }
          ]
        }
      }
    }
  },
  "workbookRecipes": {
    "standard_workbook": [
      { "use": "report_sheet", "data": "$" }
    ]
  }
}
```

## Script And Hybrid Packs

Script packs are useful when the template itself is an algorithm: dynamic directories, page-family
selection, custom geometry, or PowerPoint-specific validation. The app still owns linking,
validation metadata, content selection, output paths, and result management; the template pack owns
the script, assets, and template-specific rules.

Minimal script pack shape:

```json
{
  "schemaVersion": "1.0",
  "templateId": "custom-script-presentation",
  "name": "自定义脚本演示模板",
  "format": "pptx",
  "templateType": "script",
  "input": {
    "formats": ["md"],
    "schemaId": "chapter_markdown_v1"
  },
  "renderer": {
    "type": "script.python",
    "runtime": "python3",
    "entry": "scripts/build_from_chapter_md.py",
    "input": "chapterMarkdown",
    "output": "pptx",
    "args": ["{input}", "--output", "{output}"]
  },
  "pipeline": [
    {
      "id": "validate_output",
      "type": "script.python",
      "runtime": "python3",
      "entry": "scripts/validate_pptx_in_powerpoint.py",
      "args": ["{output}"]
    }
  ]
}
```

Supported script types are:

- `script.python`: defaults to `python3`
- `script.node`: defaults to `node`
- `script.shell`: defaults to `sh` on macOS/Linux and Windows PowerShell on Windows
- `script.powershell`: defaults to `powershell.exe` on Windows and `pwsh` elsewhere
- `script.command`: requires an explicit `runtime` and invokes it as an interpreter for `entry`

All script types support `{input}`, `{output}`, `{templateDir}`, `{outputDir}`, `{outputStem}`,
and `{outputBaseName}` in `renderer.args` and `pipeline[].args`. If `renderer.args` is omitted,
rDeckForge calls the script as `{input} --output {output}`. If a pipeline step omits `args`, it
receives `{output}`.

The adapter command infers the script type from the file extension or accepts an explicit type:

```bash
rdeckforge template create-script-adapter \
  --script /path/build.mjs \
  --out-dir /path/node-template-pack \
  --id node-template \
  --name "Node Template" \
  --renderer-type script.node \
  --json
```

The adapter copies the script into the pack by default. Add `--link-script` to keep an external
absolute script reference instead.

Hybrid packs can keep `pageTemplates`, `deckRecipes`, or other declarative structure while using a
script for rendering or post-processing. If `renderer` is present, `render office` invokes the
script renderer and then runs `pipeline`. If `renderer` is omitted, `render office` uses the built-in
PPTX/DOCX/XLSX renderer and then runs `pipeline`. In that case `entry` is still required when the
declarative part needs a source Office file.

## Template Self-tests

A template pack can carry deterministic smoke cases alongside its authoring examples:

```json
{
  "tests": [
    {
      "id": "smoke",
      "input": "examples/content-example.json",
      "recipe": "standard_workbook",
      "outputName": "assessment-smoke.xlsx",
      "assertions": {
        "validatePackage": true,
        "minSizeBytes": 1000,
        "minSheets": 2,
        "maxSheets": 2,
        "requiredZipEntries": ["xl/styles.xml"],
        "requiredText": ["功能点评估", "评估说明"],
        "forbiddenText": ["{{title}}"]
      }
    }
  ]
}
```

`input` and every required ZIP entry must be safe relative paths. `recipe` is optional and must
reference the matching declarative recipe when present. `outputName` is optional and must be a file
name rather than a path. Supported assertions are `validatePackage`, `minSizeBytes`, `minSlides`,
`maxSlides`, `minSheets`, `maxSheets`, `requiredZipEntries`, `requiredText`, and `forbiddenText`.
Office package validation defaults
to true. `requiredText` and `forbiddenText` inspect visible OOXML text with whitespace-insensitive
matching across split Office runs; they do not perform OCR on images. Script templates that
intentionally emit a non-Office fixture can set package validation to false, but cannot use text
assertions in that mode.

Run all cases or one named case with:

```bash
rdeckforge template test --template /path/template-pack --json
rdeckforge template test --template /path/template-pack --case smoke --out-dir /path/test-output --json
```

Self-tests first run template readiness, then render without adding normal render-history records.
Failed rendering or assertions produce a report with `passed: false`; the CLI returns exit code 1 and
keeps the report under the JSON error envelope's `data` field.

## Private Template Rule

Keep real templates outside this repository and outside the app bundle. Link PPTX/DOCX/XLSX
template-pack folders from a private local path.
