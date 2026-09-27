use chrono::{SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderJob {
    pub job_id: String,
    pub created_at: String,
    pub format: String,
    pub deck_recipe: Option<String>,
    pub recipe_step_count: usize,
    pub planned_pages: Vec<crate::render_plan::PlannedPage>,
    pub template_dir: String,
    pub manifest_file: String,
    pub content_file: String,
    pub content_root: String,
    pub content_format: String,
    pub content_value: Option<Value>,
    pub markdown_file: Option<String>,
    pub output_file: String,
}

pub fn build_pptx_job(
    template_dir: &Path,
    content_file: &Path,
    markdown_file: Option<&Path>,
    output_file: &Path,
    requested_recipe: Option<&str>,
) -> Result<RenderJob, String> {
    crate::template_manifest::validate_template_pack(template_dir)?;
    let manifest = crate::template_manifest::load_manifest(template_dir)?;
    let content_source = crate::content_source::load_content_source(content_file)?;
    let input_spec = crate::template_manifest::normalized_input_spec(&manifest);
    let allowed_formats = crate::template_manifest::effective_input_formats(&manifest);
    if !allowed_formats.contains(&content_source.input_format) {
        return Err(format!(
            "input format '{}' is not declared by template '{}'; allowed formats: {}",
            content_source.input_format,
            manifest.template_id,
            allowed_formats.join(", ")
        ));
    }
    let content_source =
        crate::content_source::apply_md_profile(content_source, input_spec.md_profile.as_deref())?;
    crate::schema_validation::enforce_input_schema(
        template_dir,
        &input_spec,
        &content_source.value,
    )?;
    let explicit_pages = requested_recipe.is_none()
        && crate::render_plan::uses_declarative_explicit_pages(&manifest, &content_source.value);
    let deck_recipe = if explicit_pages {
        None
    } else {
        crate::template_manifest::resolve_recipe_id(&manifest, requested_recipe)?
    };
    if let Some(recipe_id) = deck_recipe.as_deref() {
        crate::template_manifest::validate_deck_recipe(&manifest, recipe_id)?;
    }
    let recipe_step_count = deck_recipe
        .as_deref()
        .and_then(|recipe_id| manifest.deck_recipes.get(recipe_id))
        .map(|steps| steps.len())
        .unwrap_or(0);
    let planned_pages = match deck_recipe.as_deref() {
        Some(recipe_id) => {
            crate::render_plan::build_plan_from_value(&manifest, &content_source.value, recipe_id)?
        }
        None if explicit_pages => {
            crate::render_plan::build_plan_from_explicit_pages(&manifest, &content_source.value)?
        }
        None => Vec::new(),
    };
    let source_path = content_source
        .source_path
        .clone()
        .unwrap_or_else(|| content_file.to_path_buf());
    let content_root = content_source
        .content_root
        .clone()
        .or_else(|| source_path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."));
    let content_value = if content_source.input_format == "json" {
        None
    } else {
        Some(content_source.value)
    };

    let manifest_file = crate::template_manifest::manifest_path(template_dir);
    Ok(RenderJob {
        job_id: format!("job_{}", Uuid::new_v4()),
        created_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        format: "pptx".to_string(),
        deck_recipe,
        recipe_step_count,
        planned_pages,
        template_dir: absolutize(template_dir).display().to_string(),
        manifest_file: absolutize(&manifest_file).display().to_string(),
        content_file: absolutize(&source_path).display().to_string(),
        content_root: absolutize(&content_root).display().to_string(),
        content_format: content_source.input_format,
        content_value,
        markdown_file: markdown_file.map(|path| absolutize(path).display().to_string()),
        output_file: absolutize(output_file).display().to_string(),
    })
}

fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}
