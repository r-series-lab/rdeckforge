use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedOfficeRenderResult {
    pub renderer_status: &'static str,
    pub format: String,
    pub output_file: String,
    pub input_file: String,
    pub template_dir: String,
    pub selected_recipe: Option<String>,
    pub planned_page_count: Option<usize>,
    pub acceptance_summary: crate::content_ir::AcceptanceSummary,
    pub output_integrity: crate::office_package::OfficePackageIntegrity,
    pub render_result: Value,
    pub history: Option<crate::storage::RenderHistoryRecord>,
}

pub fn render_accepted_office(
    input: &Path,
    template: &Path,
    recipe: Option<&str>,
    out: &Path,
) -> Result<AcceptedOfficeRenderResult, String> {
    let manifest = crate::template_manifest::load_manifest(template)?;
    let template_type = crate::template_manifest::normalized_template_type(&manifest)?;
    if template_type == "script" || (template_type == "hybrid" && manifest.renderer.is_some()) {
        return render_accepted_script_template(input, template, out, &manifest.format);
    }

    let validation = crate::content_ir::validate_content_workspace_file_with_profile(
        input,
        Some(template),
        recipe,
        None,
    )?;
    let acceptance_summary = validation.acceptance_summary.clone();
    if !acceptance_summary.can_render {
        return Err(format!(
            "AI output is not accepted for rendering: {}",
            acceptance_summary.message
        ));
    }

    let format = validation
        .render_format
        .clone()
        .ok_or_else(|| "template format is required for accepted Office rendering".to_string())?;
    let selected_recipe = validation
        .selected_recipe
        .clone()
        .or_else(|| recipe.map(ToString::to_string));
    let planned_page_count = if format == "pptx" {
        Some(validation.planned_pages.len())
    } else {
        None
    };

    let mut transaction = crate::office_package::AtomicOfficeOutput::new(out, &format)?;
    let staged_output = transaction.staging_path().to_path_buf();
    let final_output = transaction.final_path().to_path_buf();
    let pipeline = || {
        crate::script_render::run_pipeline_steps(
            template,
            &manifest.pipeline,
            input,
            &staged_output,
        )
    };

    let (render_result, history, output_integrity) = match format.as_str() {
        "pptx" => {
            let mut job = crate::render_job::build_pptx_job(
                template,
                input,
                None,
                &staged_output,
                selected_recipe.as_deref(),
            )?;
            let mut result = crate::renderer::render_pptx_job(&job, &staged_output)?;
            let pipeline_result = pipeline()?;
            let integrity = transaction.validate()?;
            transaction.commit()?;
            job.output_file = final_output.display().to_string();
            result.output_file = final_output.display().to_string();
            result.output_integrity = Some(integrity.clone());
            let final_job_file = crate::renderer::write_render_job_metadata(&job, &final_output)?;
            result.job_file = final_job_file.display().to_string();
            let result_value = serde_json::to_value(&result)
                .map_err(|err| format!("failed to serialize PPTX render result: {err}"))?;
            let result_value = normalize_output_paths(
                with_pipeline(result_value, pipeline_result),
                &staged_output,
                &final_output,
            );
            let history = optional_history(crate::storage::record_render_history(&job, &result));
            (result_value, history, integrity)
        }
        "docx" => {
            let mut result = crate::office_export::render_docx(
                input,
                Some(template),
                selected_recipe.as_deref(),
                &staged_output,
            )?;
            let pipeline_result = pipeline()?;
            let integrity = transaction.validate()?;
            transaction.commit()?;
            result.output_file = final_output.display().to_string();
            result.output_integrity = Some(integrity.clone());
            let result_value = serde_json::to_value(result)
                .map_err(|err| format!("failed to serialize DOCX render result: {err}"))?;
            let result_value = normalize_output_paths(
                with_pipeline(result_value, pipeline_result),
                &staged_output,
                &final_output,
            );
            let history = optional_history(crate::storage::record_accepted_render_history(
                &format,
                selected_recipe.as_deref(),
                template,
                input,
                &final_output,
                planned_page_count,
                &acceptance_summary,
                &result_value,
            ));
            (result_value, history, integrity)
        }
        "xlsx" => {
            let mut result = crate::office_export::render_xlsx(
                input,
                Some(template),
                selected_recipe.as_deref(),
                &staged_output,
            )?;
            let pipeline_result = pipeline()?;
            let integrity = transaction.validate()?;
            transaction.commit()?;
            result.output_file = final_output.display().to_string();
            result.output_integrity = Some(integrity.clone());
            let result_value = serde_json::to_value(result)
                .map_err(|err| format!("failed to serialize XLSX render result: {err}"))?;
            let result_value = normalize_output_paths(
                with_pipeline(result_value, pipeline_result),
                &staged_output,
                &final_output,
            );
            let history = optional_history(crate::storage::record_accepted_render_history(
                &format,
                selected_recipe.as_deref(),
                template,
                input,
                &final_output,
                planned_page_count,
                &acceptance_summary,
                &result_value,
            ));
            (result_value, history, integrity)
        }
        other => {
            return Err(format!(
                "unsupported template format for accepted Office rendering: {other}"
            ));
        }
    };

    Ok(AcceptedOfficeRenderResult {
        renderer_status: "rendered",
        format,
        output_file: final_output.display().to_string(),
        input_file: absolutize(input).display().to_string(),
        template_dir: absolutize(template).display().to_string(),
        selected_recipe,
        planned_page_count,
        acceptance_summary,
        output_integrity,
        render_result,
        history,
    })
}

fn with_pipeline(
    render_result: Value,
    pipeline: Vec<crate::script_render::ScriptStepResult>,
) -> Value {
    if pipeline.is_empty() {
        render_result
    } else {
        json!({
            "renderer": render_result,
            "pipeline": pipeline
        })
    }
}

fn optional_history(
    result: Result<crate::storage::RenderHistoryRecord, String>,
) -> Option<crate::storage::RenderHistoryRecord> {
    result.ok()
}

fn render_accepted_script_template(
    input: &Path,
    template: &Path,
    out: &Path,
    declared_format: &str,
) -> Result<AcceptedOfficeRenderResult, String> {
    let mut transaction = crate::office_package::AtomicOfficeOutput::new(out, declared_format)?;
    let staged_output = transaction.staging_path().to_path_buf();
    let final_output = transaction.final_path().to_path_buf();
    let mut result = crate::script_render::render_script_template(input, template, &staged_output)?;
    let format = result.format.clone();
    if format != declared_format {
        return Err(format!(
            "script renderer format '{format}' does not match template format '{declared_format}'"
        ));
    }
    let renderer_type = result.renderer_type.clone();
    let template_warning_count = result.template_warnings.len();
    let status = if template_warning_count == 0 {
        crate::content_ir::AcceptanceStatus::Pass
    } else {
        crate::content_ir::AcceptanceStatus::Warn
    };
    let acceptance_summary = crate::content_ir::AcceptanceSummary {
        status,
        can_render: true,
        schema_error_count: 0,
        content_warning_count: 0,
        template_warning_count,
        binding_warning_count: 0,
        asset_warning_count: 0,
        planned_page_count: 0,
        message: format!("Script template rendered with {renderer_type}"),
    };
    let output_integrity = transaction.validate()?;
    transaction.commit()?;
    result.output_file = final_output.display().to_string();
    result.output_integrity = Some(output_integrity.clone());
    let render_result = normalize_output_paths(
        serde_json::to_value(result)
            .map_err(|err| format!("failed to serialize script render result: {err}"))?,
        &staged_output,
        &final_output,
    );
    let history = crate::storage::record_accepted_render_history(
        &format,
        None,
        template,
        input,
        &final_output,
        None,
        &acceptance_summary,
        &render_result,
    )?;

    Ok(AcceptedOfficeRenderResult {
        renderer_status: "rendered",
        format,
        output_file: final_output.display().to_string(),
        input_file: absolutize(input).display().to_string(),
        template_dir: absolutize(template).display().to_string(),
        selected_recipe: None,
        planned_page_count: None,
        acceptance_summary,
        output_integrity,
        render_result,
        history: Some(history),
    })
}

fn normalize_output_paths(mut value: Value, staged: &Path, final_output: &Path) -> Value {
    let staged = staged.display().to_string();
    let final_output = final_output.display().to_string();
    replace_path_in_value(&mut value, &staged, &final_output);
    value
}

fn replace_path_in_value(value: &mut Value, staged: &str, final_output: &str) {
    match value {
        Value::String(text) => {
            if text.contains(staged) {
                *text = text.replace(staged, final_output);
            }
        }
        Value::Array(values) => {
            for value in values {
                replace_path_in_value(value, staged, final_output);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                replace_path_in_value(value, staged, final_output);
            }
        }
        _ => {}
    }
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
