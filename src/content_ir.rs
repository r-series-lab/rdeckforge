use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::Path};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentDocument {
    pub schema_version: String,
    pub document_type: String,
    pub title: String,
    #[serde(default)]
    pub speaker: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub goals: Vec<String>,
    #[serde(default)]
    pub chapters: Vec<Chapter>,
    #[serde(default)]
    pub summary: Vec<String>,
    #[serde(default)]
    pub evaluations: Vec<Evaluation>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub id: String,
    #[serde(default)]
    pub label: Option<String>,
    pub title: String,
    #[serde(default)]
    pub content_title: Option<String>,
    #[serde(default)]
    pub items: Vec<ContentItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentItem {
    pub heading: String,
    pub body: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Evaluation {
    pub title: String,
    #[serde(default)]
    pub items: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentValidation {
    pub input_format: String,
    pub input_profile: Option<String>,
    pub input_schema: Option<String>,
    pub input_schema_id: Option<String>,
    pub input_builtin_schema: bool,
    pub root_type: String,
    pub top_level_keys: Vec<String>,
    pub schema_version: String,
    pub document_type: String,
    pub title: String,
    pub speaker: Option<String>,
    pub chapters: usize,
    pub goals: usize,
    pub summary_items: usize,
    pub evaluations: usize,
    pub warnings: Vec<String>,
    pub schema_errors: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentWorkspaceValidation {
    pub content: ContentValidation,
    pub template: Option<crate::template_manifest::TemplateValidation>,
    pub render_format: Option<String>,
    pub selected_recipe: Option<String>,
    pub deck_recipe: Option<String>,
    pub document_recipe: Option<String>,
    pub workbook_recipe: Option<String>,
    pub planned_pages: Vec<crate::render_plan::PlannedPage>,
    pub binding_warnings: Vec<String>,
    pub acceptance_summary: AcceptanceSummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceSummary {
    pub status: AcceptanceStatus,
    pub can_render: bool,
    pub schema_error_count: usize,
    pub content_warning_count: usize,
    pub template_warning_count: usize,
    pub binding_warning_count: usize,
    pub asset_warning_count: usize,
    pub planned_page_count: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum AcceptanceStatus {
    Pass,
    Warn,
    Fail,
}

pub fn load_content(path: &Path) -> Result<ContentDocument, String> {
    crate::validator::require_file(path, "content.json")?;
    let raw =
        fs::read_to_string(path).map_err(|err| format!("failed to read content.json: {err}"))?;
    load_content_text(&raw)
}

pub fn load_content_text(raw: &str) -> Result<ContentDocument, String> {
    serde_json::from_str(raw).map_err(|err| format!("invalid content.json: {err}"))
}

pub fn validate_content(path: &Path) -> Result<ContentValidation, String> {
    let content = load_content(path)?;
    validate_content_document(&content)
}

pub fn validate_content_text(raw: &str) -> Result<ContentValidation, String> {
    let content = load_content_text(raw)?;
    validate_content_document(&content)
}

pub fn validate_content_workspace(
    raw: &str,
    template_dir: Option<&Path>,
    requested_recipe: Option<&str>,
) -> Result<ContentWorkspaceValidation, String> {
    validate_content_workspace_text(raw, "json", template_dir, requested_recipe)
}

pub fn validate_content_workspace_file(
    input: &Path,
    template_dir: Option<&Path>,
    requested_recipe: Option<&str>,
) -> Result<ContentWorkspaceValidation, String> {
    validate_content_workspace_file_with_profile(input, template_dir, requested_recipe, None)
}

pub fn validate_content_workspace_file_with_profile(
    input: &Path,
    template_dir: Option<&Path>,
    requested_recipe: Option<&str>,
    profile_id: Option<&str>,
) -> Result<ContentWorkspaceValidation, String> {
    let source = crate::content_source::load_content_source(input)?;
    validate_content_workspace_value(source, template_dir, requested_recipe, profile_id)
}

pub fn validate_content_workspace_text(
    raw: &str,
    input_format: &str,
    template_dir: Option<&Path>,
    requested_recipe: Option<&str>,
) -> Result<ContentWorkspaceValidation, String> {
    validate_content_workspace_text_with_profile(
        raw,
        input_format,
        template_dir,
        requested_recipe,
        None,
    )
}

pub fn validate_content_workspace_text_with_profile(
    raw: &str,
    input_format: &str,
    template_dir: Option<&Path>,
    requested_recipe: Option<&str>,
    profile_id: Option<&str>,
) -> Result<ContentWorkspaceValidation, String> {
    let source = crate::content_source::parse_content_source_text(raw, input_format)?;
    validate_content_workspace_value(source, template_dir, requested_recipe, profile_id)
}

fn validate_content_workspace_value(
    source: crate::content_source::ContentSource,
    template_dir: Option<&Path>,
    requested_recipe: Option<&str>,
    profile_id: Option<&str>,
) -> Result<ContentWorkspaceValidation, String> {
    let Some(template_dir) = template_dir else {
        if let Some(profile_id) = profile_id.filter(|value| !value.trim().is_empty()) {
            return validate_profile_content_workspace(source, profile_id);
        }

        let content = summarize_content_value(&source.value, &source.input_format, None, None);
        let acceptance_summary = acceptance_summary(&content, None, &[], &[], None, None);
        return Ok(ContentWorkspaceValidation {
            content,
            template: None,
            render_format: None,
            selected_recipe: None,
            deck_recipe: None,
            document_recipe: None,
            workbook_recipe: None,
            planned_pages: Vec::new(),
            binding_warnings: Vec::new(),
            acceptance_summary,
        });
    };

    let template = crate::template_manifest::validate_template_pack(template_dir)?;
    let manifest = crate::template_manifest::load_manifest(template_dir)?;
    let input_spec = crate::template_manifest::normalized_input_spec(&manifest);
    let allowed_formats = crate::template_manifest::effective_input_formats(&manifest);
    let input_format_warning = if !allowed_formats.contains(&source.input_format) {
        Some(format!(
            "input format '{}' is not declared by this template; allowed formats: {}",
            source.input_format,
            allowed_formats.join(", ")
        ))
    } else {
        None
    };
    let source = crate::content_source::apply_md_profile(source, input_spec.md_profile.as_deref())?;
    let mut content = summarize_content_value(
        &source.value,
        &source.input_format,
        input_spec.schema.clone(),
        input_spec.profile.clone(),
    );
    content.input_schema = input_spec.schema.clone();
    content.input_schema_id = input_spec.schema_id.clone();
    content.input_builtin_schema = input_spec.schema.is_none()
        && input_spec
            .profile
            .as_deref()
            .is_some_and(|profile| crate::profiles::get_profile(profile).is_some());
    content.schema_errors =
        crate::schema_validation::validate_input_schema(template_dir, &input_spec, &source.value)?;
    if let Some(warning) = input_format_warning {
        content.warnings.push(warning);
    }
    let render_format = manifest.format.clone();
    let mut deck_recipe = None;
    let mut document_recipe = None;
    let mut workbook_recipe = None;
    let explicit_pages = render_format == "pptx"
        && requested_recipe.is_none()
        && crate::render_plan::uses_declarative_explicit_pages(&manifest, &source.value);
    match render_format.as_str() {
        "pptx" => {
            if !explicit_pages {
                deck_recipe =
                    crate::template_manifest::resolve_recipe_id(&manifest, requested_recipe)?;
                if let Some(recipe_id) = deck_recipe.as_deref() {
                    crate::template_manifest::validate_deck_recipe(&manifest, recipe_id)?;
                }
            }
        }
        "docx" => {
            document_recipe =
                crate::template_manifest::resolve_document_recipe_id(&manifest, requested_recipe)?;
            if let Some(recipe_id) = document_recipe.as_deref() {
                crate::template_manifest::validate_document_recipe(&manifest, recipe_id)?;
            }
        }
        "xlsx" => {
            workbook_recipe =
                crate::template_manifest::resolve_workbook_recipe_id(&manifest, requested_recipe)?;
            if let Some(recipe_id) = workbook_recipe.as_deref() {
                crate::template_manifest::validate_workbook_recipe(&manifest, recipe_id)?;
            }
        }
        _ => {}
    }
    let selected_recipe = deck_recipe
        .clone()
        .or_else(|| document_recipe.clone())
        .or_else(|| workbook_recipe.clone());
    if !content.schema_errors.is_empty() {
        let acceptance_summary = acceptance_summary(
            &content,
            Some(&template),
            &[],
            &[],
            Some(&render_format),
            selected_recipe.as_deref(),
        );
        return Ok(ContentWorkspaceValidation {
            content,
            template: Some(template),
            render_format: Some(render_format),
            selected_recipe,
            deck_recipe,
            document_recipe,
            workbook_recipe,
            planned_pages: Vec::new(),
            binding_warnings: Vec::new(),
            acceptance_summary,
        });
    }
    let planned_pages = match deck_recipe.as_deref() {
        Some(recipe_id) => {
            crate::render_plan::build_plan_from_value(&manifest, &source.value, recipe_id)?
        }
        None if explicit_pages => {
            crate::render_plan::build_plan_from_explicit_pages(&manifest, &source.value)?
        }
        None => Vec::new(),
    };
    suppress_recipe_handled_content_warnings(&mut content, &manifest, deck_recipe.as_deref());
    let binding_warnings = validate_planned_bindings(
        &manifest,
        &source.value,
        source.content_root.as_deref(),
        &planned_pages,
    );
    let acceptance_summary = acceptance_summary(
        &content,
        Some(&template),
        &planned_pages,
        &binding_warnings,
        Some(&render_format),
        selected_recipe.as_deref(),
    );

    Ok(ContentWorkspaceValidation {
        content,
        template: Some(template),
        render_format: Some(render_format),
        selected_recipe,
        deck_recipe,
        document_recipe,
        workbook_recipe,
        planned_pages,
        binding_warnings,
        acceptance_summary,
    })
}

fn suppress_recipe_handled_content_warnings(
    content: &mut ContentValidation,
    manifest: &crate::template_manifest::TemplateManifest,
    deck_recipe: Option<&str>,
) {
    if !recipe_splits_teaching_chapter_items(manifest, deck_recipe) {
        return;
    }

    content.warnings.retain(|warning| {
        !warning.ends_with("has more than 4 items; content may overflow fixed PPT layouts")
    });
}

fn recipe_splits_teaching_chapter_items(
    manifest: &crate::template_manifest::TemplateManifest,
    deck_recipe: Option<&str>,
) -> bool {
    let Some(recipe_id) = deck_recipe else {
        return false;
    };
    let Some(steps) = manifest.deck_recipes.get(recipe_id) else {
        return false;
    };

    steps
        .iter()
        .any(|step| step_splits_teaching_chapter_items(step, false))
}

fn step_splits_teaching_chapter_items(
    step: &crate::template_manifest::DeckStep,
    inside_chapter_scope: bool,
) -> bool {
    let enters_chapter_scope = step.repeat.as_deref() == Some("$.chapters");
    let chapter_scope = inside_chapter_scope || enters_chapter_scope;
    let step_handles_items = chapter_scope
        && step
            .overflow
            .as_ref()
            .is_some_and(overflow_splits_teaching_items);
    step_handles_items
        || step
            .steps
            .iter()
            .any(|child| step_splits_teaching_chapter_items(child, chapter_scope))
}

fn overflow_splits_teaching_items(overflow: &crate::template_manifest::DeckStepOverflow) -> bool {
    overflow.path == "$.items"
        && overflow.max_items <= 4
        && overflow.strategy.as_deref().unwrap_or("split") == "split"
}

fn validate_profile_content_workspace(
    source: crate::content_source::ContentSource,
    profile_id: &str,
) -> Result<ContentWorkspaceValidation, String> {
    let input_spec = crate::profiles::input_spec_for_profile(profile_id)?;
    let allowed_formats = input_spec.formats.clone();
    let input_format_warning = if !allowed_formats.contains(&source.input_format) {
        Some(format!(
            "input format '{}' is not declared by profile '{}'; allowed formats: {}",
            source.input_format,
            profile_id,
            allowed_formats.join(", ")
        ))
    } else {
        None
    };
    let source = crate::content_source::apply_md_profile(source, input_spec.md_profile.as_deref())?;
    let mut content = summarize_content_value(
        &source.value,
        &source.input_format,
        input_spec.schema.clone(),
        input_spec.profile.clone(),
    );
    content.input_schema = input_spec.schema.clone();
    content.input_schema_id = input_spec.schema_id.clone();
    content.input_builtin_schema = true;
    content.schema_errors = crate::schema_validation::validate_input_schema(
        Path::new("."),
        &input_spec,
        &source.value,
    )?;
    if let Some(warning) = input_format_warning {
        content.warnings.push(warning);
    }
    let acceptance_summary = acceptance_summary(&content, None, &[], &[], None, None);
    Ok(ContentWorkspaceValidation {
        content,
        template: None,
        render_format: None,
        selected_recipe: None,
        deck_recipe: None,
        document_recipe: None,
        workbook_recipe: None,
        planned_pages: Vec::new(),
        binding_warnings: Vec::new(),
        acceptance_summary,
    })
}

fn acceptance_summary(
    content: &ContentValidation,
    template: Option<&crate::template_manifest::TemplateValidation>,
    planned_pages: &[crate::render_plan::PlannedPage],
    binding_warnings: &[String],
    render_format: Option<&str>,
    selected_recipe: Option<&str>,
) -> AcceptanceSummary {
    let schema_error_count = content.schema_errors.len();
    let content_warning_count = content.warnings.len();
    let template_warning_count = template
        .map(|template| template.warnings.len())
        .unwrap_or_default();
    let binding_warning_count = binding_warnings.len();
    let asset_warning_count = binding_warnings
        .iter()
        .filter(|warning| is_asset_warning(warning))
        .count();
    let planned_page_count = planned_pages.len();
    let has_blocking_binding_warning = binding_warnings
        .iter()
        .any(|warning| is_blocking_binding_warning(warning));
    let is_script_template = template.is_some_and(|template| {
        matches!(template.template_type.as_str(), "script" | "hybrid")
            && template.renderer_type.is_some()
    });
    let can_render = schema_error_count == 0
        && template.is_some()
        && match render_format {
            Some(_) if is_script_template => !has_blocking_binding_warning,
            Some("pptx") => planned_page_count > 0 && !has_blocking_binding_warning,
            Some("docx" | "xlsx") => selected_recipe.is_some() && !has_blocking_binding_warning,
            _ => false,
        };
    let status = if schema_error_count > 0 {
        AcceptanceStatus::Fail
    } else if content_warning_count > 0 || template_warning_count > 0 || binding_warning_count > 0 {
        AcceptanceStatus::Warn
    } else {
        AcceptanceStatus::Pass
    };
    let message = match status {
        AcceptanceStatus::Pass if is_script_template => "Script template input passed".to_string(),
        AcceptanceStatus::Pass => match render_format {
            Some("pptx") => format!("AI output passed: {planned_page_count} planned pages"),
            Some(format) => format!(
                "AI output passed for {format} recipe '{}'",
                selected_recipe.unwrap_or("-")
            ),
            None => "AI output passed schema validation".to_string(),
        },
        AcceptanceStatus::Warn if is_script_template => format!(
            "Script template input is usable with warnings: {content_warning_count} content warnings, {template_warning_count} template warnings"
        ),
        AcceptanceStatus::Warn => match render_format {
            Some("pptx") => format!(
                "AI output is usable with warnings: {planned_page_count} planned pages, {content_warning_count} content warnings, {template_warning_count} template warnings, {binding_warning_count} binding warnings"
            ),
            Some(format) => format!(
                "AI output is usable with warnings for {format} recipe '{}': {content_warning_count} content warnings, {template_warning_count} template warnings, {binding_warning_count} binding warnings",
                selected_recipe.unwrap_or("-"),
            ),
            None => "AI output is usable with warnings".to_string(),
        },
        AcceptanceStatus::Fail => {
            format!("AI output failed schema validation: {schema_error_count} schema errors")
        }
    };
    AcceptanceSummary {
        status,
        can_render,
        schema_error_count,
        content_warning_count,
        template_warning_count,
        binding_warning_count,
        asset_warning_count,
        planned_page_count,
        message,
    }
}

fn is_asset_warning(warning: &str) -> bool {
    warning.contains("image file")
        || warning.contains("image path")
        || warning.contains("image source")
        || warning.contains("remote/data")
        || warning.contains("relative image")
        || warning.contains("absolute image")
}

fn is_blocking_binding_warning(warning: &str) -> bool {
    warning.contains("image file not found")
        || warning.contains("required but resolved empty")
        || warning.contains("uses unsupported remote/data")
        || warning.contains("relative image path")
        || warning.contains("invalid dataPath")
        || warning.contains("expects image data")
        || warning.contains("expects chart object")
        || warning.contains("expects table data")
        || warning.contains("expects list data")
}

fn validate_content_document(content: &ContentDocument) -> Result<ContentValidation, String> {
    let mut warnings = Vec::new();

    if content.schema_version.trim().is_empty() {
        warnings.push("schemaVersion is empty".to_string());
    }
    if content.document_type != "teaching_deck" {
        warnings.push(format!(
            "documentType is '{}', expected 'teaching_deck' for the MVP",
            content.document_type
        ));
    }
    if content.goals.len() > 4 {
        warnings.push(
            "goals has more than 4 items; many teaching templates will need compression"
                .to_string(),
        );
    }
    if content.chapters.len() > 6 {
        warnings.push(
            "chapters has more than 6 items; first teaching template flow targets up to 6 chapters"
                .to_string(),
        );
    }
    for chapter in &content.chapters {
        if chapter.items.len() > 4 {
            warnings.push(format!(
                "{} has more than 4 items; content may overflow fixed PPT layouts",
                chapter.id
            ));
        }
    }

    if content.title.trim().is_empty() {
        return Err("title is required".to_string());
    }
    if content.chapters.is_empty() {
        return Err("at least one chapter is required".to_string());
    }

    Ok(ContentValidation {
        input_format: "json".to_string(),
        input_profile: None,
        input_schema: None,
        input_schema_id: Some("teaching_deck_v1".to_string()),
        input_builtin_schema: false,
        root_type: "object".to_string(),
        top_level_keys: Vec::new(),
        schema_version: content.schema_version.clone(),
        document_type: content.document_type.clone(),
        title: content.title.clone(),
        speaker: content.speaker.clone(),
        chapters: content.chapters.len(),
        goals: content.goals.len(),
        summary_items: content.summary.len(),
        evaluations: content.evaluations.len(),
        warnings,
        schema_errors: Vec::new(),
    })
}

fn summarize_content_value(
    value: &Value,
    input_format: &str,
    input_schema: Option<String>,
    input_profile: Option<String>,
) -> ContentValidation {
    let mut warnings = Vec::new();
    let teaching = serde_json::from_value::<ContentDocument>(value.clone()).ok();
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| teaching.as_ref().map(|content| content.title.clone()))
        .unwrap_or_else(|| "Untitled content".to_string());
    let schema_version = value
        .get("schemaVersion")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let document_type = value
        .get("documentType")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let top_level_keys = value
        .as_object()
        .map(|object| object.keys().cloned().collect())
        .unwrap_or_default();

    let (speaker, goals, chapters, summary_items, evaluations, teaching_warnings) = match teaching {
        Some(content) => {
            let validation = validate_content_document(&content).ok();
            (
                content.speaker,
                content.goals.len(),
                content.chapters.len(),
                content.summary.len(),
                content.evaluations.len(),
                validation.map(|result| result.warnings).unwrap_or_default(),
            )
        }
        None => (None, 0, 0, 0, 0, Vec::new()),
    };
    warnings.extend(teaching_warnings);

    ContentValidation {
        input_format: input_format.to_string(),
        input_profile,
        input_schema,
        input_schema_id: None,
        input_builtin_schema: false,
        root_type: value_type_name(value).to_string(),
        top_level_keys,
        schema_version,
        document_type,
        title,
        speaker,
        chapters,
        goals,
        summary_items,
        evaluations,
        warnings,
        schema_errors: Vec::new(),
    }
}

fn validate_planned_bindings(
    manifest: &crate::template_manifest::TemplateManifest,
    content: &Value,
    content_root: Option<&Path>,
    planned_pages: &[crate::render_plan::PlannedPage],
) -> Vec<String> {
    let mut warnings = Vec::new();

    for page in planned_pages {
        let Some(page_template) = manifest.page_templates.get(&page.page_template) else {
            warnings.push(format!(
                "page {} references missing pageTemplate '{}'",
                page.page_index, page.page_template
            ));
            continue;
        };

        let resolved_page_data;
        let page_data = if let Some(data_value) = &page.data_value {
            data_value
        } else {
            resolved_page_data = match resolve_data_path(content, &page.data_path, content) {
                Ok(Some(value)) => value,
                Ok(None) => {
                    warnings.push(format!(
                        "page {} data path '{}' resolved empty",
                        page.page_index, page.data_path
                    ));
                    continue;
                }
                Err(err) => {
                    warnings.push(format!(
                        "page {} data path '{}' is invalid: {err}",
                        page.page_index, page.data_path
                    ));
                    continue;
                }
            };
            &resolved_page_data
        };

        for (binding_id, binding) in &page_template.bindings {
            if binding.shape_name.is_none() {
                warnings.push(format!(
                    "page {} binding '{binding_id}' has no shapeName; creationId rendering is not implemented yet",
                    page.page_index
                ));
                continue;
            }

            let data_path = binding.data_path.as_deref().unwrap_or("$");
            let value = match resolve_data_path(&page_data, data_path, &page_data) {
                Ok(value) => value,
                Err(err) => {
                    warnings.push(format!(
                        "page {} binding '{binding_id}' has invalid dataPath '{data_path}': {err}",
                        page.page_index
                    ));
                    continue;
                }
            };

            let Some(value) = value else {
                if binding.required {
                    warnings.push(format!(
                        "page {} binding '{binding_id}' is required but resolved empty",
                        page.page_index
                    ));
                }
                continue;
            };

            let binding_type = binding.binding_type.as_deref().unwrap_or("text");
            if matches!(binding_type, "image" | "backgroundImage") {
                validate_image_binding(
                    &mut warnings,
                    page.page_index,
                    binding_id,
                    &value,
                    content_root,
                );
                continue;
            }
            if binding_type == "chart" && binding.chart_mode.as_deref() == Some("image") {
                validate_chart_image_binding(
                    &mut warnings,
                    page.page_index,
                    binding_id,
                    &value,
                    content_root,
                );
                continue;
            }
            if binding_type == "chart" {
                validate_chart_data_binding(&mut warnings, page.page_index, binding_id, &value);
                continue;
            }
            if binding_type == "table" {
                validate_table_binding(&mut warnings, page.page_index, binding_id, &value);
                continue;
            }
            if binding_type == "list" {
                if let Some(items) = value.as_array() {
                    if let Some(max_items) = binding.max_items {
                        if items.len() > max_items {
                            warnings.push(format!(
                                "page {} binding '{binding_id}' has {} items, maxItems is {max_items}",
                                page.page_index,
                                items.len()
                            ));
                        }
                    }
                } else {
                    warnings.push(format!(
                        "page {} binding '{binding_id}' expects list data but resolved {}",
                        page.page_index,
                        value_type_name(&value)
                    ));
                }
                continue;
            }

            if binding_type != "text" {
                warnings.push(format!(
                    "page {} binding '{binding_id}' uses unsupported type '{binding_type}', rendered as text",
                    page.page_index
                ));
            }

            let text = stringify_value(&value);
            if let Some(max_length) = binding.max_length {
                let length = text.chars().count();
                if length > max_length {
                    warnings.push(format!(
                        "page {} binding '{binding_id}' length {length} exceeds maxLength {max_length}",
                        page.page_index
                    ));
                }
            }
        }
    }

    warnings
}

fn validate_chart_data_binding(
    warnings: &mut Vec<String>,
    page_index: usize,
    binding_id: &str,
    value: &Value,
) {
    let Some(object) = value.as_object() else {
        warnings.push(format!(
            "page {page_index} binding '{binding_id}' expects chart object data but resolved {}",
            value_type_name(value)
        ));
        return;
    };
    validate_chart_data_object(warnings, page_index, binding_id, object);
}

fn validate_chart_image_binding(
    warnings: &mut Vec<String>,
    page_index: usize,
    binding_id: &str,
    value: &Value,
    content_root: Option<&Path>,
) {
    if image_src(value).is_some() || value.is_string() {
        validate_image_binding(warnings, page_index, binding_id, value, content_root);
        return;
    }

    let Some(object) = value.as_object() else {
        warnings.push(format!(
            "page {page_index} binding '{binding_id}' expects chart image data as an object or image source"
        ));
        return;
    };
    validate_chart_data_object(warnings, page_index, binding_id, object);
}

fn validate_chart_data_object(
    warnings: &mut Vec<String>,
    page_index: usize,
    binding_id: &str,
    object: &serde_json::Map<String, Value>,
) {
    let labels_len = object
        .get("labels")
        .or_else(|| object.get("categories"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if labels_len == 0 {
        warnings.push(format!(
            "page {page_index} binding '{binding_id}' chart data has no labels"
        ));
    }

    let series = object.get("series").and_then(Value::as_array);
    let direct_values_len = object
        .get("values")
        .or_else(|| object.get("data"))
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    if series.is_none_or(Vec::is_empty) && direct_values_len == 0 {
        warnings.push(format!(
            "page {page_index} binding '{binding_id}' chart data has no series or values"
        ));
    }
    if let Some(series) = series {
        for (index, item) in series.iter().enumerate() {
            let values_len = if let Some(values) = item.as_array() {
                values.len()
            } else {
                item.as_object()
                    .and_then(|object| object.get("values").or_else(|| object.get("data")))
                    .and_then(Value::as_array)
                    .map(Vec::len)
                    .unwrap_or(0)
            };
            if values_len == 0 {
                warnings.push(format!(
                    "page {page_index} binding '{binding_id}' chart series {index} has no values"
                ));
            } else if labels_len > 0 && values_len != labels_len {
                warnings.push(format!(
                    "page {page_index} binding '{binding_id}' chart series {index} has {values_len} values but labels has {labels_len}"
                ));
            }
        }
    } else if labels_len > 0 && direct_values_len > 0 && direct_values_len != labels_len {
        warnings.push(format!(
            "page {page_index} binding '{binding_id}' chart has {direct_values_len} values but labels has {labels_len}"
        ));
    }
}

fn validate_table_binding(
    warnings: &mut Vec<String>,
    page_index: usize,
    binding_id: &str,
    value: &Value,
) {
    if let Some(rows) = value.as_array() {
        if rows.is_empty() {
            warnings.push(format!(
                "page {page_index} binding '{binding_id}' table has no rows"
            ));
            return;
        }
        let valid_rows = rows
            .iter()
            .all(|row| row.is_array() || row.is_object() || row.is_string() || row.is_number());
        if !valid_rows {
            warnings.push(format!(
                "page {page_index} binding '{binding_id}' table array contains unsupported row values"
            ));
        }
        return;
    }

    let Some(object) = value.as_object() else {
        warnings.push(format!(
            "page {page_index} binding '{binding_id}' expects table data but resolved {}",
            value_type_name(value)
        ));
        return;
    };
    let has_columns_rows = object
        .get("columns")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty())
        && object
            .get("rows")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty());
    let has_body = object
        .get("body")
        .and_then(Value::as_array)
        .is_some_and(|items| !items.is_empty());
    if !has_columns_rows && !has_body {
        warnings.push(format!(
            "page {page_index} binding '{binding_id}' table object needs columns+rows or body"
        ));
    }
}

fn validate_image_binding(
    warnings: &mut Vec<String>,
    page_index: usize,
    binding_id: &str,
    value: &Value,
    content_root: Option<&Path>,
) {
    let Some(src) = image_src(value) else {
        warnings.push(format!(
            "page {page_index} binding '{binding_id}' expects image data as a path string or object with src"
        ));
        return;
    };
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
        warnings.push(format!(
            "page {page_index} binding '{binding_id}' uses unsupported remote/data image source"
        ));
        return;
    }
    let path = Path::new(src);
    if path.is_absolute() {
        if !path.is_file() {
            warnings.push(format!(
                "page {page_index} binding '{binding_id}' image file not found: {}",
                path.display()
            ));
        } else {
            warnings.push(format!(
                "page {page_index} binding '{binding_id}' uses an absolute image path; content packs are more portable with relative assets"
            ));
        }
        return;
    }
    let Some(content_root) = content_root else {
        warnings.push(format!(
            "page {page_index} binding '{binding_id}' uses relative image path '{src}' but no content root is available"
        ));
        return;
    };
    let resolved = content_root.join(path);
    if !resolved.is_file() {
        warnings.push(format!(
            "page {page_index} binding '{binding_id}' image file not found: {}",
            resolved.display()
        ));
    }
}

fn image_src(value: &Value) -> Option<&str> {
    value
        .as_str()
        .or_else(|| value.as_object()?.get("src")?.as_str())
}

pub(crate) fn resolve_data_path(
    root: &Value,
    expression: &str,
    fallback_root: &Value,
) -> Result<Option<Value>, String> {
    let trimmed = expression.trim();
    if is_quoted_literal(trimmed) {
        return Ok(Some(Value::String(
            trimmed[1..trimmed.len() - 1].to_string(),
        )));
    }
    if trimmed == "$" {
        return Ok(Some(root.clone()));
    }
    if !trimmed.starts_with("$.") {
        return Ok(Some(fallback_root.clone()));
    }

    let mut current = root;
    for segment in parse_path_segments(trimmed)? {
        match segment {
            PathSegment::Key(key) => {
                let Some(next) = current.get(&key) else {
                    return Ok(None);
                };
                current = next;
            }
            PathSegment::Index(index) => {
                let Some(next) = current.as_array().and_then(|items| items.get(index)) else {
                    return Ok(None);
                };
                current = next;
            }
        }
    }
    Ok(Some(current.clone()))
}

enum PathSegment {
    Key(String),
    Index(usize),
}

fn parse_path_segments(expression: &str) -> Result<Vec<PathSegment>, String> {
    let path = &expression[2..];
    let bytes = path.as_bytes();
    let mut segments = Vec::new();
    let mut cursor = 0;

    while cursor < path.len() {
        if bytes[cursor] == b'.' {
            cursor += 1;
        }
        if cursor >= path.len() {
            return Err(format!("empty segment in dataPath '{expression}'"));
        }

        if bytes[cursor] == b'[' {
            let close = path[cursor..]
                .find(']')
                .map(|offset| cursor + offset)
                .ok_or_else(|| format!("missing closing bracket in dataPath '{expression}'"))?;
            let index = path[cursor + 1..close]
                .parse::<usize>()
                .map_err(|_| format!("array index is not an integer in dataPath '{expression}'"))?;
            segments.push(PathSegment::Index(index));
            cursor = close + 1;
            continue;
        }

        let start = cursor;
        while cursor < path.len() && bytes[cursor] != b'.' && bytes[cursor] != b'[' {
            cursor += 1;
        }
        let key = &path[start..cursor];
        if key.is_empty() {
            return Err(format!("empty segment in dataPath '{expression}'"));
        }
        segments.push(PathSegment::Key(key.to_string()));
    }

    Ok(segments)
}

fn is_quoted_literal(value: &str) -> bool {
    value.len() >= 2
        && ((value.starts_with('\'') && value.ends_with('\''))
            || (value.starts_with('"') && value.ends_with('"')))
}

pub(crate) fn stringify_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| String::new()),
    }
}

fn value_type_name(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        fs::File,
        io::Write,
        path::{Path, PathBuf},
        time::SystemTime,
    };
    use zip::{ZipWriter, write::FileOptions};

    #[test]
    fn validates_content_against_builtin_profile_without_template() -> Result<(), String> {
        let raw = r#"{
  "title": "功能点评估",
  "detailed_rows": [{
    "module": "营销管理",
    "feature": "新增宣传模块",
    "change_type": "全新页面",
    "effort": 2,
    "backend_dependency": "是",
    "backend_notes": "需要列表接口",
    "owner": "",
    "progress": "未开始",
    "notes": "按现有权限体系接入"
  }],
  "summary": {
    "evaluation_basis": "基于需求和代码基线估算。",
    "assumptions": "接口按期提供。",
    "out_of_scope": "不含上线陪跑。"
  }
}"#;

        let validation = validate_content_workspace_text_with_profile(
            raw,
            "json",
            None,
            None,
            Some("feature_assessment_v1"),
        )?;

        assert_eq!(
            validation.content.input_profile.as_deref(),
            Some("feature_assessment_v1")
        );
        assert_eq!(
            validation.content.input_schema_id.as_deref(),
            Some("feature_assessment_v1")
        );
        assert!(validation.content.input_builtin_schema);
        assert!(validation.content.schema_errors.is_empty());
        Ok(())
    }

    #[test]
    fn reports_builtin_profile_schema_errors() -> Result<(), String> {
        let validation = validate_content_workspace_text_with_profile(
            r#"{"title":"缺少明细"}"#,
            "json",
            None,
            None,
            Some("feature_assessment_v1"),
        )?;

        assert!(!validation.content.schema_errors.is_empty());
        assert!(
            validation
                .content
                .schema_errors
                .iter()
                .any(|message| message.contains("detailed_rows"))
        );
        Ok(())
    }

    #[test]
    fn acceptance_summary_allows_script_template_without_planned_pages() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-script-acceptance-{}",
            uuid::Uuid::new_v4()
        ));
        let scripts = root.join("scripts");
        fs::create_dir_all(&scripts).map_err(|err| format!("failed to create scripts: {err}"))?;
        fs::write(
            scripts.join("build.py"),
            r#"from pathlib import Path
Path("unused.txt").write_text("ok", encoding="utf-8")
"#,
        )
        .map_err(|err| format!("failed to write script: {err}"))?;
        fs::write(
            root.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "script-acceptance-test",
  "name": "Script Acceptance Test",
  "format": "pptx",
  "templateType": "script",
  "input": { "formats": ["md"] },
  "renderer": {
    "type": "script.python",
    "entry": "scripts/build.py"
  }
}"#,
        )
        .map_err(|err| format!("failed to write manifest: {err}"))?;

        let validation = validate_content_workspace_text_with_profile(
            "# Script Input",
            "md",
            Some(&root),
            None,
            None,
        )?;

        assert!(validation.acceptance_summary.can_render);
        assert_eq!(validation.acceptance_summary.planned_page_count, 0);
        assert_eq!(
            validation.acceptance_summary.message,
            "Script template input passed"
        );
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn acceptance_summary_blocks_missing_relative_assets() -> Result<(), String> {
        let raw = r#"{
  "schemaVersion": "1.0",
  "documentType": "teaching_deck",
  "title": "AI 输出验收",
  "coverImage": "assets/missing-cover.png",
  "goals": ["理解验收链路"],
  "chapters": [{
    "id": "chapter-1",
    "title": "验收流程",
    "items": [{ "heading": "检查", "body": "检查协议、绑定和资源路径。" }]
  }],
  "summary": ["先验收，再渲染"],
  "evaluations": [{ "title": "课堂提问", "items": ["资源路径是否存在？"] }]
}"#;

        let validation = validate_content_workspace_text_with_profile(
            raw,
            "json",
            Some(Path::new("examples/templates/demo-medical-teaching-v1")),
            Some("teaching_deck"),
            None,
        )?;

        assert_eq!(validation.acceptance_summary.schema_error_count, 0);
        assert_eq!(validation.acceptance_summary.asset_warning_count, 1);
        assert!(!validation.acceptance_summary.can_render);
        assert!(
            validation
                .binding_warnings
                .iter()
                .any(|message| message.contains("relative image path"))
        );
        Ok(())
    }

    #[test]
    fn overflow_recipe_suppresses_fixed_layout_item_warning() -> Result<(), String> {
        let raw = r#"{
  "schemaVersion": "1.0",
  "documentType": "teaching_deck",
  "title": "长章节拆页",
  "goals": ["理解拆页"],
  "chapters": [{
    "id": "chapter-1",
    "title": "护理执行",
    "contentTitle": "将预防措施嵌入每班工作",
    "items": [
      { "heading": "要点 1", "body": "正文" },
      { "heading": "要点 2", "body": "正文" },
      { "heading": "要点 3", "body": "正文" },
      { "heading": "要点 4", "body": "正文" },
      { "heading": "要点 5", "body": "正文" }
    ]
  }],
  "summary": ["复盘"],
  "evaluations": [{ "title": "提问", "items": ["如何交接？"] }]
}"#;

        let validation = validate_content_workspace_text_with_profile(
            raw,
            "json",
            Some(Path::new("examples/templates/demo-medical-teaching-v1")),
            Some("chapter_overflow_demo"),
            None,
        )?;

        assert!(
            !validation
                .content
                .warnings
                .iter()
                .any(|message| message.contains("may overflow fixed PPT layouts"))
        );
        assert_eq!(validation.acceptance_summary.content_warning_count, 0);
        assert_eq!(validation.acceptance_summary.planned_page_count, 4);
        Ok(())
    }

    #[test]
    fn acceptance_summary_allows_docx_template_recipe_without_planned_pages() -> Result<(), String>
    {
        let template_dir = temp_dir("docx-template")?;
        fs::write(
            template_dir.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "test-docx",
  "name": "Test DOCX",
  "format": "docx",
  "entry": "template.docx",
  "input": { "formats": ["json"] },
  "blockTemplates": {
    "body": {
      "bindings": {
        "title": { "placeholder": "{{title}}", "dataPath": "$.title", "type": "text" }
      }
    }
  },
  "documentRecipes": {
    "standard_document": [{ "use": "body", "data": "$" }]
  }
}"#,
        )
        .map_err(|err| err.to_string())?;
        write_zip(
            &template_dir.join("template.docx"),
            &[("word/document.xml", "<w:document>{{title}}</w:document>")],
        )?;

        let validation = validate_content_workspace_text_with_profile(
            r#"{"title":"DOCX AI 输出"}"#,
            "json",
            Some(&template_dir),
            Some("standard_document"),
            None,
        )?;

        assert_eq!(validation.render_format.as_deref(), Some("docx"));
        assert_eq!(
            validation.selected_recipe.as_deref(),
            Some("standard_document")
        );
        assert_eq!(validation.acceptance_summary.planned_page_count, 0);
        assert!(validation.acceptance_summary.can_render);
        let _ = fs::remove_dir_all(template_dir);
        Ok(())
    }

    #[test]
    fn acceptance_summary_allows_xlsx_template_recipe_without_planned_pages() -> Result<(), String>
    {
        let template_dir = temp_dir("xlsx-template")?;
        fs::write(
            template_dir.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "test-xlsx",
  "name": "Test XLSX",
  "format": "xlsx",
  "entry": "template.xlsx",
  "input": { "formats": ["json"] },
  "sheetTemplates": {
    "sheet": {
      "sourceSheet": "Sheet1",
      "bindings": {
        "title": { "cell": "A1", "dataPath": "$.title", "type": "text" }
      }
    }
  },
  "workbookRecipes": {
    "standard_workbook": [{ "use": "sheet", "data": "$" }]
  }
}"#,
        )
        .map_err(|err| err.to_string())?;
        write_zip(
            &template_dir.join("template.xlsx"),
            &[(
                "xl/workbook.xml",
                r#"<workbook><sheets><sheet name="Sheet1" sheetId="1" r:id="rId1"/></sheets></workbook>"#,
            )],
        )?;

        let validation = validate_content_workspace_text_with_profile(
            r#"{"title":"XLSX AI 输出"}"#,
            "json",
            Some(&template_dir),
            Some("standard_workbook"),
            None,
        )?;

        assert_eq!(validation.render_format.as_deref(), Some("xlsx"));
        assert_eq!(
            validation.selected_recipe.as_deref(),
            Some("standard_workbook")
        );
        assert_eq!(validation.acceptance_summary.planned_page_count, 0);
        assert!(validation.acceptance_summary.can_render);
        let _ = fs::remove_dir_all(template_dir);
        Ok(())
    }

    fn temp_dir(label: &str) -> Result<PathBuf, String> {
        let suffix = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_err(|err| err.to_string())?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("rdeckforge-{label}-{suffix}"));
        fs::create_dir_all(&path).map_err(|err| err.to_string())?;
        Ok(path)
    }

    fn write_zip(path: &Path, entries: &[(&str, &str)]) -> Result<(), String> {
        let file = File::create(path).map_err(|err| err.to_string())?;
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default();
        for (name, content) in entries {
            zip.start_file(*name, options)
                .map_err(|err| err.to_string())?;
            zip.write_all(content.as_bytes())
                .map_err(|err| err.to_string())?;
        }
        zip.finish().map_err(|err| err.to_string())?;
        Ok(())
    }
}
