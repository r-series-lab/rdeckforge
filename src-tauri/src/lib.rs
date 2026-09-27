use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use serde::Serialize;
use tauri::Manager;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct OfficeWorkflowRunResult {
    workflow: &'static str,
    stage: &'static str,
    validation: rdeckforge_core::content_ir::ContentWorkspaceValidation,
    repair_hints: Vec<rdeckforge_core::workflow::WorkflowRepairHint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    render: Option<rdeckforge_core::accepted_render::AcceptedOfficeRenderResult>,
}

#[tauri::command]
fn app_info() -> rdeckforge_core::capabilities::AppInfo {
    rdeckforge_core::capabilities::app_info()
}

#[tauri::command]
fn app_capabilities() -> rdeckforge_core::capabilities::AppCapabilities {
    rdeckforge_core::capabilities::capabilities()
}

#[tauri::command]
fn environment_diagnostics() -> rdeckforge_core::environment::EnvironmentDiagnostics {
    rdeckforge_core::environment::environment_diagnostics()
}

#[tauri::command]
fn list_content_profiles() -> Vec<rdeckforge_core::profiles::ContentProfile> {
    rdeckforge_core::profiles::list_profiles()
}

#[tauri::command]
fn build_prompt_pack(
    pack: String,
    brief: String,
    template: String,
    out: String,
) -> Result<rdeckforge_core::prompt_pack::PromptBuildResult, String> {
    let pack = resolve_prompt_pack_path(&pack);
    rdeckforge_core::prompt_pack::build_prompt(
        &pack,
        &PathBuf::from(brief),
        &PathBuf::from(template),
        &PathBuf::from(out),
    )
}

fn resolve_prompt_pack_path(pack: &str) -> PathBuf {
    let trimmed = pack.trim();
    let is_record_id = !trimmed.is_empty() && trimmed.chars().all(|ch| ch.is_ascii_digit());
    if trimmed.is_empty() || is_record_id {
        return default_prompt_pack_path();
    }

    let explicit = PathBuf::from(trimmed);
    if explicit.is_dir() {
        return explicit;
    }

    let by_id = default_prompt_pack_root().join(trimmed);
    if by_id.is_dir() {
        return by_id;
    }

    explicit
}

fn default_prompt_pack_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("examples")
        .join("prompt-packs")
}

fn default_prompt_pack_path() -> PathBuf {
    default_prompt_pack_root().join("template-contract-handoff-v1")
}

#[tauri::command]
fn validate_template_pack(
    template: String,
) -> Result<rdeckforge_core::template_manifest::TemplateValidation, String> {
    rdeckforge_core::template_manifest::validate_template_pack(&PathBuf::from(template))
}

#[tauri::command]
fn diagnose_template_readiness(
    template: String,
) -> Result<rdeckforge_core::template_readiness::TemplateReadinessReport, String> {
    rdeckforge_core::template_readiness::diagnose_template_readiness(&PathBuf::from(template))
}

#[tauri::command]
fn test_template_pack(
    template: String,
    case: Option<String>,
    out_dir: Option<String>,
) -> Result<rdeckforge_core::template_self_test::TemplateTestReport, String> {
    let out_dir = out_dir
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    rdeckforge_core::template_self_test::run_template_tests(
        &PathBuf::from(template),
        case.as_deref().filter(|value| !value.trim().is_empty()),
        out_dir.as_deref(),
    )
}

#[tauri::command]
fn create_pptx_template_pack_draft(
    input: String,
    out_dir: String,
) -> Result<rdeckforge_core::template_manifest::PptxTemplatePackDraft, String> {
    rdeckforge_core::template_manifest::create_pptx_template_pack_draft(
        &PathBuf::from(input),
        &PathBuf::from(out_dir),
    )
}

#[tauri::command]
fn create_docx_template_pack_draft(
    input: String,
    out_dir: String,
) -> Result<rdeckforge_core::template_manifest::DocxTemplatePackDraft, String> {
    rdeckforge_core::template_manifest::create_docx_template_pack_draft(
        &PathBuf::from(input),
        &PathBuf::from(out_dir),
    )
}

#[tauri::command]
fn create_xlsx_template_pack_draft(
    input: String,
    out_dir: String,
) -> Result<rdeckforge_core::template_manifest::XlsxTemplatePackDraft, String> {
    rdeckforge_core::template_manifest::create_xlsx_template_pack_draft(
        &PathBuf::from(input),
        &PathBuf::from(out_dir),
    )
}

#[tauri::command]
fn export_template_contract(
    template: String,
    out: Option<String>,
) -> Result<rdeckforge_core::template_contract::TemplateContractResult, String> {
    let out_path = out
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    rdeckforge_core::template_contract::export_template_contract(
        &PathBuf::from(template),
        out_path.as_deref(),
    )
}

#[tauri::command]
fn export_content_skeleton(
    template: String,
    out: Option<String>,
    mode: Option<String>,
    recipe: Option<String>,
) -> Result<rdeckforge_core::template_contract::TemplateContentSkeletonResult, String> {
    let out_path = out
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let mode = mode.as_deref().filter(|value| !value.trim().is_empty());
    let recipe = recipe.as_deref().filter(|value| !value.trim().is_empty());
    rdeckforge_core::template_contract::export_content_skeleton(
        &PathBuf::from(template),
        out_path.as_deref(),
        mode,
        recipe,
    )
}

#[tauri::command]
fn create_script_template_adapter(
    script: String,
    out_dir: String,
    id: String,
    name: String,
    format: Option<String>,
    input_format: Option<String>,
    renderer_type: Option<String>,
    runtime: Option<String>,
    copy_script: Option<bool>,
    force: Option<bool>,
) -> Result<rdeckforge_core::script_render::ScriptTemplateAdapter, String> {
    let format = format.as_deref().unwrap_or("pptx");
    let input_format = input_format.as_deref().unwrap_or("json");
    let script_path = PathBuf::from(script);
    let out_dir_path = PathBuf::from(out_dir);
    rdeckforge_core::script_render::create_script_template_adapter(
        rdeckforge_core::script_render::ScriptTemplateAdapterOptions {
            script: &script_path,
            out_dir: &out_dir_path,
            template_id: &id,
            name: &name,
            format,
            input_format,
            renderer_type: renderer_type.as_deref(),
            runtime: runtime.as_deref(),
            copy_script: copy_script.unwrap_or(true),
            force: force.unwrap_or(false),
        },
    )
}

#[tauri::command]
fn list_template_packs() -> Result<Vec<rdeckforge_core::storage::TemplatePackRecord>, String> {
    rdeckforge_core::storage::list_template_packs()
}

#[tauri::command]
fn link_template_pack(
    template: String,
) -> Result<rdeckforge_core::storage::TemplatePackRecord, String> {
    rdeckforge_core::storage::link_template_pack(&PathBuf::from(template))
}

#[tauri::command]
fn export_template_pack_archive(
    template: String,
    output: String,
    force: Option<bool>,
) -> Result<rdeckforge_core::template_archive::TemplateArchiveExportResult, String> {
    rdeckforge_core::template_archive::export_template_pack_archive(
        &PathBuf::from(template),
        &PathBuf::from(output),
        force.unwrap_or(false),
    )
}

#[tauri::command]
fn import_template_pack_archive(
    archive: String,
    out_dir: String,
    force: Option<bool>,
) -> Result<rdeckforge_core::template_archive::LinkedTemplateArchiveImport, String> {
    rdeckforge_core::template_archive::import_and_link_template_pack_archive(
        &PathBuf::from(archive),
        &PathBuf::from(out_dir),
        force.unwrap_or(false),
    )
}

#[tauri::command]
fn refresh_template_pack(
    id: String,
) -> Result<rdeckforge_core::storage::TemplatePackRecord, String> {
    rdeckforge_core::storage::refresh_template_pack(&id)
}

#[tauri::command]
fn relink_template_pack(
    id: String,
    template: String,
) -> Result<rdeckforge_core::storage::TemplatePackRecord, String> {
    rdeckforge_core::storage::relink_template_pack(&id, &PathBuf::from(template))
}

#[tauri::command]
fn forget_template_pack(
    id: String,
) -> Result<rdeckforge_core::storage::TemplatePackRemoval, String> {
    rdeckforge_core::storage::forget_template_pack(&id)
}

#[tauri::command]
fn validate_content(
    input: String,
) -> Result<rdeckforge_core::content_ir::ContentValidation, String> {
    rdeckforge_core::content_ir::validate_content(&PathBuf::from(input))
}

#[tauri::command]
fn validate_content_file(
    input: String,
    template: Option<String>,
    recipe: Option<String>,
    profile: Option<String>,
) -> Result<rdeckforge_core::content_ir::ContentWorkspaceValidation, String> {
    let template_path = template
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let recipe = recipe.as_deref().filter(|value| !value.trim().is_empty());
    let profile = profile.as_deref().filter(|value| !value.trim().is_empty());
    rdeckforge_core::content_ir::validate_content_workspace_file_with_profile(
        &PathBuf::from(input),
        template_path.as_deref(),
        recipe,
        profile,
    )
}

#[tauri::command]
fn validate_content_text(
    content: String,
    input_format: Option<String>,
    template: Option<String>,
    recipe: Option<String>,
    profile: Option<String>,
) -> Result<rdeckforge_core::content_ir::ContentWorkspaceValidation, String> {
    let template_path = template
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let recipe = recipe.as_deref().filter(|value| !value.trim().is_empty());
    let profile = profile.as_deref().filter(|value| !value.trim().is_empty());
    let input_format = input_format.as_deref().unwrap_or("json");
    rdeckforge_core::content_ir::validate_content_workspace_text_with_profile(
        &content,
        input_format,
        template_path.as_deref(),
        recipe,
        profile,
    )
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let path = PathBuf::from(path);
    rdeckforge_core::validator::require_file(&path, "text file")?;
    fs::read_to_string(&path).map_err(|err| format!("failed to read '{}': {err}", path.display()))
}

#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create '{}': {err}", parent.display()))?;
    }
    fs::write(&path, content).map_err(|err| format!("failed to write '{}': {err}", path.display()))
}

#[tauri::command]
fn build_pptx_render_job(
    template: String,
    recipe: Option<String>,
    input: String,
    markdown: Option<String>,
    output: String,
) -> Result<rdeckforge_core::render_job::RenderJob, String> {
    let markdown_path = markdown.map(PathBuf::from);
    rdeckforge_core::render_job::build_pptx_job(
        &PathBuf::from(template),
        &PathBuf::from(input),
        markdown_path.as_deref(),
        &PathBuf::from(output),
        recipe.as_deref(),
    )
}

#[tauri::command]
fn render_pptx(
    template: String,
    recipe: Option<String>,
    input: String,
    markdown: Option<String>,
    output: String,
) -> Result<rdeckforge_core::renderer::PptxRenderResult, String> {
    let output_path = PathBuf::from(output);
    let markdown_path = markdown.map(PathBuf::from);
    let job = rdeckforge_core::render_job::build_pptx_job(
        &PathBuf::from(template),
        &PathBuf::from(input),
        markdown_path.as_deref(),
        &output_path,
        recipe.as_deref(),
    )?;
    render_and_record(job, output_path)
}

#[tauri::command]
fn render_docx(
    input: String,
    template: Option<String>,
    recipe: Option<String>,
    output: String,
) -> Result<rdeckforge_core::office_export::OfficeExportResult, String> {
    let template_path = template
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let recipe = recipe.as_deref().filter(|value| !value.trim().is_empty());
    rdeckforge_core::office_export::render_docx_atomic(
        &PathBuf::from(input),
        template_path.as_deref(),
        recipe,
        &PathBuf::from(output),
    )
}

#[tauri::command]
fn render_xlsx(
    input: String,
    template: Option<String>,
    recipe: Option<String>,
    output: String,
) -> Result<rdeckforge_core::office_export::OfficeExportResult, String> {
    let template_path = template
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let recipe = recipe.as_deref().filter(|value| !value.trim().is_empty());
    rdeckforge_core::office_export::render_xlsx_atomic(
        &PathBuf::from(input),
        template_path.as_deref(),
        recipe,
        &PathBuf::from(output),
    )
}

#[tauri::command]
fn render_accepted_office(
    input: String,
    template: String,
    recipe: Option<String>,
    output: String,
) -> Result<rdeckforge_core::accepted_render::AcceptedOfficeRenderResult, String> {
    let recipe = recipe.as_deref().filter(|value| !value.trim().is_empty());
    rdeckforge_core::accepted_render::render_accepted_office(
        &PathBuf::from(input),
        &PathBuf::from(template),
        recipe,
        &PathBuf::from(output),
    )
}

#[tauri::command]
fn inspect_office_workflow(
    input: String,
    template: String,
    recipe: Option<String>,
) -> Result<OfficeWorkflowRunResult, String> {
    let recipe = recipe.as_deref().filter(|value| !value.trim().is_empty());
    let input = PathBuf::from(input);
    let template = PathBuf::from(template);
    let validation = rdeckforge_core::content_ir::validate_content_workspace_file_with_profile(
        &input,
        Some(&template),
        recipe,
        None,
    )?;
    let repair_hints = rdeckforge_core::workflow::repair_hints(&validation);
    Ok(OfficeWorkflowRunResult {
        workflow: "run",
        stage: "validate",
        validation,
        repair_hints,
        render: None,
    })
}

#[tauri::command]
fn run_office_workflow(
    input: String,
    template: String,
    recipe: Option<String>,
    output: String,
) -> Result<OfficeWorkflowRunResult, String> {
    let recipe = recipe.as_deref().filter(|value| !value.trim().is_empty());
    let input = PathBuf::from(input);
    let template = PathBuf::from(template);
    let output = PathBuf::from(output);
    let validation = rdeckforge_core::content_ir::validate_content_workspace_file_with_profile(
        &input,
        Some(&template),
        recipe,
        None,
    )?;
    let repair_hints = rdeckforge_core::workflow::repair_hints(&validation);
    if !validation.acceptance_summary.can_render {
        return Ok(OfficeWorkflowRunResult {
            workflow: "run",
            stage: "validate",
            validation,
            repair_hints,
            render: None,
        });
    }

    let render =
        rdeckforge_core::accepted_render::render_accepted_office(&input, &template, recipe, &output)?;
    Ok(OfficeWorkflowRunResult {
        workflow: "run",
        stage: "rendered",
        validation,
        repair_hints,
        render: Some(render),
    })
}

#[tauri::command]
fn export_xlsx_report(
    input: String,
    template: Option<String>,
    recipe: Option<String>,
    output: String,
) -> Result<rdeckforge_core::office_export::OfficeExportResult, String> {
    let template_path = template
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);
    let recipe = recipe.as_deref().filter(|value| !value.trim().is_empty());
    rdeckforge_core::office_export::export_xlsx_report(
        &PathBuf::from(input),
        template_path.as_deref(),
        recipe,
        &PathBuf::from(output),
    )
}

#[tauri::command]
fn list_render_history(
    limit: Option<usize>,
) -> Result<Vec<rdeckforge_core::storage::RenderHistoryRecord>, String> {
    rdeckforge_core::storage::list_render_history(limit.unwrap_or(50))
}

#[tauri::command]
fn rerender_history(
    id: String,
) -> Result<rdeckforge_core::accepted_render::AcceptedOfficeRenderResult, String> {
    rdeckforge_core::storage::rerender_history(&id)
}

#[tauri::command]
fn resolve_output_path(
    path: String,
    conflict_policy: String,
) -> Result<rdeckforge_core::output_policy::OutputPathResolution, String> {
    rdeckforge_core::output_policy::resolve_output_path(&PathBuf::from(path), &conflict_policy)
}

#[tauri::command]
async fn list_workflow_tasks(
    limit: Option<usize>,
) -> Result<Vec<rdeckforge_core::storage::WorkflowTaskRecord>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        rdeckforge_core::storage::list_workflow_tasks(limit.unwrap_or(50))
    })
    .await
    .map_err(|err| format!("workflow task storage failed: {err}"))?
}

#[tauri::command]
async fn save_workflow_task(
    task: rdeckforge_core::storage::WorkflowTaskInput,
) -> Result<rdeckforge_core::storage::WorkflowTaskRecord, String> {
    tauri::async_runtime::spawn_blocking(move || {
        rdeckforge_core::storage::save_workflow_task(&task)
    })
    .await
    .map_err(|err| format!("workflow task storage failed: {err}"))?
}

#[tauri::command]
async fn delete_workflow_task(
    id: String,
) -> Result<rdeckforge_core::storage::WorkflowTaskRemoval, String> {
    tauri::async_runtime::spawn_blocking(move || {
        rdeckforge_core::storage::delete_workflow_task(&id)
    })
    .await
    .map_err(|err| format!("workflow task storage failed: {err}"))?
}

fn render_and_record(
    job: rdeckforge_core::render_job::RenderJob,
    output_path: PathBuf,
) -> Result<rdeckforge_core::renderer::PptxRenderResult, String> {
    let result = rdeckforge_core::renderer::render_pptx_job_atomic(&job, &output_path)?;
    rdeckforge_core::storage::record_render_history(&job, &result)?;
    Ok(result)
}

#[tauri::command]
fn open_path(path: String) -> Result<(), String> {
    let path = normalize_existing_path(&path)?;
    open_with_system(&path, OpenMode::Open)
}

#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    let path = normalize_existing_path(&path)?;
    open_with_system(&path, OpenMode::Reveal)
}

enum OpenMode {
    Open,
    Reveal,
}

fn normalize_existing_path(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(format!("path does not exist: {}", path.display()));
    }
    path.canonicalize()
        .map_err(|err| format!("failed to normalize path '{}': {err}", path.display()))
}

fn open_with_system(path: &Path, mode: OpenMode) -> Result<(), String> {
    let mut command = if cfg!(target_os = "macos") {
        let mut command = Command::new("open");
        if matches!(mode, OpenMode::Reveal) {
            command.arg("-R");
        }
        command.arg(path);
        command
    } else if cfg!(target_os = "windows") {
        let mut command = Command::new("explorer");
        if matches!(mode, OpenMode::Reveal) {
            command.arg(format!("/select,{}", path.display()));
        } else {
            command.arg(path);
        }
        command
    } else {
        let mut command = Command::new("xdg-open");
        if matches!(mode, OpenMode::Reveal) && path.is_file() {
            command.arg(path.parent().unwrap_or(path));
        } else {
            command.arg(path);
        }
        command
    };

    let status = command
        .status()
        .map_err(|err| format!("failed to open path '{}': {err}", path.display()))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "system opener failed for '{}' with status {status}",
            path.display()
        ))
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            configure_pptx_sidecar_resource(app);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            app_capabilities,
            environment_diagnostics,
            list_content_profiles,
            build_prompt_pack,
            validate_template_pack,
            diagnose_template_readiness,
            test_template_pack,
            create_pptx_template_pack_draft,
            create_docx_template_pack_draft,
            create_xlsx_template_pack_draft,
            export_template_contract,
            export_content_skeleton,
            create_script_template_adapter,
            list_template_packs,
            link_template_pack,
            export_template_pack_archive,
            import_template_pack_archive,
            refresh_template_pack,
            relink_template_pack,
            forget_template_pack,
            validate_content,
            validate_content_file,
            validate_content_text,
            read_text_file,
            write_text_file,
            build_pptx_render_job,
            render_pptx,
            render_docx,
            render_xlsx,
            render_accepted_office,
            inspect_office_workflow,
            run_office_workflow,
            export_xlsx_report,
            list_render_history,
            rerender_history,
            resolve_output_path,
            list_workflow_tasks,
            save_workflow_task,
            delete_workflow_task,
            open_path,
            reveal_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running rDeckForge");
}

fn configure_pptx_sidecar_resource<R: tauri::Runtime>(app: &tauri::App<R>) {
    let Ok(resource_dir) = app.path().resource_dir() else {
        return;
    };
    let sidecar_name = if cfg!(target_os = "windows") {
        "rdeckforge-pptx.exe"
    } else {
        "rdeckforge-pptx"
    };
    let mut candidates = Vec::new();
    if let Ok(current_exe) = std::env::current_exe() {
        if let Some(executable_dir) = current_exe.parent() {
            candidates.push(executable_dir.join(sidecar_name));
        }
    }
    candidates.extend([
        resource_dir.join(sidecar_name),
        resource_dir
            .join("renderers")
            .join("pptx-node")
            .join("dist")
            .join("index.js"),
        resource_dir.join("dist").join("index.js"),
        resource_dir.join("index.js"),
    ]);
    for candidate in candidates {
        if candidate.is_file() {
            let _ = rdeckforge_core::renderer::set_pptx_renderer_override(candidate);
            return;
        }
    }
}
