use serde::Serialize;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateFamilyDescriptor {
    pub schema_id: String,
    pub format: String,
    pub name: String,
    pub anchor_template_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateFamilyRenderOutput {
    pub template_id: String,
    pub template_name: String,
    pub template_dir: String,
    pub variant_id: String,
    pub status: &'static str,
    pub output_file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub renderer_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_recipe: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acceptance_summary: Option<crate::content_ir::AcceptanceSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedFamilyTemplate {
    pub template_id: String,
    pub template_name: String,
    pub template_dir: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateFamilyRenderResult {
    pub status: &'static str,
    pub family: TemplateFamilyDescriptor,
    pub input_file: String,
    pub input_format: String,
    pub output_dir: String,
    pub base_name: String,
    pub template_count: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub outputs: Vec<TemplateFamilyRenderOutput>,
    pub skipped_templates: Vec<SkippedFamilyTemplate>,
}

#[derive(Debug, Clone)]
struct FamilyTemplateCandidate {
    template_id: String,
    name: String,
    path: PathBuf,
    format: String,
    input_formats: Vec<String>,
    template_type: String,
    renderer_type: Option<String>,
    recipe_ids: Vec<String>,
}

pub fn render_linked_template_family(
    anchor_template: &Path,
    input: &Path,
    out_dir: &Path,
    base_name: Option<&str>,
    recipe: Option<&str>,
    fail_fast: bool,
) -> Result<TemplateFamilyRenderResult, String> {
    let anchor_path = normalize_existing_path(anchor_template);
    let anchor_validation = crate::template_manifest::validate_template_pack(&anchor_path)?;
    let schema_id = anchor_validation
        .input_schema_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            format!(
                "template '{}' has no input.schemaId; template family rendering requires an explicit schema id",
                anchor_validation.template_id
            )
        })?
        .to_string();
    let family_format = anchor_validation.format.to_ascii_lowercase();
    let source_path = crate::content_source::resolve_content_source_path(input)?;
    let input_format = crate::content_source::infer_format(&source_path);

    let mut candidates = vec![candidate_from_validation(&anchor_path, &anchor_validation)];
    let mut seen_paths = HashSet::from([path_identity(&anchor_path)]);
    for record in crate::storage::list_template_packs()? {
        let record_schema = record.input_schema_id.as_deref().map(str::trim);
        if record_schema != Some(schema_id.as_str())
            || !record.format.eq_ignore_ascii_case(&family_format)
        {
            continue;
        }
        let record_path = normalize_existing_path(Path::new(&record.path));
        if seen_paths.insert(path_identity(&record_path)) {
            candidates.push(candidate_from_record(record_path, record));
        }
    }
    candidates.sort_by(|left, right| left.template_id.cmp(&right.template_id));

    let family_name = family_name(&candidates, &schema_id);
    let mut skipped_templates = Vec::new();
    let mut compatible = Vec::new();
    for candidate in candidates {
        if supports_input_format(&candidate.input_formats, &input_format) {
            compatible.push(candidate);
        } else {
            skipped_templates.push(SkippedFamilyTemplate {
                template_id: candidate.template_id,
                template_name: candidate.name,
                template_dir: absolutize(&candidate.path).display().to_string(),
                reason: format!("input format '{input_format}' is not accepted by this template"),
            });
        }
    }
    if compatible.is_empty() {
        return Err(format!(
            "no templates in family '{schema_id}' accept input format '{input_format}'"
        ));
    }

    fs::create_dir_all(out_dir).map_err(|err| {
        format!(
            "failed to create family output directory '{}': {err}",
            out_dir.display()
        )
    })?;
    let output_dir = absolutize(out_dir);
    let base_name = normalized_base_name(base_name, &source_path);
    let shared_prefix = common_template_id_prefix(&compatible);
    let template_count = compatible.len();
    let mut used_variants = HashSet::new();
    let mut outputs = Vec::with_capacity(template_count);

    for (index, candidate) in compatible.iter().enumerate() {
        let base_variant = variant_slug(&candidate.template_id, shared_prefix);
        let variant_id = unique_variant_id(base_variant, &mut used_variants);
        let output_file = output_dir.join(format!(
            "{base_name}-{variant_id}.{}",
            candidate.format.to_ascii_lowercase()
        ));
        let selected_recipe = recipe_for_candidate(candidate, recipe);
        match crate::accepted_render::render_accepted_office(
            input,
            &candidate.path,
            selected_recipe.as_deref(),
            &output_file,
        ) {
            Ok(result) => outputs.push(TemplateFamilyRenderOutput {
                template_id: candidate.template_id.clone(),
                template_name: candidate.name.clone(),
                template_dir: absolutize(&candidate.path).display().to_string(),
                variant_id,
                status: "rendered",
                output_file: result.output_file,
                renderer_status: Some(result.renderer_status.to_string()),
                selected_recipe: result.selected_recipe,
                acceptance_summary: Some(result.acceptance_summary),
                error: None,
            }),
            Err(error) => {
                outputs.push(TemplateFamilyRenderOutput {
                    template_id: candidate.template_id.clone(),
                    template_name: candidate.name.clone(),
                    template_dir: absolutize(&candidate.path).display().to_string(),
                    variant_id,
                    status: "failed",
                    output_file: absolutize(&output_file).display().to_string(),
                    renderer_status: None,
                    selected_recipe,
                    acceptance_summary: None,
                    error: Some(error),
                });
                if fail_fast {
                    for pending in compatible.iter().skip(index + 1) {
                        skipped_templates.push(SkippedFamilyTemplate {
                            template_id: pending.template_id.clone(),
                            template_name: pending.name.clone(),
                            template_dir: absolutize(&pending.path).display().to_string(),
                            reason: "not attempted because --fail-fast stopped the batch"
                                .to_string(),
                        });
                    }
                    break;
                }
            }
        }
    }

    let succeeded = outputs
        .iter()
        .filter(|output| output.status == "rendered")
        .count();
    let failed = outputs.len() - succeeded;
    let status = match (succeeded, failed) {
        (_, 0) => "completed",
        (0, _) => "failed",
        _ => "partial",
    };

    Ok(TemplateFamilyRenderResult {
        status,
        family: TemplateFamilyDescriptor {
            schema_id,
            format: family_format,
            name: family_name,
            anchor_template_id: anchor_validation.template_id,
        },
        input_file: absolutize(&source_path).display().to_string(),
        input_format,
        output_dir: output_dir.display().to_string(),
        base_name,
        template_count,
        succeeded,
        failed,
        outputs,
        skipped_templates,
    })
}

fn candidate_from_validation(
    path: &Path,
    validation: &crate::template_manifest::TemplateValidation,
) -> FamilyTemplateCandidate {
    FamilyTemplateCandidate {
        template_id: validation.template_id.clone(),
        name: validation.name.clone(),
        path: path.to_path_buf(),
        format: validation.format.clone(),
        input_formats: validation.input_formats.clone(),
        template_type: validation.template_type.clone(),
        renderer_type: validation.renderer_type.clone(),
        recipe_ids: recipe_ids(
            &validation.format,
            &validation.deck_recipe_ids,
            &validation.document_recipe_ids,
            &validation.workbook_recipe_ids,
        ),
    }
}

fn candidate_from_record(
    path: PathBuf,
    record: crate::storage::TemplatePackRecord,
) -> FamilyTemplateCandidate {
    FamilyTemplateCandidate {
        template_id: record.template_id,
        name: record.name,
        path,
        format: record.format.clone(),
        input_formats: record.input_formats,
        template_type: record.template_type,
        renderer_type: record.renderer_type,
        recipe_ids: recipe_ids(
            &record.format,
            &record.deck_recipe_ids,
            &record.document_recipe_ids,
            &record.workbook_recipe_ids,
        ),
    }
}

fn recipe_ids(
    format: &str,
    deck: &[String],
    document: &[String],
    workbook: &[String],
) -> Vec<String> {
    match format {
        "docx" => document.to_vec(),
        "xlsx" => workbook.to_vec(),
        _ => deck.to_vec(),
    }
}

fn recipe_for_candidate(
    candidate: &FamilyTemplateCandidate,
    preferred: Option<&str>,
) -> Option<String> {
    if candidate.template_type == "script"
        || (candidate.template_type == "hybrid" && candidate.renderer_type.is_some())
    {
        return None;
    }
    preferred
        .filter(|recipe| candidate.recipe_ids.iter().any(|item| item == recipe))
        .map(ToString::to_string)
        .or_else(|| candidate.recipe_ids.first().cloned())
}

fn supports_input_format(formats: &[String], input_format: &str) -> bool {
    let formats = if formats.is_empty() {
        vec!["json".to_string()]
    } else {
        formats
            .iter()
            .map(|format| crate::content_source::normalize_format(format))
            .collect()
    };
    formats.iter().any(|format| format == input_format)
}

fn family_name(candidates: &[FamilyTemplateCandidate], fallback: &str) -> String {
    let prefixes = candidates
        .iter()
        .map(|candidate| candidate.name.split(" · ").next().unwrap_or("").trim())
        .collect::<Vec<_>>();
    match prefixes.first() {
        Some(first) if !first.is_empty() && prefixes.iter().all(|prefix| prefix == first) => {
            (*first).to_string()
        }
        _ => fallback.to_string(),
    }
}

fn common_template_id_prefix(candidates: &[FamilyTemplateCandidate]) -> usize {
    let values = candidates
        .iter()
        .map(|candidate| candidate.template_id.split('-').collect::<Vec<_>>())
        .collect::<Vec<_>>();
    let shortest = values.iter().map(Vec::len).min().unwrap_or_default();
    (0..shortest)
        .take_while(|index| {
            values
                .iter()
                .all(|value| value[*index] == values[0][*index])
        })
        .count()
}

fn variant_slug(template_id: &str, shared_prefix: usize) -> String {
    let parts = template_id.split('-').collect::<Vec<_>>();
    let variant = parts.get(shared_prefix..).unwrap_or_default().join("-");
    let value = if variant.is_empty() {
        template_id
    } else {
        &variant
    };
    sanitize_ascii_file_part(value)
}

fn unique_variant_id(base: String, used: &mut HashSet<String>) -> String {
    if used.insert(base.clone()) {
        return base;
    }
    for index in 2.. {
        let candidate = format!("{base}-{index}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
    }
    unreachable!()
}

fn normalized_base_name(requested: Option<&str>, source_path: &Path) -> String {
    let raw = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            source_path
                .file_stem()
                .and_then(|value| value.to_str())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "office-output".to_string());
    sanitize_unicode_file_part(&raw)
}

fn sanitize_ascii_file_part(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    trim_repeated_hyphens(&cleaned, "variant")
}

fn sanitize_unicode_file_part(value: &str) -> String {
    let cleaned = value
        .chars()
        .map(|character| {
            if character.is_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    trim_repeated_hyphens(&cleaned, "office-output")
}

fn trim_repeated_hyphens(value: &str, fallback: &str) -> String {
    let mut cleaned = String::with_capacity(value.len());
    for character in value.chars() {
        if character != '-' || !cleaned.ends_with('-') {
            cleaned.push(character);
        }
    }
    let cleaned = cleaned.trim_matches('-');
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned.to_string()
    }
}

fn normalize_existing_path(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| absolutize(path))
}

fn path_identity(path: &Path) -> String {
    normalize_existing_path(path).display().to_string()
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

    fn candidate(id: &str, name: &str) -> FamilyTemplateCandidate {
        FamilyTemplateCandidate {
            template_id: id.to_string(),
            name: name.to_string(),
            path: PathBuf::from(id),
            format: "pptx".to_string(),
            input_formats: vec!["json".to_string()],
            template_type: "script".to_string(),
            renderer_type: Some("script.python".to_string()),
            recipe_ids: Vec::new(),
        }
    }

    #[test]
    fn family_variants_drop_the_shared_template_id_prefix() {
        let candidates = vec![
            candidate(
                "teaching-deck-style-classic-v2",
                "通用教学演示 · 经典样式 V2",
            ),
            candidate(
                "teaching-deck-style-modern-cards-v2",
                "通用教学演示 · 现代卡片式 V2",
            ),
            candidate(
                "teaching-deck-style-editorial-v2",
                "通用教学演示 · 简洁刊物式 V2",
            ),
        ];
        let prefix = common_template_id_prefix(&candidates);
        let variants = candidates
            .iter()
            .map(|candidate| variant_slug(&candidate.template_id, prefix))
            .collect::<Vec<_>>();
        assert_eq!(
            variants,
            vec!["classic-v2", "modern-cards-v2", "editorial-v2"]
        );
        assert_eq!(family_name(&candidates, "fallback"), "通用教学演示");
    }

    #[test]
    fn base_names_keep_chinese_text_and_remove_path_punctuation() {
        assert_eq!(
            normalized_base_name(Some("护理 教学 / 示例"), Path::new("content.json")),
            "护理-教学-示例"
        );
    }

    #[test]
    fn markdown_aliases_are_compatible() {
        assert!(supports_input_format(&["markdown".to_string()], "md"));
        assert!(!supports_input_format(&["json".to_string()], "md"));
    }
}
