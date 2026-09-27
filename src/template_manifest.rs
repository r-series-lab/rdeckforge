use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};
use zip::ZipArchive;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateManifest {
    pub schema_version: String,
    pub template_id: String,
    pub name: String,
    #[serde(default)]
    pub family_id: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    pub format: String,
    #[serde(default)]
    pub template_type: Option<String>,
    #[serde(default)]
    pub entry: Option<String>,
    #[serde(default)]
    pub content_schema: Option<String>,
    #[serde(default)]
    pub input: Option<TemplateInputSpec>,
    #[serde(default)]
    pub preview: Option<TemplatePreviewSpec>,
    #[serde(default)]
    pub render_mode: Option<String>,
    #[serde(default)]
    pub renderer: Option<TemplateRendererSpec>,
    #[serde(default)]
    pub pipeline: Vec<TemplatePipelineStep>,
    #[serde(default)]
    pub dependencies: TemplateDependencySpec,
    #[serde(default, rename = "tests")]
    pub test_cases: Vec<TemplateTestCase>,
    #[serde(default)]
    pub page_templates: BTreeMap<String, PageTemplate>,
    #[serde(default)]
    pub deck_recipes: BTreeMap<String, Vec<DeckStep>>,
    #[serde(default)]
    pub block_templates: BTreeMap<String, BlockTemplate>,
    #[serde(default)]
    pub document_recipes: BTreeMap<String, Vec<DocumentStep>>,
    #[serde(default)]
    pub sheet_templates: BTreeMap<String, SheetTemplate>,
    #[serde(default)]
    pub workbook_recipes: BTreeMap<String, Vec<WorkbookStep>>,
    #[serde(default)]
    pub layouts: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplateRendererSpec {
    #[serde(default, rename = "type")]
    pub renderer_type: Option<String>,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub entry: Option<String>,
    #[serde(default)]
    pub input: Option<String>,
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePipelineStep {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, rename = "type")]
    pub step_type: Option<String>,
    #[serde(default)]
    pub runtime: Option<String>,
    #[serde(default)]
    pub entry: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDependencySpec {
    #[serde(default)]
    pub files: Vec<TemplateFileDependency>,
    #[serde(default)]
    pub commands: Vec<TemplateCommandDependency>,
    #[serde(default)]
    pub environment: Vec<TemplateEnvironmentDependency>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateFileDependency {
    pub path: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateCommandDependency {
    pub id: String,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateEnvironmentDependency {
    pub name: String,
    #[serde(default = "default_true")]
    pub required: bool,
    #[serde(default)]
    pub allow_empty: bool,
    #[serde(default)]
    pub platforms: Vec<String>,
    #[serde(default)]
    pub description: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateTestCase {
    pub id: String,
    pub input: String,
    #[serde(default)]
    pub recipe: Option<String>,
    #[serde(default)]
    pub output_name: Option<String>,
    #[serde(default)]
    pub assertions: TemplateTestAssertions,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplateTestAssertions {
    #[serde(default)]
    pub validate_package: Option<bool>,
    #[serde(default)]
    pub min_size_bytes: Option<u64>,
    #[serde(default)]
    pub min_slides: Option<usize>,
    #[serde(default)]
    pub max_slides: Option<usize>,
    #[serde(default)]
    pub min_sheets: Option<usize>,
    #[serde(default)]
    pub max_sheets: Option<usize>,
    #[serde(default)]
    pub required_zip_entries: Vec<String>,
    #[serde(default)]
    pub required_text: Vec<String>,
    #[serde(default)]
    pub forbidden_text: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplateInputSpec {
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub schema_id: Option<String>,
    #[serde(default)]
    pub md_profile: Option<String>,
    #[serde(default)]
    pub authoring: Option<TemplateAuthoringSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplateAuthoringSpec {
    #[serde(default)]
    pub instructions: Vec<String>,
    #[serde(default)]
    pub examples: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreviewSpec {
    #[serde(default)]
    pub cover: Option<String>,
    #[serde(default)]
    pub slides: Vec<TemplatePreviewSlide>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePreviewSlide {
    #[serde(default)]
    pub title: Option<String>,
    pub image: String,
}

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TemplateResolvedPreview {
    pub cover: Option<String>,
    pub slides: Vec<TemplatePreviewSlide>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageTemplate {
    pub source_slide: u32,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub bindings: BTreeMap<String, PageBinding>,
    #[serde(default)]
    pub constraints: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageBinding {
    #[serde(default)]
    pub shape_name: Option<String>,
    #[serde(default)]
    pub creation_id: Option<String>,
    #[serde(default)]
    pub data_path: Option<String>,
    #[serde(default, rename = "type")]
    pub binding_type: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub max_items: Option<usize>,
    #[serde(default)]
    pub fit: Option<String>,
    #[serde(default)]
    pub chart_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckStep {
    #[serde(default, rename = "use")]
    pub use_template: String,
    #[serde(default)]
    pub data: Option<String>,
    #[serde(default)]
    pub repeat: Option<String>,
    #[serde(default)]
    pub steps: Vec<DeckStep>,
    #[serde(default)]
    pub chunk: Option<RepeatChunk>,
    #[serde(default)]
    pub variants: Vec<DeckStepVariant>,
    #[serde(default)]
    pub overflow: Option<DeckStepOverflow>,
    #[serde(default)]
    pub when: Option<StepCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckStepOverflow {
    #[serde(default)]
    pub strategy: Option<String>,
    pub path: String,
    pub max_items: usize,
    #[serde(default, rename = "as")]
    pub items_as: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum StepCondition {
    Path(String),
    Rule(StepConditionRule),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StepConditionRule {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub exists: Option<bool>,
    #[serde(default)]
    pub truthy: Option<bool>,
    #[serde(default)]
    pub count_min: Option<usize>,
    #[serde(default)]
    pub count_max: Option<usize>,
    #[serde(default)]
    pub count_eq: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckStepVariant {
    #[serde(rename = "use")]
    pub use_template: String,
    #[serde(default)]
    pub when: Option<VariantCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariantCondition {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub count_min: Option<usize>,
    #[serde(default)]
    pub count_max: Option<usize>,
    #[serde(default)]
    pub count_eq: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepeatChunk {
    pub size: usize,
    #[serde(default, rename = "as")]
    pub items_as: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockTemplate {
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub bindings: BTreeMap<String, DocumentBinding>,
    #[serde(default)]
    pub constraints: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentBinding {
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub bookmark: Option<String>,
    #[serde(default)]
    pub content_control: Option<String>,
    #[serde(default)]
    pub data_path: Option<String>,
    #[serde(default, rename = "type")]
    pub binding_type: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub max_items: Option<usize>,
    #[serde(default)]
    pub width_inches: Option<f64>,
    #[serde(default)]
    pub height_inches: Option<f64>,
    #[serde(default)]
    pub block_styles: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentStep {
    #[serde(rename = "use")]
    pub use_template: String,
    #[serde(default)]
    pub data: Option<String>,
    #[serde(default)]
    pub repeat: Option<String>,
    #[serde(default)]
    pub when: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetTemplate {
    #[serde(default)]
    pub source_sheet: Option<String>,
    #[serde(default)]
    pub source_sheet_index: Option<u32>,
    #[serde(default)]
    pub output_sheet_name: Option<String>,
    #[serde(default)]
    pub bindings: BTreeMap<String, SheetBinding>,
    #[serde(default)]
    pub constraints: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SheetBinding {
    #[serde(default)]
    pub cell: Option<String>,
    #[serde(default)]
    pub named_range: Option<String>,
    #[serde(default)]
    pub data_path: Option<String>,
    #[serde(default, rename = "type")]
    pub binding_type: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub max_length: Option<usize>,
    #[serde(default)]
    pub max_items: Option<usize>,
    #[serde(default)]
    pub table_columns: Vec<TableColumn>,
    #[serde(default)]
    pub table_body_row_offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TableColumn {
    pub key: String,
    #[serde(default)]
    pub header: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbookStep {
    #[serde(rename = "use")]
    pub use_template: String,
    #[serde(default)]
    pub data: Option<String>,
    #[serde(default)]
    pub repeat: Option<String>,
    #[serde(default)]
    pub when: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateValidation {
    pub template_id: String,
    pub name: String,
    pub family_id: Option<String>,
    pub role: String,
    pub format: String,
    pub template_type: String,
    pub entry_path: String,
    pub renderer_type: Option<String>,
    pub renderer_entry: Option<String>,
    pub pipeline_step_count: usize,
    pub pipeline_step_ids: Vec<String>,
    pub test_case_count: usize,
    pub test_case_ids: Vec<String>,
    pub page_template_count: usize,
    pub deck_recipe_count: usize,
    pub block_template_count: usize,
    pub document_recipe_count: usize,
    pub sheet_template_count: usize,
    pub workbook_recipe_count: usize,
    pub page_template_ids: Vec<String>,
    pub deck_recipe_ids: Vec<String>,
    pub block_template_ids: Vec<String>,
    pub document_recipe_ids: Vec<String>,
    pub sheet_template_ids: Vec<String>,
    pub workbook_recipe_ids: Vec<String>,
    pub legacy_layout_count: usize,
    pub input_formats: Vec<String>,
    pub input_profile: Option<String>,
    pub input_profile_name: Option<String>,
    pub input_schema: Option<String>,
    pub input_schema_id: Option<String>,
    pub input_builtin_schema: bool,
    pub md_profile: Option<String>,
    pub authoring_instruction_count: usize,
    pub authoring_example_count: usize,
    pub preview: TemplateResolvedPreview,
    pub health: crate::template_health::TemplateHealth,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxTemplateInspection {
    pub input_file: String,
    pub output_file: String,
    pub slide_count: usize,
    pub slides: Vec<PptxSlideInspection>,
    pub draft_manifest: TemplateManifest,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxSlideInspection {
    pub source_slide: u32,
    pub shape_count: usize,
    pub bindable_shape_count: usize,
    pub shapes: Vec<PptxShapeInspection>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxShapeInspection {
    pub name: String,
    pub element_type: String,
    pub suggested_binding_type: String,
    pub suggested_binding_id: String,
    pub bindable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxTemplatePackDraft {
    pub pack_dir: String,
    pub template_file: String,
    pub manifest_file: String,
    pub inspection: PptxTemplateInspection,
    pub validation: TemplateValidation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocxTemplateInspection {
    pub input_file: String,
    pub output_file: String,
    pub placeholder_count: usize,
    pub placeholders: Vec<DocxPlaceholderInspection>,
    pub draft_manifest: TemplateManifest,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocxPlaceholderInspection {
    pub placeholder: String,
    pub occurrence_count: usize,
    pub suggested_binding_type: String,
    pub suggested_binding_id: String,
    pub bindable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocxTemplatePackDraft {
    pub pack_dir: String,
    pub template_file: String,
    pub manifest_file: String,
    pub inspection: DocxTemplateInspection,
    pub validation: TemplateValidation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XlsxTemplateInspection {
    pub input_file: String,
    pub output_file: String,
    pub sheet_count: usize,
    pub named_range_count: usize,
    pub sheets: Vec<XlsxSheetInspection>,
    pub named_ranges: Vec<XlsxNamedRangeInspection>,
    pub draft_manifest: TemplateManifest,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XlsxSheetInspection {
    pub source_sheet: String,
    pub source_sheet_index: u32,
    pub named_range_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XlsxNamedRangeInspection {
    pub name: String,
    pub reference: String,
    pub source_sheet: Option<String>,
    pub start_cell: Option<String>,
    pub suggested_binding_type: String,
    pub suggested_binding_id: String,
    pub bindable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct XlsxTemplatePackDraft {
    pub pack_dir: String,
    pub template_file: String,
    pub manifest_file: String,
    pub inspection: XlsxTemplateInspection,
    pub validation: TemplateValidation,
}

pub fn manifest_path(template_dir: &Path) -> PathBuf {
    template_dir.join("template.manifest.json")
}

pub fn load_manifest(template_dir: &Path) -> Result<TemplateManifest, String> {
    crate::validator::require_dir(template_dir, "template pack")?;
    let path = manifest_path(template_dir);
    crate::validator::require_file(&path, "template.manifest.json")?;
    let raw = fs::read_to_string(&path)
        .map_err(|err| format!("failed to read template.manifest.json: {err}"))?;
    serde_json::from_str(&raw).map_err(|err| format!("invalid template.manifest.json: {err}"))
}

pub fn validate_template_pack(template_dir: &Path) -> Result<TemplateValidation, String> {
    let manifest = load_manifest(template_dir)?;
    let template_type = normalized_template_type(&manifest)?;
    let entry = manifest
        .entry
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|entry| template_dir.join(entry));
    if requires_office_entry(&manifest, &template_type) {
        let entry = entry.as_ref().ok_or_else(|| {
            "template entry is required for declarative Office template packs".to_string()
        })?;
        crate::validator::require_file(entry, "template entry")?;
    }

    let mut warnings = Vec::new();
    match manifest.format.as_str() {
        "pptx" | "docx" | "xlsx" => {}
        other => warnings.push(format!("format '{other}' is not supported by rDeckForge")),
    }
    let input_spec = normalized_input_spec(&manifest);
    if input_spec.formats.is_empty() {
        warnings.push("input.formats is empty; defaulting to json".to_string());
    }
    let input_builtin_schema =
        crate::schema_validation::validate_effective_input_schema(template_dir, &input_spec)?;
    let authoring_context =
        crate::template_authoring::load_template_authoring_context(template_dir, &input_spec)?;
    let input_profile = input_spec.profile.clone();
    let input_profile_name = input_profile
        .as_deref()
        .and_then(crate::profiles::get_profile)
        .map(|profile| profile.name.to_string());
    if let Some(profile) = input_profile.as_deref() {
        if crate::profiles::get_profile(profile).is_none() {
            warnings.push(format!(
                "input.profile '{profile}' is not a built-in profile; supported profiles: {}",
                crate::profiles::profile_ids().join(", ")
            ));
        }
    }
    if input_spec.schema.is_none() && manifest.content_schema.is_some() {
        warnings.push(
            "contentSchema is a legacy field; prefer input.schema or input.schemaId".to_string(),
        );
    }
    validate_renderer_spec(template_dir, &manifest, &template_type, &mut warnings);
    validate_dependency_spec(template_dir, &manifest.dependencies, &mut warnings)?;
    validate_preview_spec(template_dir, manifest.preview.as_ref(), &mut warnings)?;
    validate_test_cases(template_dir, &manifest, &mut warnings)?;
    validate_format_shape(&manifest, &mut warnings);
    if !manifest.layouts.is_empty() {
        warnings.push(
            "layouts is a legacy compatibility field; prefer pageTemplates + deckRecipes"
                .to_string(),
        );
    }
    validate_pptx_recipes(&manifest, &mut warnings);
    validate_document_recipes(&manifest, &mut warnings);
    validate_workbook_recipes(&manifest, &mut warnings);
    let health = match entry.as_deref() {
        Some(entry) if has_declarative_shape(&manifest) => {
            crate::template_health::inspect_template_pack(&manifest, entry)
        }
        _ => crate::template_health::TemplateHealth::unchecked(),
    };
    warnings.extend(health.warnings.clone());
    let normalized_template_dir = if template_dir.is_absolute() {
        template_dir.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(template_dir)
    };
    if normalized_template_dir
        .components()
        .any(|part| part.as_os_str() == "rdeckforge")
    {
        warnings.push("template pack appears to live inside the app repo; keep private templates outside the repo".to_string());
    }

    let input_formats = effective_input_formats(&manifest);
    let preview = resolve_manifest_preview(template_dir, &manifest);
    let role = normalized_template_role(&manifest);

    Ok(TemplateValidation {
        template_id: manifest.template_id,
        name: manifest.name,
        family_id: normalize_optional_id(manifest.family_id),
        role,
        format: manifest.format,
        template_type,
        entry_path: entry
            .as_deref()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        renderer_type: manifest
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.renderer_type.clone()),
        renderer_entry: manifest
            .renderer
            .as_ref()
            .and_then(|renderer| renderer.entry.clone()),
        pipeline_step_count: manifest.pipeline.len(),
        pipeline_step_ids: manifest
            .pipeline
            .iter()
            .enumerate()
            .map(|(index, step)| {
                step.id
                    .clone()
                    .unwrap_or_else(|| format!("step_{}", index + 1))
            })
            .collect(),
        test_case_count: manifest.test_cases.len(),
        test_case_ids: manifest
            .test_cases
            .iter()
            .map(|test_case| test_case.id.clone())
            .collect(),
        page_template_count: manifest.page_templates.len(),
        deck_recipe_count: manifest.deck_recipes.len(),
        block_template_count: manifest.block_templates.len(),
        document_recipe_count: manifest.document_recipes.len(),
        sheet_template_count: manifest.sheet_templates.len(),
        workbook_recipe_count: manifest.workbook_recipes.len(),
        page_template_ids: manifest.page_templates.keys().cloned().collect(),
        deck_recipe_ids: manifest.deck_recipes.keys().cloned().collect(),
        block_template_ids: manifest.block_templates.keys().cloned().collect(),
        document_recipe_ids: manifest.document_recipes.keys().cloned().collect(),
        sheet_template_ids: manifest.sheet_templates.keys().cloned().collect(),
        workbook_recipe_ids: manifest.workbook_recipes.keys().cloned().collect(),
        legacy_layout_count: manifest.layouts.len(),
        input_formats,
        input_profile,
        input_profile_name,
        input_schema: input_spec.schema,
        input_schema_id: input_spec.schema_id,
        input_builtin_schema,
        md_profile: input_spec.md_profile,
        authoring_instruction_count: authoring_context.instructions.len(),
        authoring_example_count: authoring_context.examples.len(),
        preview,
        health,
        warnings,
    })
}

pub fn normalized_template_role(manifest: &TemplateManifest) -> String {
    manifest
        .role
        .as_deref()
        .map(normalize_role)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default_role_for_format(&manifest.format).to_string())
}

fn normalize_optional_id(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_role(value: &str) -> String {
    value
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '_'
            }
        })
        .collect::<String>()
        .split('_')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("_")
}

fn default_role_for_format(format: &str) -> &'static str {
    match format {
        "docx" => "document",
        "xlsx" => "workbook",
        _ => "slides",
    }
}

pub fn template_preview_for_dir(template_dir: &Path) -> TemplateResolvedPreview {
    load_manifest(template_dir)
        .map(|manifest| resolve_manifest_preview(template_dir, &manifest))
        .unwrap_or_default()
}

pub fn normalized_input_spec(manifest: &TemplateManifest) -> TemplateInputSpec {
    let mut input = manifest.input.clone().unwrap_or_default();
    crate::profiles::apply_profile_defaults(&mut input);
    if input.schema_id.is_none() {
        input.schema_id = manifest.content_schema.clone();
    }
    if input.formats.is_empty() {
        input.formats = vec!["json".to_string()];
    }
    input.formats = input
        .formats
        .into_iter()
        .map(|format| crate::content_source::normalize_format(&format))
        .collect();
    input
}

pub fn effective_input_formats(manifest: &TemplateManifest) -> Vec<String> {
    normalized_input_spec(manifest).formats
}

pub fn normalized_template_type(manifest: &TemplateManifest) -> Result<String, String> {
    let value = manifest
        .template_type
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_ascii_lowercase())
        .unwrap_or_else(|| {
            if manifest.renderer.is_some() || !manifest.pipeline.is_empty() {
                if has_declarative_shape(manifest) {
                    "hybrid".to_string()
                } else {
                    "script".to_string()
                }
            } else {
                "declarative".to_string()
            }
        });
    match value.as_str() {
        "declarative" | "script" | "hybrid" => Ok(value),
        other => Err(format!(
            "templateType '{other}' is not supported; expected declarative, script, or hybrid"
        )),
    }
}

fn requires_office_entry(manifest: &TemplateManifest, template_type: &str) -> bool {
    template_type == "declarative" || has_declarative_shape(manifest)
}

fn has_declarative_shape(manifest: &TemplateManifest) -> bool {
    !manifest.page_templates.is_empty()
        || !manifest.deck_recipes.is_empty()
        || !manifest.block_templates.is_empty()
        || !manifest.document_recipes.is_empty()
        || !manifest.sheet_templates.is_empty()
        || !manifest.workbook_recipes.is_empty()
        || !manifest.layouts.is_empty()
}

fn validate_renderer_spec(
    template_dir: &Path,
    manifest: &TemplateManifest,
    template_type: &str,
    warnings: &mut Vec<String>,
) {
    if template_type == "script" || template_type == "hybrid" {
        if manifest.renderer.is_none() && manifest.pipeline.is_empty() {
            warnings.push(format!(
                "templateType '{template_type}' should declare renderer or pipeline"
            ));
        }
    }

    if let Some(renderer) = &manifest.renderer {
        if renderer
            .renderer_type
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            warnings.push("renderer.type is empty".to_string());
        }
        match renderer
            .entry
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            Some(entry) => {
                let entry_path = template_dir.join(entry);
                if !entry_path.is_file() {
                    warnings.push(format!(
                        "renderer.entry was not found: {}",
                        entry_path.display()
                    ));
                }
            }
            None => warnings.push("renderer.entry is empty".to_string()),
        }
    }

    for (index, step) in manifest.pipeline.iter().enumerate() {
        let step_label = step
            .id
            .clone()
            .unwrap_or_else(|| format!("step_{}", index + 1));
        if step
            .step_type
            .as_deref()
            .is_none_or(|value| value.trim().is_empty())
        {
            warnings.push(format!("pipeline.{step_label}.type is empty"));
        }
        if let Some(entry) = step
            .entry
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            let entry_path = template_dir.join(entry);
            if !entry_path.is_file() {
                warnings.push(format!(
                    "pipeline.{step_label}.entry was not found: {}",
                    entry_path.display()
                ));
            }
        }
    }
}

fn validate_dependency_spec(
    template_dir: &Path,
    dependencies: &TemplateDependencySpec,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let mut file_paths = BTreeSet::new();
    for dependency in &dependencies.files {
        let value = dependency.path.trim();
        if value.is_empty() {
            warnings.push("dependencies.files contains an empty path".to_string());
            continue;
        }
        let path = Path::new(value);
        if path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
        {
            return Err(format!(
                "dependency file must stay inside the template pack: {value}"
            ));
        }
        if !file_paths.insert(value.to_string()) {
            warnings.push(format!(
                "dependency file is declared more than once: {value}"
            ));
        }
        if let Some(kind) = dependency.kind.as_deref()
            && !matches!(
                kind,
                "python_requirements" | "node_package" | "config" | "resource" | "other"
            )
        {
            warnings.push(format!(
                "dependency file '{value}' uses unsupported kind '{kind}'"
            ));
        }
        if dependency.required && !template_dir.join(path).is_file() {
            warnings.push(format!("required dependency file was not found: {value}"));
        }
    }

    let mut command_ids = BTreeSet::new();
    for dependency in &dependencies.commands {
        let id = dependency.id.trim();
        if id.is_empty() {
            warnings.push("dependencies.commands contains an empty id".to_string());
        } else if !command_ids.insert(id.to_string()) {
            warnings.push(format!(
                "dependency command id is declared more than once: {id}"
            ));
        }
        if dependency.command.trim().is_empty() {
            warnings.push(format!("dependency command '{id}' has an empty command"));
        }
        validate_dependency_platforms(
            &dependency.platforms,
            &format!("dependency command '{id}'"),
            warnings,
        );
    }

    let mut environment_names = BTreeSet::new();
    for dependency in &dependencies.environment {
        let name = dependency.name.trim();
        if name.is_empty() {
            warnings.push("dependencies.environment contains an empty name".to_string());
        } else if !environment_names.insert(name.to_string()) {
            warnings.push(format!(
                "dependency environment variable is declared more than once: {name}"
            ));
        }
        validate_dependency_platforms(
            &dependency.platforms,
            &format!("dependency environment variable '{name}'"),
            warnings,
        );
    }
    Ok(())
}

fn validate_dependency_platforms(platforms: &[String], label: &str, warnings: &mut Vec<String>) {
    for platform in platforms {
        if !matches!(platform.as_str(), "macos" | "windows" | "linux") {
            warnings.push(format!(
                "{label} uses unsupported platform '{platform}'; expected macos, windows, or linux"
            ));
        }
    }
}

fn validate_test_cases(
    template_dir: &Path,
    manifest: &TemplateManifest,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let mut ids = BTreeSet::new();
    for test_case in &manifest.test_cases {
        let id = test_case.id.trim();
        if id.is_empty() {
            warnings.push("tests contains an empty id".to_string());
        } else if !ids.insert(id.to_string()) {
            warnings.push(format!("template test id is declared more than once: {id}"));
        }
        let input = safe_template_relative_path(&test_case.input, &format!("tests.{id}.input"))?;
        if !template_dir.join(&input).is_file() {
            warnings.push(format!(
                "template test '{id}' input was not found: {}",
                test_case.input
            ));
        }
        if let Some(output_name) = test_case.output_name.as_deref() {
            let output =
                safe_template_relative_path(output_name, &format!("tests.{id}.outputName"))?;
            if output.components().count() != 1 {
                return Err(format!(
                    "tests.{id}.outputName must be a file name without directories"
                ));
            }
        }
        if let Some(recipe) = test_case.recipe.as_deref() {
            let exists = match manifest.format.as_str() {
                "pptx" => manifest.deck_recipes.contains_key(recipe),
                "docx" => manifest.document_recipes.contains_key(recipe),
                "xlsx" => manifest.workbook_recipes.contains_key(recipe),
                _ => false,
            };
            if !exists && manifest.renderer.is_none() {
                warnings.push(format!(
                    "template test '{id}' references unknown recipe '{recipe}'"
                ));
            }
        }
        if test_case
            .assertions
            .min_slides
            .zip(test_case.assertions.max_slides)
            .is_some_and(|(minimum, maximum)| minimum > maximum)
        {
            warnings.push(format!(
                "template test '{id}' minSlides is greater than maxSlides"
            ));
        }
        if test_case
            .assertions
            .min_sheets
            .zip(test_case.assertions.max_sheets)
            .is_some_and(|(minimum, maximum)| minimum > maximum)
        {
            warnings.push(format!(
                "template test '{id}' minSheets is greater than maxSheets"
            ));
        }
        if manifest.format != "pptx"
            && (test_case.assertions.min_slides.is_some()
                || test_case.assertions.max_slides.is_some())
        {
            warnings.push(format!(
                "template test '{id}' uses slide assertions for a non-PPTX template"
            ));
        }
        if manifest.format != "xlsx"
            && (test_case.assertions.min_sheets.is_some()
                || test_case.assertions.max_sheets.is_some())
        {
            warnings.push(format!(
                "template test '{id}' uses sheet assertions for a non-XLSX template"
            ));
        }
        for entry in &test_case.assertions.required_zip_entries {
            safe_template_relative_path(entry, &format!("tests.{id}.requiredZipEntries"))?;
        }
        validate_text_assertions(
            id,
            "requiredText",
            &test_case.assertions.required_text,
            warnings,
        );
        validate_text_assertions(
            id,
            "forbiddenText",
            &test_case.assertions.forbidden_text,
            warnings,
        );
        if test_case.assertions.validate_package == Some(false)
            && (!test_case.assertions.required_text.is_empty()
                || !test_case.assertions.forbidden_text.is_empty())
        {
            warnings.push(format!(
                "template test '{id}' declares text assertions while validatePackage is false"
            ));
        }
    }
    Ok(())
}

fn validate_preview_spec(
    template_dir: &Path,
    preview: Option<&TemplatePreviewSpec>,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let Some(preview) = preview else {
        return Ok(());
    };
    if let Some(cover) = preview.cover.as_deref() {
        validate_preview_asset(template_dir, "preview.cover", cover, warnings)?;
    }
    for (index, slide) in preview.slides.iter().enumerate() {
        let label = format!("preview.slides[{index}].image");
        validate_preview_asset(template_dir, &label, &slide.image, warnings)?;
        if slide
            .title
            .as_deref()
            .is_some_and(|title| title.trim().is_empty())
        {
            warnings.push(format!("preview.slides[{index}].title is empty"));
        }
    }
    Ok(())
}

fn validate_preview_asset(
    template_dir: &Path,
    label: &str,
    value: &str,
    warnings: &mut Vec<String>,
) -> Result<(), String> {
    let relative = safe_template_relative_path(value, label)?;
    if !template_dir.join(&relative).is_file() {
        warnings.push(format!("{label} was not found: {value}"));
    }
    Ok(())
}

fn resolve_manifest_preview(
    template_dir: &Path,
    manifest: &TemplateManifest,
) -> TemplateResolvedPreview {
    let Some(preview) = manifest.preview.as_ref() else {
        return TemplateResolvedPreview::default();
    };
    let cover = preview
        .cover
        .as_deref()
        .and_then(|value| resolve_existing_preview_path(template_dir, value));
    let slides = preview
        .slides
        .iter()
        .filter_map(|slide| {
            resolve_existing_preview_path(template_dir, &slide.image).map(|image| {
                TemplatePreviewSlide {
                    title: slide.title.clone(),
                    image,
                }
            })
        })
        .collect::<Vec<_>>();
    let cover = cover.or_else(|| slides.first().map(|slide| slide.image.clone()));
    TemplateResolvedPreview { cover, slides }
}

fn resolve_existing_preview_path(template_dir: &Path, value: &str) -> Option<String> {
    let relative = safe_template_relative_path(value, "preview").ok()?;
    let path = template_dir.join(relative);
    path.is_file().then(|| path.display().to_string())
}

fn validate_text_assertions(
    test_id: &str,
    field: &str,
    values: &[String],
    warnings: &mut Vec<String>,
) {
    let mut unique = BTreeSet::new();
    for value in values {
        let value = value.trim();
        if value.is_empty() {
            warnings.push(format!(
                "template test '{test_id}' {field} contains an empty value"
            ));
        } else if !unique.insert(value) {
            warnings.push(format!(
                "template test '{test_id}' {field} contains a duplicate value: {value}"
            ));
        }
    }
}

fn safe_template_relative_path(value: &str, label: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value.trim());
    if value.trim().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::ParentDir
                    | std::path::Component::RootDir
                    | std::path::Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "{label} must be a relative path inside the template pack: {value}"
        ));
    }
    Ok(path)
}

fn validate_format_shape(manifest: &TemplateManifest, warnings: &mut Vec<String>) {
    if manifest
        .template_type
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("script"))
        && !has_declarative_shape(manifest)
    {
        return;
    }
    match manifest.format.as_str() {
        "pptx" => {
            if manifest.page_templates.is_empty() && manifest.layouts.is_empty() {
                warnings.push(
                    "pageTemplates is empty; PPTX render jobs will not have page-level binding targets"
                        .to_string(),
                );
            }
            if manifest.deck_recipes.is_empty() {
                warnings.push(
                    "deckRecipes is empty; PPTX render jobs must choose pages manually".to_string(),
                );
            }
        }
        "docx" => {
            if manifest.block_templates.is_empty() {
                warnings.push(
                    "blockTemplates is empty; DOCX template packs need reusable document blocks"
                        .to_string(),
                );
            }
            if manifest.document_recipes.is_empty() {
                warnings.push(
                    "documentRecipes is empty; DOCX template packs need a document recipe"
                        .to_string(),
                );
            }
        }
        "xlsx" => {
            if manifest.sheet_templates.is_empty() {
                warnings.push(
                    "sheetTemplates is empty; XLSX template packs need reusable sheet templates"
                        .to_string(),
                );
            }
            if manifest.workbook_recipes.is_empty() {
                warnings.push(
                    "workbookRecipes is empty; XLSX template packs need a workbook recipe"
                        .to_string(),
                );
            }
        }
        _ => {}
    }
}

fn validate_pptx_recipes(manifest: &TemplateManifest, warnings: &mut Vec<String>) {
    for (recipe_id, steps) in &manifest.deck_recipes {
        validate_pptx_recipe_steps(manifest, recipe_id, steps, warnings);
    }
}

fn validate_pptx_recipe_steps(
    manifest: &TemplateManifest,
    recipe_id: &str,
    steps: &[DeckStep],
    warnings: &mut Vec<String>,
) {
    for step in steps {
        let step_label = deck_step_label(step);
        if !step.steps.is_empty() {
            if !step.use_template.trim().is_empty() {
                warnings.push(format!(
                    "deckRecipes.{recipe_id} step '{step_label}' declares both use and steps; put the page template in the first child step"
                ));
            }
            if step.chunk.is_some() || step.overflow.is_some() || !step.variants.is_empty() {
                warnings.push(format!(
                    "deckRecipes.{recipe_id} step '{step_label}' is a group step; put chunk, overflow, or variants on child steps"
                ));
            }
        }
        if step.use_template.trim().is_empty() {
            if step.steps.is_empty() {
                warnings.push(format!(
                    "deckRecipes.{recipe_id} contains a step without use or steps"
                ));
            }
        } else if !manifest.page_templates.contains_key(&step.use_template) {
            warnings.push(format!(
                "deckRecipes.{recipe_id} references missing pageTemplate '{}'",
                step.use_template
            ));
        }
        if let Some(condition) = &step.when {
            validate_step_condition(recipe_id, &step_label, condition, warnings);
        }
        for variant in &step.variants {
            if !manifest.page_templates.contains_key(&variant.use_template) {
                warnings.push(format!(
                    "deckRecipes.{recipe_id} step '{step_label}' variant references missing pageTemplate '{}'",
                    variant.use_template
                ));
            }
            validate_variant_condition(recipe_id, &step_label, variant, warnings);
        }
        if let Some(chunk) = &step.chunk {
            if step.repeat.is_none() {
                warnings.push(format!(
                    "deckRecipes.{recipe_id} step '{step_label}' declares chunk without repeat"
                ));
            }
            if chunk.size == 0 {
                warnings.push(format!(
                    "deckRecipes.{recipe_id} step '{step_label}' declares chunk.size 0"
                ));
            }
        }
        if let Some(overflow) = &step.overflow {
            validate_step_overflow(recipe_id, &step_label, overflow, warnings);
        }
        if !step.steps.is_empty() {
            validate_pptx_recipe_steps(manifest, recipe_id, &step.steps, warnings);
        }
    }
}

fn validate_step_overflow(
    recipe_id: &str,
    step_template: &str,
    overflow: &DeckStepOverflow,
    warnings: &mut Vec<String>,
) {
    if overflow.max_items == 0 {
        warnings.push(format!(
            "deckRecipes.{recipe_id} step '{step_template}' declares overflow.maxItems 0"
        ));
    }
    if overflow.path.trim().is_empty() {
        warnings.push(format!(
            "deckRecipes.{recipe_id} step '{step_template}' declares empty overflow.path"
        ));
    }
    if overflow
        .strategy
        .as_deref()
        .is_some_and(|strategy| strategy != "split")
    {
        warnings.push(format!(
            "deckRecipes.{recipe_id} step '{step_template}' declares unsupported overflow.strategy '{}'; supported: split",
            overflow.strategy.as_deref().unwrap_or_default()
        ));
    }
}

fn validate_step_condition(
    recipe_id: &str,
    step_label: &str,
    condition: &StepCondition,
    warnings: &mut Vec<String>,
) {
    match condition {
        StepCondition::Path(path) => {
            if path.trim().is_empty() {
                warnings.push(format!(
                    "deckRecipes.{recipe_id} step '{step_label}' declares empty when path"
                ));
            }
        }
        StepCondition::Rule(rule) => {
            if rule
                .path
                .as_deref()
                .is_some_and(|path| path.trim().is_empty())
            {
                warnings.push(format!(
                    "deckRecipes.{recipe_id} step '{step_label}' declares empty when.path"
                ));
            }
            if let (Some(min), Some(max)) = (rule.count_min, rule.count_max) {
                if min > max {
                    warnings.push(format!(
                        "deckRecipes.{recipe_id} step '{step_label}' when has countMin greater than countMax"
                    ));
                }
            }
        }
    }
}

fn validate_variant_condition(
    recipe_id: &str,
    step_template: &str,
    variant: &DeckStepVariant,
    warnings: &mut Vec<String>,
) {
    let Some(condition) = &variant.when else {
        return;
    };
    if let (Some(min), Some(max)) = (condition.count_min, condition.count_max) {
        if min > max {
            warnings.push(format!(
                "deckRecipes.{recipe_id} step '{step_template}' variant '{}' has countMin greater than countMax",
                variant.use_template
            ));
        }
    }
}

fn validate_document_recipes(manifest: &TemplateManifest, warnings: &mut Vec<String>) {
    for (recipe_id, steps) in &manifest.document_recipes {
        for step in steps {
            if !manifest.block_templates.contains_key(&step.use_template) {
                warnings.push(format!(
                    "documentRecipes.{recipe_id} references missing blockTemplate '{}'",
                    step.use_template
                ));
            }
        }
    }
}

fn validate_workbook_recipes(manifest: &TemplateManifest, warnings: &mut Vec<String>) {
    for (recipe_id, steps) in &manifest.workbook_recipes {
        for step in steps {
            if !manifest.sheet_templates.contains_key(&step.use_template) {
                warnings.push(format!(
                    "workbookRecipes.{recipe_id} references missing sheetTemplate '{}'",
                    step.use_template
                ));
            }
        }
    }
}

pub fn resolve_recipe_id(
    manifest: &TemplateManifest,
    requested: Option<&str>,
) -> Result<Option<String>, String> {
    if let Some(recipe) = requested {
        if manifest.deck_recipes.contains_key(recipe) {
            return Ok(Some(recipe.to_string()));
        }
        return Err(format!("deck recipe not found: {recipe}"));
    }

    Ok(manifest.deck_recipes.keys().next().cloned())
}

pub fn validate_deck_recipe(manifest: &TemplateManifest, recipe_id: &str) -> Result<(), String> {
    let steps = manifest
        .deck_recipes
        .get(recipe_id)
        .ok_or_else(|| format!("deck recipe not found: {recipe_id}"))?;
    if steps.is_empty() {
        return Err(format!("deck recipe is empty: {recipe_id}"));
    }
    validate_deck_recipe_steps(manifest, recipe_id, steps)?;
    Ok(())
}

fn validate_deck_recipe_steps(
    manifest: &TemplateManifest,
    recipe_id: &str,
    steps: &[DeckStep],
) -> Result<(), String> {
    for step in steps {
        let step_label = deck_step_label(step);
        if !step.steps.is_empty() {
            if !step.use_template.trim().is_empty() {
                return Err(format!(
                    "deck recipe '{recipe_id}' step '{step_label}' declares both use and steps; put the page template in the first child step"
                ));
            }
            if step.chunk.is_some() || step.overflow.is_some() || !step.variants.is_empty() {
                return Err(format!(
                    "deck recipe '{recipe_id}' step '{step_label}' is a group step; put chunk, overflow, or variants on child steps"
                ));
            }
        }
        if step.use_template.trim().is_empty() {
            if step.steps.is_empty() {
                return Err(format!(
                    "deck recipe '{recipe_id}' contains a step without use or steps"
                ));
            }
        } else if !manifest.page_templates.contains_key(&step.use_template) {
            return Err(format!(
                "deck recipe '{recipe_id}' references missing page template '{}'",
                step.use_template
            ));
        }
        if let Some(condition) = &step.when {
            validate_deck_step_condition(recipe_id, &step_label, condition)?;
        }
        for variant in &step.variants {
            if !manifest.page_templates.contains_key(&variant.use_template) {
                return Err(format!(
                    "deck recipe '{recipe_id}' step '{step_label}' variant references missing page template '{}'",
                    variant.use_template
                ));
            }
            if let Some(condition) = &variant.when {
                if let (Some(min), Some(max)) = (condition.count_min, condition.count_max) {
                    if min > max {
                        return Err(format!(
                            "deck recipe '{recipe_id}' step '{step_label}' variant '{}' has countMin greater than countMax",
                            variant.use_template
                        ));
                    }
                }
            }
        }
        if let Some(chunk) = &step.chunk {
            if step.repeat.is_none() {
                return Err(format!(
                    "deck recipe '{recipe_id}' step '{step_label}' declares chunk without repeat"
                ));
            }
            if chunk.size == 0 {
                return Err(format!(
                    "deck recipe '{recipe_id}' step '{step_label}' declares chunk.size 0"
                ));
            }
        }
        if let Some(overflow) = &step.overflow {
            if overflow.max_items == 0 {
                return Err(format!(
                    "deck recipe '{recipe_id}' step '{step_label}' declares overflow.maxItems 0"
                ));
            }
            if overflow.path.trim().is_empty() {
                return Err(format!(
                    "deck recipe '{recipe_id}' step '{step_label}' declares empty overflow.path"
                ));
            }
            if overflow
                .strategy
                .as_deref()
                .is_some_and(|strategy| strategy != "split")
            {
                return Err(format!(
                    "deck recipe '{recipe_id}' step '{step_label}' declares unsupported overflow.strategy '{}'; supported: split",
                    overflow.strategy.as_deref().unwrap_or_default()
                ));
            }
        }
        if !step.steps.is_empty() {
            validate_deck_recipe_steps(manifest, recipe_id, &step.steps)?;
        }
    }
    Ok(())
}

fn validate_deck_step_condition(
    recipe_id: &str,
    step_label: &str,
    condition: &StepCondition,
) -> Result<(), String> {
    match condition {
        StepCondition::Path(path) => {
            if path.trim().is_empty() {
                return Err(format!(
                    "deck recipe '{recipe_id}' step '{step_label}' declares empty when path"
                ));
            }
        }
        StepCondition::Rule(rule) => {
            if rule
                .path
                .as_deref()
                .is_some_and(|path| path.trim().is_empty())
            {
                return Err(format!(
                    "deck recipe '{recipe_id}' step '{step_label}' declares empty when.path"
                ));
            }
            if let (Some(min), Some(max)) = (rule.count_min, rule.count_max) {
                if min > max {
                    return Err(format!(
                        "deck recipe '{recipe_id}' step '{step_label}' when has countMin greater than countMax"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn deck_step_label(step: &DeckStep) -> String {
    if step.use_template.trim().is_empty() {
        "group".to_string()
    } else {
        step.use_template.clone()
    }
}

pub fn resolve_document_recipe_id(
    manifest: &TemplateManifest,
    requested: Option<&str>,
) -> Result<Option<String>, String> {
    if let Some(recipe) = requested {
        if manifest.document_recipes.contains_key(recipe) {
            return Ok(Some(recipe.to_string()));
        }
        return Err(format!("document recipe not found: {recipe}"));
    }

    Ok(manifest.document_recipes.keys().next().cloned())
}

pub fn validate_document_recipe(
    manifest: &TemplateManifest,
    recipe_id: &str,
) -> Result<(), String> {
    let steps = manifest
        .document_recipes
        .get(recipe_id)
        .ok_or_else(|| format!("document recipe not found: {recipe_id}"))?;
    if steps.is_empty() {
        return Err(format!("document recipe is empty: {recipe_id}"));
    }
    for step in steps {
        if !manifest.block_templates.contains_key(&step.use_template) {
            return Err(format!(
                "document recipe '{recipe_id}' references missing block template '{}'",
                step.use_template
            ));
        }
    }
    Ok(())
}

pub fn resolve_workbook_recipe_id(
    manifest: &TemplateManifest,
    requested: Option<&str>,
) -> Result<Option<String>, String> {
    if let Some(recipe) = requested {
        if manifest.workbook_recipes.contains_key(recipe) {
            return Ok(Some(recipe.to_string()));
        }
        return Err(format!("workbook recipe not found: {recipe}"));
    }

    Ok(manifest.workbook_recipes.keys().next().cloned())
}

pub fn validate_workbook_recipe(
    manifest: &TemplateManifest,
    recipe_id: &str,
) -> Result<(), String> {
    let steps = manifest
        .workbook_recipes
        .get(recipe_id)
        .ok_or_else(|| format!("workbook recipe not found: {recipe_id}"))?;
    if steps.is_empty() {
        return Err(format!("workbook recipe is empty: {recipe_id}"));
    }
    for step in steps {
        if !manifest.sheet_templates.contains_key(&step.use_template) {
            return Err(format!(
                "workbook recipe '{recipe_id}' references missing sheet template '{}'",
                step.use_template
            ));
        }
    }
    Ok(())
}

pub fn inspect_docx(input: &Path, out: &Path) -> Result<DocxTemplateInspection, String> {
    crate::validator::require_file(input, "template.docx")?;
    let file_name = office_file_name(input, "template.docx");
    let file_stem = office_file_stem(input);
    let (placeholders, warnings) = inspect_docx_placeholders(input)?;
    let bindings = placeholders
        .iter()
        .filter(|placeholder| placeholder.bindable)
        .map(|placeholder| {
            (
                placeholder.suggested_binding_id.clone(),
                DocumentBinding {
                    placeholder: Some(placeholder.placeholder.clone()),
                    bookmark: None,
                    content_control: None,
                    data_path: Some(format!("$.{}", placeholder.suggested_binding_id)),
                    binding_type: Some(placeholder.suggested_binding_type.clone()),
                    required: false,
                    max_length: None,
                    max_items: None,
                    width_inches: None,
                    height_inches: None,
                    block_styles: BTreeMap::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let block_templates = BTreeMap::from([(
        "document".to_string(),
        BlockTemplate {
            description: Some("Draft generated from DOCX placeholders".to_string()),
            bindings,
            constraints: BTreeMap::new(),
        },
    )]);
    let document_recipes = BTreeMap::from([(
        "standard_document".to_string(),
        vec![DocumentStep {
            use_template: "document".to_string(),
            data: Some("$".to_string()),
            repeat: None,
            when: None,
        }],
    )]);
    let manifest = TemplateManifest {
        schema_version: "1.0".to_string(),
        template_id: sanitize_template_id(&file_stem),
        name: file_stem,
        family_id: None,
        role: Some("document".to_string()),
        format: "docx".to_string(),
        template_type: Some("declarative".to_string()),
        entry: Some(file_name),
        content_schema: None,
        input: Some(default_draft_input()),
        preview: None,
        render_mode: None,
        renderer: None,
        pipeline: Vec::new(),
        dependencies: TemplateDependencySpec::default(),
        test_cases: Vec::new(),
        page_templates: BTreeMap::new(),
        deck_recipes: BTreeMap::new(),
        block_templates,
        document_recipes,
        sheet_templates: BTreeMap::new(),
        workbook_recipes: BTreeMap::new(),
        layouts: BTreeMap::new(),
    };
    write_draft_manifest(out, &manifest)?;
    Ok(DocxTemplateInspection {
        input_file: absolutize(input).display().to_string(),
        output_file: absolutize(out).display().to_string(),
        placeholder_count: placeholders.len(),
        placeholders,
        draft_manifest: manifest,
        warnings,
    })
}

pub fn create_docx_template_pack_draft(
    input: &Path,
    out_dir: &Path,
) -> Result<DocxTemplatePackDraft, String> {
    let (template_file, manifest_file) = prepare_office_draft_pack(input, out_dir, "docx")?;
    let mut inspection = inspect_docx(&template_file, &manifest_file)?;
    apply_draft_identity(&mut inspection.draft_manifest, input, "docx");
    write_draft_manifest(&manifest_file, &inspection.draft_manifest)?;
    let validation = validate_template_pack(out_dir)?;
    Ok(DocxTemplatePackDraft {
        pack_dir: absolutize(out_dir).display().to_string(),
        template_file: absolutize(&template_file).display().to_string(),
        manifest_file: absolutize(&manifest_file).display().to_string(),
        inspection,
        validation,
    })
}

pub fn inspect_xlsx(input: &Path, out: &Path) -> Result<XlsxTemplateInspection, String> {
    crate::validator::require_file(input, "template.xlsx")?;
    let file_name = office_file_name(input, "template.xlsx");
    let file_stem = office_file_stem(input);
    let (sheet_names, named_ranges, warnings) = inspect_xlsx_workbook(input)?;
    let sheets = sheet_names
        .iter()
        .enumerate()
        .map(|(index, source_sheet)| XlsxSheetInspection {
            source_sheet: source_sheet.clone(),
            source_sheet_index: index as u32 + 1,
            named_range_count: named_ranges
                .iter()
                .filter(|range| range.bindable && range.source_sheet.as_ref() == Some(source_sheet))
                .count(),
        })
        .collect::<Vec<_>>();
    let sheet_templates = sheets
        .iter()
        .map(|sheet| {
            let bindings = named_ranges
                .iter()
                .filter(|range| {
                    range.bindable && range.source_sheet.as_ref() == Some(&sheet.source_sheet)
                })
                .map(|range| {
                    (
                        range.suggested_binding_id.clone(),
                        SheetBinding {
                            cell: None,
                            named_range: Some(range.name.clone()),
                            data_path: Some(format!("$.{}", range.suggested_binding_id)),
                            binding_type: Some(range.suggested_binding_type.clone()),
                            required: false,
                            max_length: None,
                            max_items: None,
                            table_columns: Vec::new(),
                            table_body_row_offset: None,
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>();
            (
                format!("sheet_{:02}", sheet.source_sheet_index),
                SheetTemplate {
                    source_sheet: Some(sheet.source_sheet.clone()),
                    source_sheet_index: Some(sheet.source_sheet_index),
                    output_sheet_name: Some(sheet.source_sheet.clone()),
                    bindings,
                    constraints: BTreeMap::new(),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let workbook_recipes = BTreeMap::from([(
        "standard_workbook".to_string(),
        sheets
            .iter()
            .map(|sheet| WorkbookStep {
                use_template: format!("sheet_{:02}", sheet.source_sheet_index),
                data: Some("$".to_string()),
                repeat: None,
                when: None,
            })
            .collect::<Vec<_>>(),
    )]);
    let manifest = TemplateManifest {
        schema_version: "1.0".to_string(),
        template_id: sanitize_template_id(&file_stem),
        name: file_stem,
        family_id: None,
        role: Some("workbook".to_string()),
        format: "xlsx".to_string(),
        template_type: Some("declarative".to_string()),
        entry: Some(file_name),
        content_schema: None,
        input: Some(default_draft_input()),
        preview: None,
        render_mode: None,
        renderer: None,
        pipeline: Vec::new(),
        dependencies: TemplateDependencySpec::default(),
        test_cases: Vec::new(),
        page_templates: BTreeMap::new(),
        deck_recipes: BTreeMap::new(),
        block_templates: BTreeMap::new(),
        document_recipes: BTreeMap::new(),
        sheet_templates,
        workbook_recipes,
        layouts: BTreeMap::new(),
    };
    write_draft_manifest(out, &manifest)?;
    Ok(XlsxTemplateInspection {
        input_file: absolutize(input).display().to_string(),
        output_file: absolutize(out).display().to_string(),
        sheet_count: sheets.len(),
        named_range_count: named_ranges.len(),
        sheets,
        named_ranges,
        draft_manifest: manifest,
        warnings,
    })
}

pub fn create_xlsx_template_pack_draft(
    input: &Path,
    out_dir: &Path,
) -> Result<XlsxTemplatePackDraft, String> {
    let (template_file, manifest_file) = prepare_office_draft_pack(input, out_dir, "xlsx")?;
    let mut inspection = inspect_xlsx(&template_file, &manifest_file)?;
    apply_draft_identity(&mut inspection.draft_manifest, input, "xlsx");
    write_draft_manifest(&manifest_file, &inspection.draft_manifest)?;
    let validation = validate_template_pack(out_dir)?;
    Ok(XlsxTemplatePackDraft {
        pack_dir: absolutize(out_dir).display().to_string(),
        template_file: absolutize(&template_file).display().to_string(),
        manifest_file: absolutize(&manifest_file).display().to_string(),
        inspection,
        validation,
    })
}

fn inspect_docx_placeholders(
    input: &Path,
) -> Result<(Vec<DocxPlaceholderInspection>, Vec<String>), String> {
    let document = read_office_zip_text(input, "word/document.xml", "DOCX")?;
    let visible_text = collect_xml_text(&document, "w:t");
    let visible_placeholders = collect_placeholders(&visible_text);
    let raw_placeholders = collect_placeholders(&document)
        .into_iter()
        .collect::<BTreeSet<_>>();
    let mut warnings = Vec::new();
    let mut used_ids = BTreeMap::new();
    let placeholders = visible_placeholders
        .into_iter()
        .map(|placeholder| {
            let bindable = raw_placeholders.contains(&placeholder);
            if !bindable {
                warnings.push(format!(
                    "placeholder '{placeholder}' is split across Word runs; keep it in one text run before using the generated binding"
                ));
            }
            let inner = placeholder
                .strip_prefix("{{")
                .and_then(|value| value.strip_suffix("}}"))
                .unwrap_or("field")
                .trim();
            let base_id = sanitize_field_id(inner, "field");
            let suggested_binding_id = dedupe_binding_id(base_id, &mut used_ids);
            DocxPlaceholderInspection {
                occurrence_count: visible_text.matches(&placeholder).count(),
                suggested_binding_type: infer_document_binding_type(inner),
                suggested_binding_id,
                placeholder,
                bindable,
            }
        })
        .collect::<Vec<_>>();
    if placeholders.is_empty() {
        warnings.push(
            "no {{placeholder}} tokens were found in word/document.xml; add placeholders or edit the generated manifest with bookmark/content-control bindings"
                .to_string(),
        );
    }
    Ok((placeholders, warnings))
}

fn inspect_xlsx_workbook(
    input: &Path,
) -> Result<(Vec<String>, Vec<XlsxNamedRangeInspection>, Vec<String>), String> {
    let workbook = read_office_zip_text(input, "xl/workbook.xml", "XLSX")?;
    let sheet_names = collect_open_tags(&workbook, "sheet")
        .into_iter()
        .filter_map(|tag| extract_attr_value(tag, "name").map(|value| xml_unescape(&value)))
        .collect::<Vec<_>>();
    if sheet_names.is_empty() {
        return Err("XLSX template has no worksheets in xl/workbook.xml".to_string());
    }

    let mut warnings = Vec::new();
    let mut used_ids = BTreeMap::new();
    let mut named_ranges = Vec::new();
    for (tag, body) in collect_elements(&workbook, "definedName") {
        let Some(name) = extract_attr_value(tag, "name").map(|value| xml_unescape(&value)) else {
            continue;
        };
        if name.starts_with("_xlnm.") {
            continue;
        }
        let reference = xml_unescape(body.trim());
        let local_sheet_id =
            extract_attr_value(tag, "localSheetId").and_then(|value| value.parse::<usize>().ok());
        let parsed = parse_xlsx_range_reference(&reference, local_sheet_id, &sheet_names);
        let (source_sheet, start_cell, suggested_binding_type, bindable) = match parsed {
            Some((sheet, cell, binding_type)) => (Some(sheet), Some(cell), binding_type, true),
            None => {
                warnings.push(format!(
                    "named range '{name}' has an unsupported reference '{reference}' and was not auto-bound"
                ));
                (None, None, "text".to_string(), false)
            }
        };
        let base_id = sanitize_field_id(&name, "range");
        named_ranges.push(XlsxNamedRangeInspection {
            name,
            reference,
            source_sheet,
            start_cell,
            suggested_binding_type,
            suggested_binding_id: dedupe_binding_id(base_id, &mut used_ids),
            bindable,
        });
    }
    if named_ranges.iter().all(|range| !range.bindable) {
        warnings.push(
            "no bindable named ranges were found; sheet templates were generated without bindings, so add cell or namedRange bindings in template.manifest.json"
                .to_string(),
        );
    }
    Ok((sheet_names, named_ranges, warnings))
}

fn prepare_office_draft_pack(
    input: &Path,
    out_dir: &Path,
    format: &str,
) -> Result<(PathBuf, PathBuf), String> {
    crate::validator::require_file(input, &format!("template.{format}"))?;
    if out_dir.exists() && !out_dir.is_dir() {
        return Err(format!(
            "draft template pack path exists but is not a directory: {}",
            out_dir.display()
        ));
    }
    fs::create_dir_all(out_dir).map_err(|err| {
        format!(
            "failed to create draft template pack directory '{}': {err}",
            out_dir.display()
        )
    })?;
    let template_file = out_dir.join(format!("template.{format}"));
    let manifest_file = out_dir.join("template.manifest.json");
    if template_file.exists() || manifest_file.exists() {
        return Err(format!(
            "draft template pack already contains template.{format} or template.manifest.json: {}",
            out_dir.display()
        ));
    }
    fs::copy(input, &template_file).map_err(|err| {
        format!(
            "failed to copy {} '{}' to '{}': {err}",
            format.to_ascii_uppercase(),
            input.display(),
            template_file.display()
        )
    })?;
    Ok((template_file, manifest_file))
}

fn default_draft_input() -> TemplateInputSpec {
    TemplateInputSpec {
        profile: None,
        formats: vec!["json".to_string()],
        schema: None,
        schema_id: None,
        md_profile: None,
        authoring: None,
    }
}

fn apply_draft_identity(manifest: &mut TemplateManifest, input: &Path, format: &str) {
    let file_stem = office_file_stem(input);
    manifest.template_id = sanitize_template_id(&file_stem);
    manifest.name = file_stem;
    manifest.entry = Some(format!("template.{format}"));
}

fn office_file_name(input: &Path, fallback: &str) -> String {
    input
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(fallback)
        .to_string()
}

fn office_file_stem(input: &Path) -> String {
    input
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("draft-template")
        .to_string()
}

fn write_draft_manifest(out: &Path, manifest: &TemplateManifest) -> Result<(), String> {
    let raw = serde_json::to_string_pretty(manifest)
        .map_err(|err| format!("failed to serialize draft manifest: {err}"))?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create output directory: {err}"))?;
    }
    fs::write(out, raw).map_err(|err| format!("failed to write draft manifest: {err}"))
}

fn read_office_zip_text(input: &Path, entry_path: &str, label: &str) -> Result<String, String> {
    let file = File::open(input)
        .map_err(|err| format!("failed to open {label} '{}': {err}", input.display()))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|err| format!("invalid {label} package '{}': {err}", input.display()))?;
    let mut entry = archive
        .by_name(entry_path)
        .map_err(|err| format!("{label} is missing {entry_path}: {err}"))?;
    let mut raw = String::new();
    entry
        .read_to_string(&mut raw)
        .map_err(|err| format!("failed to read {entry_path}: {err}"))?;
    Ok(raw)
}

fn collect_xml_text(raw: &str, tag: &str) -> String {
    collect_elements(raw, tag)
        .into_iter()
        .map(|(_, body)| xml_unescape(body))
        .collect::<String>()
}

fn collect_placeholders(raw: &str) -> Vec<String> {
    let mut placeholders = Vec::new();
    let mut seen = BTreeSet::new();
    let mut cursor = 0;
    while let Some(start_offset) = raw[cursor..].find("{{") {
        let start = cursor + start_offset;
        let Some(end_offset) = raw[start + 2..].find("}}") else {
            break;
        };
        let end = start + 2 + end_offset + 2;
        let placeholder = raw[start..end].to_string();
        let inner = placeholder
            .strip_prefix("{{")
            .and_then(|value| value.strip_suffix("}}"))
            .unwrap_or("")
            .trim();
        if !inner.is_empty() && placeholder.len() <= 132 && seen.insert(placeholder.clone()) {
            placeholders.push(placeholder);
        }
        cursor = end;
    }
    placeholders
}

fn collect_open_tags<'a>(raw: &'a str, tag: &str) -> Vec<&'a str> {
    let pattern = format!("<{tag}");
    let mut tags = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = raw[cursor..].find(&pattern) {
        let start = cursor + offset;
        let boundary = raw[start + pattern.len()..].chars().next();
        if !boundary.is_some_and(|ch| ch.is_ascii_whitespace() || ch == '>' || ch == '/') {
            cursor = start + pattern.len();
            continue;
        }
        let Some(close_offset) = raw[start..].find('>') else {
            break;
        };
        let end = start + close_offset + 1;
        tags.push(&raw[start..end]);
        cursor = end;
    }
    tags
}

fn collect_elements<'a>(raw: &'a str, tag: &str) -> Vec<(&'a str, &'a str)> {
    let open_pattern = format!("<{tag}");
    let close_pattern = format!("</{tag}>");
    let mut elements = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = raw[cursor..].find(&open_pattern) {
        let start = cursor + offset;
        let boundary = raw[start + open_pattern.len()..].chars().next();
        if !boundary.is_some_and(|ch| ch.is_ascii_whitespace() || ch == '>') {
            cursor = start + open_pattern.len();
            continue;
        }
        let Some(open_end_offset) = raw[start..].find('>') else {
            break;
        };
        let open_end = start + open_end_offset + 1;
        let Some(close_offset) = raw[open_end..].find(&close_pattern) else {
            break;
        };
        let close = open_end + close_offset;
        elements.push((&raw[start..open_end], &raw[open_end..close]));
        cursor = close + close_pattern.len();
    }
    elements
}

fn parse_xlsx_range_reference(
    reference: &str,
    local_sheet_id: Option<usize>,
    sheet_names: &[String],
) -> Option<(String, String, String)> {
    let first_area = reference.trim().trim_start_matches('=').split(',').next()?;
    if first_area.contains("#REF!") {
        return None;
    }
    let (sheet_name, cells) = if let Some((sheet, cells)) = first_area.rsplit_once('!') {
        (unquote_xlsx_sheet_name(sheet), cells)
    } else {
        (sheet_names.get(local_sheet_id?)?.clone(), first_area)
    };
    if !sheet_names.contains(&sheet_name) {
        return None;
    }
    let (start, end) = cells.split_once(':').unwrap_or((cells, cells));
    let start = normalize_xlsx_cell_reference(start)?;
    let end = normalize_xlsx_cell_reference(end)?;
    let (start_col, start_row) = split_xlsx_cell(&start)?;
    let (end_col, end_row) = split_xlsx_cell(&end)?;
    let binding_type = if start_col == end_col && start_row == end_row {
        "text"
    } else if start_col == end_col {
        "list"
    } else {
        "table"
    };
    Some((sheet_name, start, binding_type.to_string()))
}

fn unquote_xlsx_sheet_name(value: &str) -> String {
    let value = value.trim();
    if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        return value[1..value.len() - 1].replace("''", "'");
    }
    value.to_string()
}

fn normalize_xlsx_cell_reference(value: &str) -> Option<String> {
    let cell = value
        .trim()
        .trim_start_matches('=')
        .replace('$', "")
        .to_ascii_uppercase();
    split_xlsx_cell(&cell).map(|_| cell)
}

fn split_xlsx_cell(cell: &str) -> Option<(String, u32)> {
    let letter_count = cell
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .count();
    if letter_count == 0 || letter_count == cell.len() {
        return None;
    }
    let (column, row) = cell.split_at(letter_count);
    if !column.chars().all(|ch| ch.is_ascii_alphabetic())
        || !row.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    let row = row.parse::<u32>().ok().filter(|row| *row > 0)?;
    Some((column.to_ascii_uppercase(), row))
}

fn infer_document_binding_type(name: &str) -> String {
    let normalized = name.to_ascii_lowercase();
    if normalized.contains("image") || normalized.contains("photo") || normalized.contains("cover")
    {
        "image".to_string()
    } else if normalized.contains("table") || normalized.contains("matrix") {
        "table".to_string()
    } else if normalized.contains("list")
        || normalized.contains("items")
        || normalized.contains("goals")
        || normalized.contains("steps")
    {
        "list".to_string()
    } else {
        "text".to_string()
    }
}

fn sanitize_field_id(value: &str, fallback: &str) -> String {
    let id = sanitize_binding_id(value);
    if id == "shape" && !value.eq_ignore_ascii_case("shape") {
        fallback.to_string()
    } else {
        id
    }
}

pub fn inspect_pptx(input: &Path, out: &Path) -> Result<PptxTemplateInspection, String> {
    crate::validator::require_file(input, "template.pptx")?;
    let file_name = input
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("template.pptx");
    let file_stem = input
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("draft-template");
    let (slides, warnings) = inspect_pptx_slides(input)?;
    let page_templates = slides
        .iter()
        .map(|slide| {
            let template_id = format!("slide_{:02}", slide.source_slide);
            (
                template_id,
                page_template_from_inspected_slide(slide.source_slide, &slide.shapes),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let deck_recipes = BTreeMap::from([(
        "draft_deck".to_string(),
        slides
            .iter()
            .map(|slide| DeckStep {
                use_template: format!("slide_{:02}", slide.source_slide),
                data: Some("$".to_string()),
                repeat: None,
                steps: Vec::new(),
                chunk: None,
                variants: Vec::new(),
                overflow: None,
                when: None,
            })
            .collect::<Vec<_>>(),
    )]);
    let manifest = TemplateManifest {
        schema_version: "1.0".to_string(),
        template_id: sanitize_template_id(file_stem),
        name: file_stem.to_string(),
        family_id: None,
        role: Some("slides".to_string()),
        format: "pptx".to_string(),
        template_type: Some("declarative".to_string()),
        entry: Some(file_name.to_string()),
        content_schema: None,
        input: Some(TemplateInputSpec {
            profile: None,
            formats: vec!["json".to_string()],
            schema: None,
            schema_id: None,
            md_profile: None,
            authoring: None,
        }),
        preview: None,
        render_mode: Some("pptx_automizer".to_string()),
        renderer: None,
        pipeline: Vec::new(),
        dependencies: TemplateDependencySpec::default(),
        test_cases: Vec::new(),
        page_templates,
        deck_recipes,
        block_templates: BTreeMap::new(),
        document_recipes: BTreeMap::new(),
        sheet_templates: BTreeMap::new(),
        workbook_recipes: BTreeMap::new(),
        layouts: BTreeMap::new(),
    };
    let raw = serde_json::to_string_pretty(&manifest)
        .map_err(|err| format!("failed to serialize draft manifest: {err}"))?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create output directory: {err}"))?;
    }
    fs::write(out, raw).map_err(|err| format!("failed to write draft manifest: {err}"))?;
    Ok(PptxTemplateInspection {
        input_file: absolutize(input).display().to_string(),
        output_file: absolutize(out).display().to_string(),
        slide_count: slides.len(),
        slides,
        draft_manifest: manifest,
        warnings,
    })
}

pub fn create_pptx_template_pack_draft(
    input: &Path,
    out_dir: &Path,
) -> Result<PptxTemplatePackDraft, String> {
    crate::validator::require_file(input, "template.pptx")?;
    if out_dir.exists() && !out_dir.is_dir() {
        return Err(format!(
            "draft template pack path exists but is not a directory: {}",
            out_dir.display()
        ));
    }
    fs::create_dir_all(out_dir).map_err(|err| {
        format!(
            "failed to create draft template pack directory '{}': {err}",
            out_dir.display()
        )
    })?;

    let template_file = out_dir.join("template.pptx");
    let manifest_file = out_dir.join("template.manifest.json");
    if template_file.exists() || manifest_file.exists() {
        return Err(format!(
            "draft template pack already contains template.pptx or template.manifest.json: {}",
            out_dir.display()
        ));
    }

    fs::copy(input, &template_file).map_err(|err| {
        format!(
            "failed to copy PPTX '{}' to '{}': {err}",
            input.display(),
            template_file.display()
        )
    })?;
    let mut inspection = inspect_pptx(&template_file, &manifest_file)?;
    apply_draft_identity(&mut inspection.draft_manifest, input, "pptx");
    write_draft_manifest(&manifest_file, &inspection.draft_manifest)?;
    let validation = validate_template_pack(out_dir)?;

    Ok(PptxTemplatePackDraft {
        pack_dir: absolutize(out_dir).display().to_string(),
        template_file: absolutize(&template_file).display().to_string(),
        manifest_file: absolutize(&manifest_file).display().to_string(),
        inspection,
        validation,
    })
}

fn inspect_pptx_slides(input: &Path) -> Result<(Vec<PptxSlideInspection>, Vec<String>), String> {
    let file = File::open(input)
        .map_err(|err| format!("failed to open PPTX '{}': {err}", input.display()))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|err| format!("invalid PPTX package '{}': {err}", input.display()))?;
    let mut slide_numbers = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|err| format!("failed to inspect PPTX zip entry {index}: {err}"))?;
        if let Some(slide_number) = slide_number_from_entry_name(entry.name()) {
            slide_numbers.push(slide_number);
        }
    }
    slide_numbers.sort_unstable();
    slide_numbers.dedup();

    let mut warnings = Vec::new();
    let mut slides = Vec::new();
    for source_slide in slide_numbers {
        let entry_path = format!("ppt/slides/slide{source_slide}.xml");
        let mut slide_xml = String::new();
        match archive.by_name(&entry_path) {
            Ok(mut entry) => {
                entry
                    .read_to_string(&mut slide_xml)
                    .map_err(|err| format!("failed to read {entry_path}: {err}"))?;
            }
            Err(err) => {
                warnings.push(format!(
                    "source slide {source_slide} could not be read: {err}"
                ));
                continue;
            }
        }
        let shapes = inspect_pptx_slide_shapes(&slide_xml);
        let bindable_shape_count = shapes.iter().filter(|shape| shape.bindable).count();
        slides.push(PptxSlideInspection {
            source_slide,
            shape_count: shapes.len(),
            bindable_shape_count,
            shapes,
        });
    }

    if slides.is_empty() {
        warnings.push("no slide XML parts were found under ppt/slides".to_string());
    }
    Ok((slides, warnings))
}

fn slide_number_from_entry_name(name: &str) -> Option<u32> {
    let rest = name.strip_prefix("ppt/slides/slide")?;
    let number = rest.strip_suffix(".xml")?;
    number.parse::<u32>().ok().filter(|value| *value > 0)
}

fn inspect_pptx_slide_shapes(slide_xml: &str) -> Vec<PptxShapeInspection> {
    let mut elements = Vec::new();
    collect_pptx_elements(slide_xml, "p:sp", "shape", &mut elements);
    collect_pptx_elements(slide_xml, "p:pic", "image", &mut elements);
    collect_pptx_elements(slide_xml, "p:graphicFrame", "graphicFrame", &mut elements);
    elements.sort_by_key(|element| element.position);

    let mut used_ids = BTreeMap::<String, usize>::new();
    elements
        .into_iter()
        .filter_map(|element| {
            let name = extract_tag_attr_value(&element.xml, "p:cNvPr", "name")?;
            if name.trim().is_empty() {
                return None;
            }
            let suggested_binding_type =
                infer_pptx_binding_type(&element.element_type, &element.xml, &name);
            let base_id = sanitize_binding_id(&name);
            let suggested_binding_id = dedupe_binding_id(base_id, &mut used_ids);
            let bindable = is_bindable_shape_name(&name);
            Some(PptxShapeInspection {
                name,
                element_type: element.element_type,
                suggested_binding_type,
                suggested_binding_id,
                bindable,
            })
        })
        .collect()
}

#[derive(Debug)]
struct PptxElement {
    position: usize,
    element_type: String,
    xml: String,
}

fn collect_pptx_elements(
    slide_xml: &str,
    tag: &str,
    element_type: &str,
    elements: &mut Vec<PptxElement>,
) {
    let open_pattern = format!("<{tag}");
    let close_pattern = format!("</{tag}>");
    let mut cursor = 0;
    while let Some(offset) = slide_xml[cursor..].find(&open_pattern) {
        let start = cursor + offset;
        let Some(close_offset) = slide_xml[start..].find(&close_pattern) else {
            break;
        };
        let end = start + close_offset + close_pattern.len();
        elements.push(PptxElement {
            position: start,
            element_type: element_type.to_string(),
            xml: slide_xml[start..end].to_string(),
        });
        cursor = end;
    }
}

fn extract_tag_attr_value(raw: &str, tag: &str, attr: &str) -> Option<String> {
    let tag_pattern = format!("<{tag}");
    let start = raw.find(&tag_pattern)?;
    let close = raw[start..].find('>')? + start;
    extract_attr_value(&raw[start..=close], attr).map(|value| xml_unescape(&value))
}

fn extract_attr_value(raw: &str, attr: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let pattern = format!(r#" {attr}={quote}"#);
        if let Some(start_offset) = raw.find(&pattern) {
            let value_start = start_offset + pattern.len();
            let value_end = raw[value_start..].find(quote)? + value_start;
            return Some(raw[value_start..value_end].to_string());
        }
    }
    None
}

fn infer_pptx_binding_type(element_type: &str, element_xml: &str, shape_name: &str) -> String {
    let normalized_name = sanitize_binding_id(shape_name).to_ascii_lowercase();
    if element_xml.contains("/drawingml/2006/table") || element_xml.contains("<a:tbl") {
        return "table".to_string();
    }
    if element_xml.contains("/drawingml/2006/chart")
        || element_xml.contains("<c:chart")
        || element_xml.contains(":chart")
    {
        return "chart".to_string();
    }
    if normalized_name.contains("chart") {
        return "chart".to_string();
    }
    if element_type == "image" {
        return "image".to_string();
    }
    if matches!(
        normalized_name.as_str(),
        "goals" | "items" | "summary" | "evaluations" | "bullets"
    ) {
        return "list".to_string();
    }
    "text".to_string()
}

fn page_template_from_inspected_slide(
    source_slide: u32,
    shapes: &[PptxShapeInspection],
) -> PageTemplate {
    let bindings = shapes
        .iter()
        .filter(|shape| shape.bindable)
        .map(|shape| {
            (
                shape.suggested_binding_id.clone(),
                PageBinding {
                    shape_name: Some(shape.name.clone()),
                    creation_id: None,
                    data_path: Some(format!("$.{}", shape.suggested_binding_id)),
                    binding_type: Some(shape.suggested_binding_type.clone()),
                    required: false,
                    max_length: None,
                    max_items: None,
                    fit: if shape.suggested_binding_type == "image" {
                        Some("contain".to_string())
                    } else {
                        None
                    },
                    chart_mode: if shape.suggested_binding_type == "chart" {
                        Some(if shape.element_type == "image" {
                            "image".to_string()
                        } else {
                            "native".to_string()
                        })
                    } else {
                        None
                    },
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    PageTemplate {
        source_slide,
        description: Some(format!("Draft generated from slide {source_slide}")),
        bindings,
        constraints: BTreeMap::new(),
    }
}

fn is_bindable_shape_name(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && !trimmed.starts_with("chrome:")
        && !trimmed.starts_with("pptx:")
        && !trimmed.starts_with("rdeckforge:")
}

fn sanitize_binding_id(name: &str) -> String {
    let raw = name.strip_prefix("ph:").unwrap_or(name).trim();
    let mut output = String::new();
    let mut previous_was_separator = false;
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            output.push(ch);
            previous_was_separator = false;
        } else if !previous_was_separator && !output.is_empty() {
            output.push('_');
            previous_was_separator = true;
        }
    }
    while output.ends_with('_') {
        output.pop();
    }
    if output.is_empty() {
        output.push_str("shape");
    }
    if output
        .chars()
        .next()
        .is_some_and(|value| value.is_ascii_digit())
    {
        output.insert_str(0, "shape_");
    }
    output
}

fn dedupe_binding_id(base_id: String, used_ids: &mut BTreeMap<String, usize>) -> String {
    let count = used_ids.entry(base_id.clone()).or_insert(0);
    *count += 1;
    if *count == 1 {
        base_id
    } else {
        format!("{base_id}_{}", count)
    }
}

fn sanitize_template_id(value: &str) -> String {
    let mut id = String::new();
    let mut previous_was_separator = false;
    for ch in value.chars() {
        if ch.is_alphanumeric() {
            id.extend(ch.to_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator && !id.is_empty() {
            id.push('-');
            previous_was_separator = true;
        }
    }
    while id.ends_with('-') {
        id.pop();
    }
    if id.is_empty() {
        "draft-template".to_string()
    } else {
        id
    }
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{CompressionMethod, ZipWriter, write::FileOptions};

    #[test]
    fn validate_template_pack_uses_builtin_profile_defaults() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-profile-template-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).map_err(|err| format!("failed to create test dir: {err}"))?;
        fs::write(root.join("template.xlsx"), "")
            .map_err(|err| format!("failed to write test template: {err}"))?;
        fs::write(
            root.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "assessment-profile-template",
  "name": "Assessment Profile Template",
  "format": "xlsx",
  "entry": "template.xlsx",
  "input": { "profile": "feature_assessment_v1" },
  "sheetTemplates": {
    "report": { "bindings": {} }
  },
  "workbookRecipes": {
    "standard_workbook": [{ "use": "report", "data": "$" }]
  }
}"#,
        )
        .map_err(|err| format!("failed to write test manifest: {err}"))?;

        let validation = validate_template_pack(&root)?;

        assert_eq!(
            validation.input_profile.as_deref(),
            Some("feature_assessment_v1")
        );
        assert_eq!(
            validation.input_profile_name.as_deref(),
            Some("Feature Assessment Workbook")
        );
        assert_eq!(validation.input_formats, vec!["json"]);
        assert_eq!(
            validation.input_schema_id.as_deref(),
            Some("feature_assessment_v1")
        );
        assert!(validation.input_builtin_schema);
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn validate_template_pack_exposes_family_and_role() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-family-template-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).map_err(|err| format!("failed to create test dir: {err}"))?;
        fs::write(root.join("template.docx"), b"docx")
            .map_err(|err| format!("failed to write template entry: {err}"))?;
        fs::write(
            root.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "lesson-plan-template",
  "name": "Lesson Plan Template",
  "familyId": "nursing_training_v1",
  "role": "lesson-plan",
  "format": "docx",
  "entry": "template.docx",
  "input": { "formats": ["json"], "schemaId": "nursing_training_v1" },
  "blockTemplates": {
    "body": { "bindings": {} }
  },
  "documentRecipes": {
    "standard_document": [{ "use": "body", "data": "$" }]
  }
}"#,
        )
        .map_err(|err| format!("failed to write test manifest: {err}"))?;

        let validation = validate_template_pack(&root)?;
        assert_eq!(validation.family_id.as_deref(), Some("nursing_training_v1"));
        assert_eq!(validation.role, "lesson_plan");
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn validate_template_pack_defaults_role_from_format() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-role-default-template-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).map_err(|err| format!("failed to create test dir: {err}"))?;
        fs::write(root.join("template.xlsx"), b"xlsx")
            .map_err(|err| format!("failed to write template entry: {err}"))?;
        fs::write(
            root.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "workbook-template",
  "name": "Workbook Template",
  "format": "xlsx",
  "entry": "template.xlsx",
  "input": { "formats": ["json"] },
  "sheetTemplates": {
    "sheet": { "sourceSheet": "Sheet1", "bindings": {} }
  },
  "workbookRecipes": {
    "standard_workbook": [{ "use": "sheet", "data": "$" }]
  }
}"#,
        )
        .map_err(|err| format!("failed to write test manifest: {err}"))?;

        let validation = validate_template_pack(&root)?;
        assert_eq!(validation.family_id, None);
        assert_eq!(validation.role, "workbook");
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn validate_template_pack_accepts_script_renderer_without_office_entry() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-script-template-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("scripts"))
            .map_err(|err| format!("failed to create test script dir: {err}"))?;
        fs::write(root.join("scripts").join("build.py"), "print('ok')\n")
            .map_err(|err| format!("failed to write test script: {err}"))?;
        fs::write(
            root.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "script-template",
  "name": "Script Template",
  "format": "pptx",
  "templateType": "script",
  "input": {
    "formats": ["md"],
    "schemaId": "chapter_markdown_v1"
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

        let validation = validate_template_pack(&root)?;

        assert_eq!(validation.template_type, "script");
        assert_eq!(validation.entry_path, "");
        assert_eq!(validation.renderer_type.as_deref(), Some("script.python"));
        assert_eq!(
            validation.renderer_entry.as_deref(),
            Some("scripts/build.py")
        );
        assert!(validation.warnings.is_empty());
        assert_eq!(validation.health.status, "unchecked");
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn inspect_pptx_generates_draft_manifest_from_shapes() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-inspect-pptx-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).map_err(|err| format!("failed to create test dir: {err}"))?;
        let input = root.join("My Teaching Template.pptx");
        write_zip(
            &input,
            &[
                (
                    "ppt/slides/slide1.xml",
                    r#"<p:sld><p:cSld><p:spTree>
  <p:sp><p:nvSpPr><p:cNvPr id="1" name="chrome:accent"/></p:nvSpPr></p:sp>
  <p:sp><p:nvSpPr><p:cNvPr id="2" name="ph:title"/></p:nvSpPr></p:sp>
  <p:sp><p:nvSpPr><p:cNvPr id="3" name="ph:goals"/></p:nvSpPr></p:sp>
  <p:pic><p:nvPicPr><p:cNvPr id="4" name="ph:coverImage"/></p:nvPicPr></p:pic>
  <p:pic><p:nvPicPr><p:cNvPr id="5" name="ph:scoreChart"/></p:nvPicPr></p:pic>
  <p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="6" name="ph:assessmentTable"/></p:nvGraphicFramePr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/table"><a:tbl/></a:graphicData></a:graphic></p:graphicFrame>
</p:spTree></p:cSld></p:sld>"#,
                ),
                (
                    "ppt/slides/slide2.xml",
                    r#"<p:sld><p:cSld><p:spTree>
  <p:sp><p:nvSpPr><p:cNvPr id="1" name="ph:title"/></p:nvSpPr></p:sp>
  <p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id="2" name="ph:nativeChart"/></p:nvGraphicFramePr><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart/></a:graphicData></a:graphic></p:graphicFrame>
</p:spTree></p:cSld></p:sld>"#,
                ),
            ],
        )?;
        let out = root.join("template.manifest.json");

        let inspection = inspect_pptx(&input, &out)?;

        assert!(out.is_file());
        assert_eq!(inspection.slide_count, 2);
        assert_eq!(
            inspection.draft_manifest.template_id,
            "my-teaching-template"
        );
        assert_eq!(
            inspection
                .draft_manifest
                .deck_recipes
                .get("draft_deck")
                .map(Vec::len),
            Some(2)
        );
        let slide_1 = inspection
            .draft_manifest
            .page_templates
            .get("slide_01")
            .ok_or_else(|| "missing slide_01 template".to_string())?;
        assert!(!slide_1.bindings.contains_key("chrome_accent"));
        assert_eq!(
            slide_1
                .bindings
                .get("title")
                .and_then(|binding| binding.binding_type.as_deref()),
            Some("text")
        );
        assert_eq!(
            slide_1
                .bindings
                .get("goals")
                .and_then(|binding| binding.binding_type.as_deref()),
            Some("list")
        );
        assert_eq!(
            slide_1
                .bindings
                .get("coverImage")
                .and_then(|binding| binding.binding_type.as_deref()),
            Some("image")
        );
        assert_eq!(
            slide_1
                .bindings
                .get("scoreChart")
                .and_then(|binding| binding.binding_type.as_deref()),
            Some("chart")
        );
        assert_eq!(
            slide_1
                .bindings
                .get("scoreChart")
                .and_then(|binding| binding.chart_mode.as_deref()),
            Some("image")
        );
        assert_eq!(
            slide_1
                .bindings
                .get("assessmentTable")
                .and_then(|binding| binding.binding_type.as_deref()),
            Some("table")
        );
        let slide_2 = inspection
            .draft_manifest
            .page_templates
            .get("slide_02")
            .ok_or_else(|| "missing slide_02 template".to_string())?;
        assert_eq!(
            slide_2
                .bindings
                .get("nativeChart")
                .and_then(|binding| binding.chart_mode.as_deref()),
            Some("native")
        );
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn create_docx_draft_binds_contiguous_placeholders_and_warns_for_split_runs()
    -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-inspect-docx-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).map_err(|err| format!("failed to create test dir: {err}"))?;
        let input = root.join("Teaching Notes.docx");
        write_zip(
            &input,
            &[(
                "word/document.xml",
                r#"<w:document><w:body>
  <w:p><w:r><w:t>{{title}}</w:t></w:r></w:p>
  <w:p><w:r><w:t>{{go</w:t></w:r><w:r><w:t>als}}</w:t></w:r></w:p>
  <w:p><w:r><w:t>{{assessmentTable}}</w:t></w:r></w:p>
</w:body></w:document>"#,
            )],
        )?;
        let pack = root.join("docx-draft");

        let draft = create_docx_template_pack_draft(&input, &pack)?;

        assert!(pack.join("template.docx").is_file());
        assert!(pack.join("template.manifest.json").is_file());
        assert_eq!(
            draft.inspection.draft_manifest.template_id,
            "teaching-notes"
        );
        assert_eq!(draft.validation.name, "Teaching Notes");
        assert_eq!(draft.inspection.placeholder_count, 3);
        assert!(
            draft
                .inspection
                .warnings
                .iter()
                .any(|warning| warning.contains("split across Word runs"))
        );
        let bindings = &draft
            .inspection
            .draft_manifest
            .block_templates
            .get("document")
            .ok_or_else(|| "missing document block".to_string())?
            .bindings;
        assert!(bindings.contains_key("title"));
        assert!(!bindings.contains_key("goals"));
        assert_eq!(
            bindings
                .get("assessmentTable")
                .and_then(|binding| binding.binding_type.as_deref()),
            Some("table")
        );
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn create_xlsx_draft_maps_sheets_and_named_range_shapes() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-inspect-xlsx-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).map_err(|err| format!("failed to create test dir: {err}"))?;
        let input = root.join("Assessment Workbook.xlsx");
        write_zip(
            &input,
            &[(
                "xl/workbook.xml",
                r#"<workbook>
  <sheets>
    <sheet name="Report" sheetId="1" r:id="rId1"/>
    <sheet name="Detail" sheetId="2" r:id="rId2"/>
  </sheets>
  <definedNames>
    <definedName name="TitleCell">'Report'!$B$2</definedName>
    <definedName name="GoalsRange">'Report'!$B$4:$B$8</definedName>
    <definedName name="AssessmentTable">'Detail'!$A$2:$D$20</definedName>
    <definedName name="UnsupportedFormula">OFFSET('Report'!$A$1,0,0)</definedName>
    <definedName name="_xlnm.Print_Area">'Report'!$A$1:$D$20</definedName>
  </definedNames>
</workbook>"#,
            )],
        )?;
        let pack = root.join("xlsx-draft");

        let draft = create_xlsx_template_pack_draft(&input, &pack)?;

        assert!(pack.join("template.xlsx").is_file());
        assert_eq!(
            draft.inspection.draft_manifest.template_id,
            "assessment-workbook"
        );
        assert_eq!(draft.validation.name, "Assessment Workbook");
        assert_eq!(draft.inspection.sheet_count, 2);
        assert_eq!(draft.inspection.named_range_count, 4);
        assert!(
            draft
                .inspection
                .warnings
                .iter()
                .any(|warning| warning.contains("UnsupportedFormula"))
        );
        let report = draft
            .inspection
            .draft_manifest
            .sheet_templates
            .get("sheet_01")
            .ok_or_else(|| "missing report sheet template".to_string())?;
        assert_eq!(report.source_sheet.as_deref(), Some("Report"));
        assert_eq!(
            report
                .bindings
                .get("TitleCell")
                .and_then(|binding| binding.binding_type.as_deref()),
            Some("text")
        );
        assert_eq!(
            report
                .bindings
                .get("GoalsRange")
                .and_then(|binding| binding.binding_type.as_deref()),
            Some("list")
        );
        let detail = draft
            .inspection
            .draft_manifest
            .sheet_templates
            .get("sheet_02")
            .ok_or_else(|| "missing detail sheet template".to_string())?;
        assert_eq!(
            detail
                .bindings
                .get("AssessmentTable")
                .and_then(|binding| binding.binding_type.as_deref()),
            Some("table")
        );
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    fn write_zip(path: &Path, entries: &[(&str, &str)]) -> Result<(), String> {
        let file = File::create(path)
            .map_err(|err| format!("failed to create zip '{}': {err}", path.display()))?;
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, content) in entries {
            zip.start_file(*name, options)
                .map_err(|err| format!("failed to start zip entry {name}: {err}"))?;
            zip.write_all(content.as_bytes())
                .map_err(|err| format!("failed to write zip entry {name}: {err}"))?;
        }
        zip.finish()
            .map_err(|err| format!("failed to finish zip '{}': {err}", path.display()))?;
        Ok(())
    }
}
