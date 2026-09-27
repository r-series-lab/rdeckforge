use serde::Serialize;
use serde_json::{Value, json};
use std::{fs, path::Path};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateContractResult {
    pub template_dir: String,
    pub output_file: Option<String>,
    pub template_id: String,
    pub template_name: String,
    pub format: String,
    pub contract: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateContentSkeletonResult {
    pub template_dir: String,
    pub output_file: Option<String>,
    pub template_id: String,
    pub template_name: String,
    pub format: String,
    pub mode: String,
    pub recipe: Option<String>,
    pub warnings: Vec<String>,
    pub content: Value,
}

pub fn export_template_contract(
    template_dir: &Path,
    out_file: Option<&Path>,
) -> Result<TemplateContractResult, String> {
    let manifest = crate::template_manifest::load_manifest(template_dir)?;
    let validation = crate::template_manifest::validate_template_pack(template_dir)?;
    let input = crate::template_manifest::normalized_input_spec(&manifest);
    let authoring =
        crate::template_authoring::load_template_authoring_context(template_dir, &input)?;
    let template_type = crate::template_manifest::normalized_template_type(&manifest)?;
    let output_contract = if uses_script_renderer_contract(&manifest) {
        build_script_renderer_contract_value(&manifest, &input)
    } else {
        match manifest.format.as_str() {
            "pptx" => json!({
                "preferredMode": "explicit_pages",
                "pptxExplicitPages": build_explicit_pages_contract_value(&manifest),
                "optionalTemplateExpansion": {
                    "mode": "deck_recipe",
                    "description": "Optional template-managed expansion. Use only when the prompt intentionally asks the app to expand recipes.",
                    "deckRecipes": manifest.deck_recipes,
                }
            }),
            "docx" => json!({
                "preferredMode": "document_recipe",
                "docxDocument": build_document_contract_value(&manifest),
            }),
            "xlsx" => json!({
                "preferredMode": "workbook_recipe",
                "xlsxWorkbook": build_workbook_contract_value(&manifest),
            }),
            other => json!({
                "preferredMode": null,
                "unsupportedFormat": other,
            }),
        }
    };

    let contract = json!({
        "schemaVersion": "1.0",
        "contractType": "rdeckforge_template_contract",
        "aiOutputBoundary": [
            "Generate content/config JSON or Markdown only.",
            "Do not generate Office XML, PowerPoint files, Word files, Excel files, slide coordinates, or template files.",
            "When preferredMode is explicit_pages, every pages[] item is one final output slide.",
            "Private template files stay outside the app bundle; this contract only describes fillable structure."
        ],
        "template": {
            "id": manifest.template_id,
            "name": manifest.name,
            "familyId": validation.family_id.clone(),
            "role": validation.role.clone(),
            "format": manifest.format,
            "templateType": template_type,
            "warnings": validation.warnings,
            "health": validation.health,
        },
        "input": {
            "formats": input.formats,
            "profile": input.profile,
            "schema": input.schema,
            "schemaId": input.schema_id,
            "mdProfile": input.md_profile,
            "authoring": authoring,
        },
        "aiOutput": output_contract,
    });

    if let Some(out_file) = out_file {
        if let Some(parent) = out_file.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create contract output directory: {err}"))?;
        }
        let raw = serde_json::to_string_pretty(&contract)
            .map_err(|err| format!("failed to serialize template contract: {err}"))?;
        fs::write(out_file, raw)
            .map_err(|err| format!("failed to write template contract: {err}"))?;
    }

    Ok(TemplateContractResult {
        template_dir: template_dir.display().to_string(),
        output_file: out_file.map(|path| path.display().to_string()),
        template_id: validation.template_id,
        template_name: validation.name,
        format: validation.format,
        contract,
    })
}

pub fn export_content_skeleton(
    template_dir: &Path,
    out_file: Option<&Path>,
    mode: Option<&str>,
    recipe: Option<&str>,
) -> Result<TemplateContentSkeletonResult, String> {
    let manifest = crate::template_manifest::load_manifest(template_dir)?;
    let validation = crate::template_manifest::validate_template_pack(template_dir)?;
    let mode = normalize_skeleton_mode(mode)?;
    let mut warnings = Vec::new();
    let (content, selected_recipe) = match manifest.format.as_str() {
        "pptx" => (
            build_pptx_content_skeleton(&manifest, &mode, &mut warnings)?,
            None,
        ),
        "docx" => build_docx_content_skeleton(&manifest, &mode, recipe, &mut warnings)?,
        "xlsx" => build_xlsx_content_skeleton(&manifest, &mode, recipe, &mut warnings)?,
        other => {
            return Err(format!(
                "content skeleton is not supported for template format '{other}'"
            ));
        }
    };

    if let Some(out_file) = out_file {
        if let Some(parent) = out_file.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create skeleton output directory: {err}"))?;
        }
        let raw = serde_json::to_string_pretty(&content)
            .map_err(|err| format!("failed to serialize content skeleton: {err}"))?;
        fs::write(out_file, raw)
            .map_err(|err| format!("failed to write content skeleton: {err}"))?;
    }

    Ok(TemplateContentSkeletonResult {
        template_dir: template_dir.display().to_string(),
        output_file: out_file.map(|path| path.display().to_string()),
        template_id: validation.template_id,
        template_name: validation.name,
        format: validation.format,
        mode,
        recipe: selected_recipe,
        warnings,
        content,
    })
}

fn uses_script_renderer_contract(manifest: &crate::template_manifest::TemplateManifest) -> bool {
    let has_renderer = manifest.renderer.is_some() || !manifest.pipeline.is_empty();
    has_renderer
        && manifest.page_templates.is_empty()
        && manifest.deck_recipes.is_empty()
        && manifest.block_templates.is_empty()
        && manifest.document_recipes.is_empty()
        && manifest.sheet_templates.is_empty()
        && manifest.workbook_recipes.is_empty()
}

fn build_script_renderer_contract_value(
    manifest: &crate::template_manifest::TemplateManifest,
    input: &crate::template_manifest::TemplateInputSpec,
) -> Value {
    let renderer = manifest.renderer.as_ref().map(|renderer| {
        json!({
            "type": renderer.renderer_type.clone(),
            "runtime": renderer.runtime.clone(),
            "entry": renderer.entry.clone(),
            "input": renderer.input.clone(),
            "output": renderer.output.clone(),
            "args": renderer.args.clone(),
        })
    });
    let pipeline = manifest
        .pipeline
        .iter()
        .map(|step| {
            json!({
                "id": step.id.clone(),
                "type": step.step_type.clone(),
                "runtime": step.runtime.clone(),
                "entry": step.entry.clone(),
                "args": step.args.clone(),
            })
        })
        .collect::<Vec<_>>();

    json!({
        "preferredMode": "script_renderer",
        "scriptRenderer": {
            "format": manifest.format.clone(),
            "inputFormats": input.formats.clone(),
            "schema": input.schema.clone(),
            "schemaId": input.schema_id.clone(),
            "mdProfile": input.md_profile.clone(),
            "contentShape": {
                "description": "Generate the JSON or Markdown input accepted by this script template. rDeckForge passes the content file to the renderer; the script owns pagination, layout, and Office output details."
            },
            "rules": [
                "Do not generate Office XML, PPTX, DOCX, or XLSX internals.",
                "Do not invent pageTemplates or binding ids for a pure script template.",
                "Follow the script template package's input format and prompt instructions."
            ],
            "renderer": renderer,
            "pipeline": pipeline
        }
    })
}

pub fn build_explicit_pages_contract_value(
    manifest: &crate::template_manifest::TemplateManifest,
) -> Value {
    let templates = manifest
        .page_templates
        .iter()
        .map(|(template_id, template)| {
            let bindings = template
                .bindings
                .iter()
                .map(|(binding_id, binding)| {
                    (
                        binding_id.clone(),
                        page_binding_contract_value(binding_id, binding),
                    )
                })
                .collect::<serde_json::Map<_, _>>();
            json!({
                "use": template_id,
                "sourceSlide": template.source_slide,
                "description": template.description,
                "data": "Provide an object matching these bindings. Binding dataPath values are resolved against this page data object.",
                "bindings": bindings,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "mode": "explicit_pages",
        "documentType": "pptx_pages",
        "contentShape": {
            "schemaVersion": "1.0",
            "documentType": "pptx_pages",
            "title": "Deck title",
            "pages": [
                { "use": "page_template_id", "data": {} }
            ]
        },
        "rules": [
            "Render pages in the exact array order.",
            "Choose use from pageTemplates[].use.",
            "Put all business content for a page under data.",
            "Do not rely on the app to infer chapters, numbering, splitting, or optional pages in explicit mode."
        ],
        "pageObject": {
            "use": "One of the page template ids below",
            "data": "Object consumed by the selected page template bindings"
        },
        "pageTemplates": templates
    })
}

fn normalize_skeleton_mode(mode: Option<&str>) -> Result<String, String> {
    let mode = mode.unwrap_or("minimal").trim().to_ascii_lowercase();
    match mode.as_str() {
        "minimal" | "all" => Ok(mode),
        other => Err(format!(
            "unsupported content skeleton mode '{other}'; expected minimal or all"
        )),
    }
}

fn build_pptx_content_skeleton(
    manifest: &crate::template_manifest::TemplateManifest,
    mode: &str,
    warnings: &mut Vec<String>,
) -> Result<Value, String> {
    if manifest.page_templates.is_empty() {
        return Err("PPTX template has no pageTemplates".to_string());
    }
    let page_template_ids = if mode == "all" {
        let mut templates = manifest
            .page_templates
            .iter()
            .map(|(id, template)| (id.clone(), template.source_slide))
            .collect::<Vec<_>>();
        templates.sort_by(|(left_id, left_slide), (right_id, right_slide)| {
            left_slide
                .cmp(right_slide)
                .then_with(|| left_id.cmp(right_id))
        });
        templates
            .into_iter()
            .map(|(id, _source_slide)| id)
            .collect::<Vec<_>>()
    } else {
        vec![preferred_pptx_page_template_id(manifest)?]
    };
    let pages = page_template_ids
        .into_iter()
        .map(|template_id| {
            let template = manifest
                .page_templates
                .get(&template_id)
                .ok_or_else(|| format!("missing pageTemplate '{template_id}'"))?;
            let data = skeleton_for_page_bindings(&template_id, &template.bindings, mode, warnings);
            Ok(json!({
                "use": template_id,
                "data": data,
            }))
        })
        .collect::<Result<Vec<_>, String>>()?;

    Ok(json!({
        "schemaVersion": "1.0",
        "documentType": "pptx_pages",
        "title": format!("{} content skeleton", manifest.name),
        "pages": pages,
    }))
}

fn preferred_pptx_page_template_id(
    manifest: &crate::template_manifest::TemplateManifest,
) -> Result<String, String> {
    for id in ["cover", "title", "opening", "summary", "closing"] {
        if manifest.page_templates.contains_key(id) {
            return Ok(id.to_string());
        }
    }
    manifest
        .page_templates
        .keys()
        .next()
        .cloned()
        .ok_or_else(|| "PPTX template has no pageTemplates".to_string())
}

fn skeleton_for_page_bindings(
    template_id: &str,
    bindings: &std::collections::BTreeMap<String, crate::template_manifest::PageBinding>,
    mode: &str,
    warnings: &mut Vec<String>,
) -> Value {
    let mut scope = json!({});
    for (binding_id, binding) in bindings {
        let binding_type = binding.binding_type.as_deref().unwrap_or("text");
        if should_skip_skeleton_binding(binding.required, binding_type, mode) {
            if matches!(binding_type, "image" | "backgroundImage") {
                warnings.push(format!(
                    "optional PPTX image binding '{template_id}.{binding_id}' is omitted from skeleton; fill it with a content-pack asset path when needed"
                ));
            }
            continue;
        }
        let sample = sample_value(
            binding_id,
            binding_type,
            binding.max_length,
            binding.max_items,
        );
        if matches!(binding_type, "image" | "backgroundImage") {
            warnings.push(format!(
                "PPTX image binding '{template_id}.{binding_id}' uses placeholder asset path 'assets/example.png'"
            ));
        }
        insert_sample_value(
            &mut scope,
            binding.data_path.as_deref().unwrap_or("$"),
            sample,
            warnings,
            &format!("PPTX binding '{template_id}.{binding_id}'"),
        );
    }
    scope
}

fn build_docx_content_skeleton(
    manifest: &crate::template_manifest::TemplateManifest,
    mode: &str,
    recipe: Option<&str>,
    warnings: &mut Vec<String>,
) -> Result<(Value, Option<String>), String> {
    let recipe_id = resolve_skeleton_recipe(recipe, &manifest.document_recipes, "document recipe")?;
    let steps = manifest
        .document_recipes
        .get(&recipe_id)
        .ok_or_else(|| format!("document recipe not found: {recipe_id}"))?;
    let mut root = json!({
        "schemaVersion": "1.0",
        "title": format!("{} content skeleton", manifest.name),
    });
    for step in steps {
        let block = manifest
            .block_templates
            .get(&step.use_template)
            .ok_or_else(|| format!("missing block template: {}", step.use_template))?;
        let scope =
            skeleton_for_document_bindings(&step.use_template, &block.bindings, mode, warnings);
        insert_step_scope(
            &mut root,
            step.data.as_deref(),
            step.repeat.as_deref(),
            scope,
        );
    }
    Ok((root, Some(recipe_id)))
}

fn skeleton_for_document_bindings(
    template_id: &str,
    bindings: &std::collections::BTreeMap<String, crate::template_manifest::DocumentBinding>,
    mode: &str,
    warnings: &mut Vec<String>,
) -> Value {
    let mut scope = json!({});
    for (binding_id, binding) in bindings {
        let binding_type = binding.binding_type.as_deref().unwrap_or("text");
        if should_skip_skeleton_binding(binding.required, binding_type, mode) {
            if binding_type == "image" {
                warnings.push(format!(
                    "optional DOCX image binding '{template_id}.{binding_id}' is omitted from skeleton; fill it with a content-pack asset path when needed"
                ));
            }
            continue;
        }
        let sample = sample_value(
            binding_id,
            binding_type,
            binding.max_length,
            binding.max_items,
        );
        if binding_type == "image" {
            warnings.push(format!(
                "DOCX image binding '{template_id}.{binding_id}' uses placeholder asset path 'assets/example.png'"
            ));
        }
        insert_sample_value(
            &mut scope,
            binding.data_path.as_deref().unwrap_or("$"),
            sample,
            warnings,
            &format!("DOCX binding '{template_id}.{binding_id}'"),
        );
    }
    scope
}

fn build_xlsx_content_skeleton(
    manifest: &crate::template_manifest::TemplateManifest,
    mode: &str,
    recipe: Option<&str>,
    warnings: &mut Vec<String>,
) -> Result<(Value, Option<String>), String> {
    let recipe_id = resolve_skeleton_recipe(recipe, &manifest.workbook_recipes, "workbook recipe")?;
    let steps = manifest
        .workbook_recipes
        .get(&recipe_id)
        .ok_or_else(|| format!("workbook recipe not found: {recipe_id}"))?;
    let mut root = json!({
        "schemaVersion": "1.0",
        "title": format!("{} content skeleton", manifest.name),
    });
    for step in steps {
        let sheet = manifest
            .sheet_templates
            .get(&step.use_template)
            .ok_or_else(|| format!("missing sheet template: {}", step.use_template))?;
        let scope =
            skeleton_for_sheet_bindings(&step.use_template, &sheet.bindings, mode, warnings);
        insert_step_scope(
            &mut root,
            step.data.as_deref(),
            step.repeat.as_deref(),
            scope,
        );
    }
    Ok((root, Some(recipe_id)))
}

fn skeleton_for_sheet_bindings(
    template_id: &str,
    bindings: &std::collections::BTreeMap<String, crate::template_manifest::SheetBinding>,
    mode: &str,
    warnings: &mut Vec<String>,
) -> Value {
    let mut scope = json!({});
    for (binding_id, binding) in bindings {
        let binding_type = binding.binding_type.as_deref().unwrap_or("cell");
        if should_skip_skeleton_binding(binding.required, binding_type, mode) {
            continue;
        }
        let sample = sample_value(
            binding_id,
            binding_type,
            binding.max_length,
            binding.max_items,
        );
        insert_sample_value(
            &mut scope,
            binding.data_path.as_deref().unwrap_or("$"),
            sample,
            warnings,
            &format!("XLSX binding '{template_id}.{binding_id}'"),
        );
    }
    scope
}

fn resolve_skeleton_recipe<T>(
    requested: Option<&str>,
    recipes: &std::collections::BTreeMap<String, T>,
    label: &str,
) -> Result<String, String> {
    if let Some(requested) = requested {
        if recipes.contains_key(requested) {
            return Ok(requested.to_string());
        }
        return Err(format!("{label} not found: {requested}"));
    }
    recipes
        .keys()
        .next()
        .cloned()
        .ok_or_else(|| format!("template has no {label}s"))
}

fn insert_step_scope(
    root: &mut Value,
    data_path: Option<&str>,
    repeat_path: Option<&str>,
    scope: Value,
) {
    if let Some(repeat_path) = repeat_path {
        insert_value_at_path(root, repeat_path, Value::Array(vec![scope]));
        return;
    }
    insert_value_at_path(root, data_path.unwrap_or("$"), scope);
}

fn should_skip_skeleton_binding(required: bool, binding_type: &str, mode: &str) -> bool {
    if required {
        return false;
    }
    if mode != "all" {
        return true;
    }
    matches!(binding_type, "image" | "backgroundImage")
}

fn sample_value(
    binding_id: &str,
    binding_type: &str,
    max_length: Option<usize>,
    max_items: Option<usize>,
) -> Value {
    match binding_type {
        "list" => {
            let count = max_items.unwrap_or(2).clamp(1, 2);
            Value::Array(
                (1..=count)
                    .map(|index| {
                        json!({
                            "heading": format!("{} item {}", binding_id, index),
                            "body": "Sample item detail"
                        })
                    })
                    .collect(),
            )
        }
        "table" => json!({
            "columns": ["Column A", "Column B"],
            "rows": [["Value A", "Value B"]]
        }),
        "documentBlocks" => json!([
            { "type": "heading", "level": 2, "text": "Sample section" },
            { "type": "paragraph", "text": "Sample paragraph content." },
            { "type": "listItem", "level": 0, "text": "Sample list item" }
        ]),
        "chart" => json!({
            "type": "line",
            "labels": ["A", "B"],
            "series": [
                { "name": "Series 1", "values": [1, 2] }
            ]
        }),
        "image" | "backgroundImage" => json!({ "src": "assets/example.png" }),
        _ => Value::String(sample_text(binding_id, max_length)),
    }
}

fn sample_text(binding_id: &str, max_length: Option<usize>) -> String {
    let text = format!("Sample {binding_id}");
    match max_length {
        Some(max_length) if max_length == 0 => String::new(),
        Some(max_length) if text.chars().count() > max_length => {
            text.chars().take(max_length).collect()
        }
        _ => text,
    }
}

fn insert_sample_value(
    scope: &mut Value,
    data_path: &str,
    sample: Value,
    warnings: &mut Vec<String>,
    label: &str,
) {
    let trimmed = data_path.trim();
    if is_quoted_literal(trimmed) {
        return;
    }
    if !trimmed.starts_with('$') {
        warnings.push(format!(
            "{label} uses non-standard dataPath '{data_path}'; sample value was skipped"
        ));
        return;
    }
    insert_value_at_path(scope, trimmed, sample);
}

fn insert_value_at_path(scope: &mut Value, data_path: &str, value: Value) {
    let trimmed = data_path.trim();
    if trimmed == "$" || !trimmed.starts_with("$.") {
        merge_scope_value(scope, value);
        return;
    }
    let segments = parse_skeleton_path_segments(trimmed);
    insert_path_segments(scope, &segments, value);
}

fn merge_scope_value(scope: &mut Value, value: Value) {
    match value {
        Value::Object(source) => {
            if let Value::Object(target) = scope {
                for (key, value) in source {
                    target.entry(key).or_insert(value);
                }
            } else {
                *scope = Value::Object(source);
            }
        }
        value => {
            if scope.as_object().is_some_and(serde_json::Map::is_empty) {
                *scope = value;
            }
        }
    }
}

#[derive(Debug)]
enum SkeletonPathSegment {
    Key(String),
    Index(usize),
}

fn parse_skeleton_path_segments(expression: &str) -> Vec<SkeletonPathSegment> {
    let path = expression.trim_start_matches("$.").trim();
    let bytes = path.as_bytes();
    let mut segments = Vec::new();
    let mut cursor = 0;
    while cursor < path.len() {
        if bytes[cursor] == b'.' {
            cursor += 1;
            continue;
        }
        if bytes[cursor] == b'[' {
            let Some(close_offset) = path[cursor..].find(']') else {
                break;
            };
            let close = cursor + close_offset;
            if let Ok(index) = path[cursor + 1..close].parse::<usize>() {
                segments.push(SkeletonPathSegment::Index(index));
            }
            cursor = close + 1;
            continue;
        }
        let start = cursor;
        while cursor < path.len() && bytes[cursor] != b'.' && bytes[cursor] != b'[' {
            cursor += 1;
        }
        if start < cursor {
            segments.push(SkeletonPathSegment::Key(path[start..cursor].to_string()));
        }
    }
    segments
}

fn insert_path_segments(current: &mut Value, segments: &[SkeletonPathSegment], value: Value) {
    let Some((first, rest)) = segments.split_first() else {
        merge_scope_value(current, value);
        return;
    };
    match first {
        SkeletonPathSegment::Key(key) => {
            if !current.is_object() {
                *current = json!({});
            }
            let object = current.as_object_mut().expect("current is object");
            let next = object.entry(key.clone()).or_insert_with(|| json!({}));
            insert_path_segments(next, rest, value);
        }
        SkeletonPathSegment::Index(index) => {
            if !current.is_array() {
                *current = Value::Array(Vec::new());
            }
            let array = current.as_array_mut().expect("current is array");
            while array.len() <= *index {
                array.push(json!({}));
            }
            insert_path_segments(&mut array[*index], rest, value);
        }
    }
}

fn is_quoted_literal(value: &str) -> bool {
    value.len() >= 2
        && ((value.starts_with('\'') && value.ends_with('\''))
            || (value.starts_with('"') && value.ends_with('"')))
}

fn build_document_contract_value(manifest: &crate::template_manifest::TemplateManifest) -> Value {
    let block_templates = manifest
        .block_templates
        .iter()
        .map(|(template_id, template)| {
            let bindings = template
                .bindings
                .iter()
                .map(|(binding_id, binding)| {
                    (
                        binding_id.clone(),
                        document_binding_contract_value(binding_id, binding),
                    )
                })
                .collect::<serde_json::Map<_, _>>();
            json!({
                "use": template_id,
                "description": template.description,
                "data": "Provide an object matching these bindings. Binding dataPath values are resolved against this block data object.",
                "bindings": bindings,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "mode": "document_recipe",
        "recipes": manifest.document_recipes,
        "blockTemplates": block_templates,
    })
}

fn build_workbook_contract_value(manifest: &crate::template_manifest::TemplateManifest) -> Value {
    let sheet_templates = manifest
        .sheet_templates
        .iter()
        .map(|(template_id, template)| {
            let bindings = template
                .bindings
                .iter()
                .map(|(binding_id, binding)| {
                    (
                        binding_id.clone(),
                        sheet_binding_contract_value(binding_id, binding),
                    )
                })
                .collect::<serde_json::Map<_, _>>();
            json!({
                "use": template_id,
                "sourceSheet": template.source_sheet,
                "sourceSheetIndex": template.source_sheet_index,
                "outputSheetName": template.output_sheet_name,
                "data": "Provide an object matching these bindings. Binding dataPath values are resolved against this sheet data object.",
                "bindings": bindings,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "mode": "workbook_recipe",
        "recipes": manifest.workbook_recipes,
        "sheetTemplates": sheet_templates,
    })
}

fn page_binding_contract_value(
    binding_id: &str,
    binding: &crate::template_manifest::PageBinding,
) -> Value {
    let binding_type = binding.binding_type.as_deref().unwrap_or("text");
    json!({
        "type": binding_type,
        "dataPath": binding.data_path.as_deref().unwrap_or("$"),
        "required": binding.required,
        "requiredBy": required_by_text(binding.required),
        "acceptedShape": accepted_shape_for_binding(binding_type),
        "sampleValue": sample_value(binding_id, binding_type, binding.max_length, binding.max_items),
        "authorHint": author_hint_for_binding(binding_type),
        "maxLength": binding.max_length,
        "maxItems": binding.max_items,
        "fit": binding.fit,
        "chartMode": binding.chart_mode,
    })
}

fn document_binding_contract_value(
    binding_id: &str,
    binding: &crate::template_manifest::DocumentBinding,
) -> Value {
    let binding_type = binding.binding_type.as_deref().unwrap_or("text");
    json!({
        "type": binding_type,
        "dataPath": binding.data_path.as_deref().unwrap_or("$"),
        "required": binding.required,
        "requiredBy": required_by_text(binding.required),
        "acceptedShape": accepted_shape_for_binding(binding_type),
        "sampleValue": sample_value(binding_id, binding_type, binding.max_length, binding.max_items),
        "authorHint": author_hint_for_binding(binding_type),
        "maxLength": binding.max_length,
        "maxItems": binding.max_items,
        "widthInches": binding.width_inches,
        "heightInches": binding.height_inches,
        "blockStyles": binding.block_styles,
    })
}

fn sheet_binding_contract_value(
    binding_id: &str,
    binding: &crate::template_manifest::SheetBinding,
) -> Value {
    let binding_type = binding.binding_type.as_deref().unwrap_or("cell");
    json!({
        "type": binding_type,
        "dataPath": binding.data_path.as_deref().unwrap_or("$"),
        "required": binding.required,
        "requiredBy": required_by_text(binding.required),
        "acceptedShape": accepted_shape_for_binding(binding_type),
        "sampleValue": sample_value(binding_id, binding_type, binding.max_length, binding.max_items),
        "authorHint": author_hint_for_binding(binding_type),
        "maxLength": binding.max_length,
        "maxItems": binding.max_items,
        "tableColumns": binding.table_columns,
        "tableBodyRowOffset": binding.table_body_row_offset,
    })
}

fn required_by_text(required: bool) -> &'static str {
    if required { "required" } else { "optional" }
}

fn accepted_shape_for_binding(binding_type: &str) -> &'static str {
    match binding_type {
        "list" => "array of strings or objects",
        "table" => "object with columns+rows, object with header/body, or two-dimensional array",
        "documentBlocks" => {
            "array of semantic blocks: heading, paragraph, listItem, orderedItem, quote, code, callout, divider, pageBreak"
        }
        "chart" => {
            "object with labels and series/values, or an image source when chartMode is image"
        }
        "image" | "backgroundImage" => "local asset path string or object with src",
        _ => "string, number, boolean, or object that can be rendered as text",
    }
}

fn author_hint_for_binding(binding_type: &str) -> &'static str {
    match binding_type {
        "list" => "Keep list items concise; respect maxItems when present.",
        "table" => "Keep column keys stable and provide row values in the same order.",
        "documentBlocks" => "Use semantic blocks for long DOCX content; do not include Word XML.",
        "chart" => "Prefer structured chart data so the renderer can validate labels and series.",
        "image" | "backgroundImage" => "Use a relative content-pack asset path whenever possible.",
        _ => "Write final user-facing text only; do not include template instructions.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_pptx_contract_with_explicit_pages() -> Result<(), String> {
        let result = export_template_contract(
            Path::new("examples/templates/demo-medical-teaching-v1"),
            None,
        )?;

        assert_eq!(result.format, "pptx");
        assert_eq!(
            result.contract["aiOutput"]["preferredMode"].as_str(),
            Some("explicit_pages")
        );
        assert_eq!(
            result.contract["aiOutput"]["pptxExplicitPages"]["documentType"].as_str(),
            Some("pptx_pages")
        );
        assert!(
            result.contract["aiOutput"]["pptxExplicitPages"]["pageTemplates"]
                .as_array()
                .is_some_and(|templates| templates
                    .iter()
                    .any(|template| { template["use"].as_str() == Some("cover") }))
        );
        let cover = result.contract["aiOutput"]["pptxExplicitPages"]["pageTemplates"]
            .as_array()
            .and_then(|templates| {
                templates
                    .iter()
                    .find(|template| template["use"].as_str() == Some("cover"))
            })
            .ok_or_else(|| "missing cover contract".to_string())?;
        let title = &cover["bindings"]["title"];
        assert_eq!(title["requiredBy"].as_str(), Some("required"));
        assert!(title["acceptedShape"].as_str().is_some());
        assert!(title["sampleValue"].as_str().is_some());
        assert!(title["authorHint"].as_str().is_some());

        Ok(())
    }

    #[test]
    fn exports_script_template_contract_without_page_templates() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-script-contract-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("scripts"))
            .map_err(|err| format!("failed to create test script dir: {err}"))?;
        fs::create_dir_all(root.join("prompts"))
            .map_err(|err| format!("failed to create test prompt dir: {err}"))?;
        fs::create_dir_all(root.join("examples"))
            .map_err(|err| format!("failed to create test example dir: {err}"))?;
        fs::write(root.join("scripts").join("build.py"), "print('ok')\n")
            .map_err(|err| format!("failed to write test script: {err}"))?;
        fs::write(
            root.join("prompts").join("authoring.md"),
            "Use the chapter Markdown structure.\n",
        )
        .map_err(|err| format!("failed to write authoring instructions: {err}"))?;
        fs::write(
            root.join("examples").join("chapter.md"),
            "# Example Course\n\n## 教学目标\n1. 说明目标\n",
        )
        .map_err(|err| format!("failed to write authoring example: {err}"))?;
        fs::write(
            root.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "script-contract-test",
  "name": "Script Contract Test",
  "format": "pptx",
  "templateType": "script",
  "input": {
    "formats": ["md"],
    "schemaId": "chapter_markdown_v1",
    "authoring": {
      "instructions": ["prompts/authoring.md"],
      "examples": ["examples/chapter.md"]
    }
  },
  "renderer": {
    "type": "script.python",
    "runtime": "python3",
    "entry": "scripts/build.py",
    "input": "chapterMarkdown",
    "output": "pptx"
  }
}"#,
        )
        .map_err(|err| format!("failed to write test manifest: {err}"))?;

        let result = export_template_contract(&root, None)?;

        assert_eq!(
            result.contract["aiOutput"]["preferredMode"].as_str(),
            Some("script_renderer")
        );
        assert_eq!(
            result.contract["aiOutput"]["scriptRenderer"]["inputFormats"][0].as_str(),
            Some("md")
        );
        assert!(
            result.contract["aiOutput"]["pptxExplicitPages"]
                .as_object()
                .is_none()
        );
        assert_eq!(
            result.contract["input"]["authoring"]["instructions"][0]["path"].as_str(),
            Some("prompts/authoring.md")
        );
        assert!(
            result.contract["input"]["authoring"]["instructions"][0]["content"]
                .as_str()
                .is_some_and(|content| content.contains("chapter Markdown"))
        );
        assert_eq!(
            result.contract["input"]["authoring"]["examples"][0]["path"].as_str(),
            Some("examples/chapter.md")
        );
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn exports_minimal_pptx_content_skeleton() -> Result<(), String> {
        let result = export_content_skeleton(
            Path::new("examples/templates/demo-medical-teaching-v1"),
            None,
            Some("minimal"),
            None,
        )?;

        assert_eq!(result.format, "pptx");
        assert_eq!(result.mode, "minimal");
        assert_eq!(result.recipe, None);
        assert_eq!(result.content["documentType"].as_str(), Some("pptx_pages"));
        let pages = result.content["pages"]
            .as_array()
            .ok_or_else(|| "skeleton pages should be an array".to_string())?;
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0]["use"].as_str(), Some("cover"));
        assert!(pages[0]["data"]["title"].as_str().is_some());

        Ok(())
    }
}
