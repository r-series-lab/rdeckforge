use chrono::Utc;
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePackRecord {
    pub id: String,
    pub path: String,
    pub path_available: bool,
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
    pub preview_cover: Option<String>,
    pub preview_slides: Vec<crate::template_manifest::TemplatePreviewSlide>,
    pub health_status: String,
    pub health_checked: Vec<String>,
    pub health_warnings: Vec<String>,
    pub warnings: Vec<String>,
    pub linked_at: String,
    pub last_validated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplatePackRemoval {
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderHistoryRecord {
    pub id: String,
    pub job_id: String,
    pub created_at: String,
    pub rendered_at: String,
    pub format: String,
    pub deck_recipe: Option<String>,
    pub template_dir: String,
    pub content_file: String,
    pub markdown_file: Option<String>,
    pub output_file: String,
    pub job_file: String,
    pub sidecar_file: String,
    pub planned_page_count: usize,
    pub visible_slide_count: Option<usize>,
    pub content_check_status: Option<String>,
    pub checked_bindings: Option<usize>,
    pub missing_bindings: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowTaskInput {
    pub id: Option<String>,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub stage: String,
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub template_path: String,
    #[serde(default)]
    pub content_path: String,
    pub output_path: Option<String>,
    pub format: Option<String>,
    pub recipe: Option<String>,
    pub brief_path: Option<String>,
    pub prompt_path: Option<String>,
    pub validation_status: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowTaskRecord {
    pub id: String,
    pub name: String,
    pub status: String,
    pub stage: String,
    pub source: String,
    pub template_path: String,
    pub content_path: String,
    pub output_path: Option<String>,
    pub format: Option<String>,
    pub recipe: Option<String>,
    pub brief_path: Option<String>,
    pub prompt_path: Option<String>,
    pub validation_status: Option<String>,
    pub last_error: Option<String>,
    pub template_available: bool,
    pub content_available: bool,
    pub output_available: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowTaskRemoval {
    pub removed: bool,
}

pub fn default_storage_path() -> PathBuf {
    storage_path_for(std::env::consts::OS, |key| std::env::var_os(key))
}

fn storage_path_for<F>(platform: &str, mut env_var: F) -> PathBuf
where
    F: FnMut(&str) -> Option<OsString>,
{
    if let Some(configured) = non_empty_path(env_var("RDECKFORGE_DATA_DIR")) {
        return configured.join("rdeckforge.sqlite3");
    }

    let data_dir = match platform {
        "macos" => non_empty_path(env_var("HOME"))
            .map(|home| home.join("Library").join("Application Support")),
        "windows" => non_empty_path(env_var("LOCALAPPDATA"))
            .or_else(|| non_empty_path(env_var("APPDATA")))
            .or_else(|| {
                non_empty_path(env_var("USERPROFILE"))
                    .map(|home| home.join("AppData").join("Local"))
            }),
        _ => non_empty_path(env_var("XDG_DATA_HOME")).or_else(|| {
            non_empty_path(env_var("HOME")).map(|home| home.join(".local").join("share"))
        }),
    }
    .unwrap_or_else(|| PathBuf::from("."));

    data_dir.join("rDeckForge").join("rdeckforge.sqlite3")
}

fn non_empty_path(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

pub fn list_template_packs() -> Result<Vec<TemplatePackRecord>, String> {
    let conn = open_storage()?;
    let mut statement = conn
        .prepare(
            "SELECT id, path, template_id, name, format, entry_path, page_template_count,
                    deck_recipe_count, block_template_count, document_recipe_count,
                    sheet_template_count, workbook_recipe_count, page_template_ids_json,
                    deck_recipe_ids_json, block_template_ids_json, document_recipe_ids_json,
                    sheet_template_ids_json, workbook_recipe_ids_json, legacy_layout_count,
                    input_formats_json, input_profile, input_profile_name, input_schema,
                    input_schema_id, input_builtin_schema, md_profile, health_status,
                    health_checked_json, health_warnings_json,
                    warnings_json, linked_at, last_validated_at, template_type, renderer_type,
                    renderer_entry, pipeline_step_count
             FROM template_packs
             ORDER BY last_validated_at DESC, name ASC",
        )
        .map_err(|err| format!("failed to prepare template pack list query: {err}"))?;
    let rows = statement
        .query_map([], row_to_template_pack)
        .map_err(|err| format!("failed to query template packs: {err}"))?;

    let mut records = Vec::new();
    for row in rows {
        records.push(row.map_err(|err| format!("failed to read template pack row: {err}"))?);
    }
    Ok(records)
}

pub fn link_template_pack(template_dir: &Path) -> Result<TemplatePackRecord, String> {
    let template_dir = normalize_template_dir(template_dir)?;
    let validation = crate::template_manifest::validate_template_pack(&template_dir)?;
    let conn = open_storage()?;
    let path = template_dir.display().to_string();
    let existing = conn
        .query_row(
            "SELECT id, linked_at FROM template_packs WHERE path = ?1",
            params![&path],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()
        .map_err(|err| format!("failed to inspect existing template pack: {err}"))?;

    let now = Utc::now().to_rfc3339();
    let (id, linked_at) =
        existing.unwrap_or_else(|| (uuid::Uuid::new_v4().to_string(), now.clone()));
    let warnings_json = serde_json::to_string(&validation.warnings)
        .map_err(|err| format!("failed to serialize template warnings: {err}"))?;
    let page_template_ids_json = serde_json::to_string(&validation.page_template_ids)
        .map_err(|err| format!("failed to serialize page template ids: {err}"))?;
    let deck_recipe_ids_json = serde_json::to_string(&validation.deck_recipe_ids)
        .map_err(|err| format!("failed to serialize deck recipe ids: {err}"))?;
    let block_template_ids_json = serde_json::to_string(&validation.block_template_ids)
        .map_err(|err| format!("failed to serialize block template ids: {err}"))?;
    let document_recipe_ids_json = serde_json::to_string(&validation.document_recipe_ids)
        .map_err(|err| format!("failed to serialize document recipe ids: {err}"))?;
    let sheet_template_ids_json = serde_json::to_string(&validation.sheet_template_ids)
        .map_err(|err| format!("failed to serialize sheet template ids: {err}"))?;
    let workbook_recipe_ids_json = serde_json::to_string(&validation.workbook_recipe_ids)
        .map_err(|err| format!("failed to serialize workbook recipe ids: {err}"))?;
    let input_formats_json = serde_json::to_string(&validation.input_formats)
        .map_err(|err| format!("failed to serialize input formats: {err}"))?;
    let health_checked_json = serde_json::to_string(&validation.health.checked)
        .map_err(|err| format!("failed to serialize template health checks: {err}"))?;
    let health_warnings_json = serde_json::to_string(&validation.health.warnings)
        .map_err(|err| format!("failed to serialize template health warnings: {err}"))?;

    conn.execute(
        "INSERT INTO template_packs (
             id, path, template_id, name, format, template_type, entry_path, renderer_type,
             renderer_entry, pipeline_step_count, page_template_count,
             deck_recipe_count, block_template_count, document_recipe_count,
             sheet_template_count, workbook_recipe_count, page_template_ids_json,
             deck_recipe_ids_json, block_template_ids_json, document_recipe_ids_json,
             sheet_template_ids_json, workbook_recipe_ids_json, legacy_layout_count,
             input_formats_json, input_profile, input_profile_name, input_schema,
             input_schema_id, input_builtin_schema, md_profile, health_status,
             health_checked_json, health_warnings_json, warnings_json, linked_at,
             last_validated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32, ?33, ?34, ?35, ?36)
         ON CONFLICT(path) DO UPDATE SET
             template_id = excluded.template_id,
             name = excluded.name,
             format = excluded.format,
             template_type = excluded.template_type,
             entry_path = excluded.entry_path,
             renderer_type = excluded.renderer_type,
             renderer_entry = excluded.renderer_entry,
             pipeline_step_count = excluded.pipeline_step_count,
             page_template_count = excluded.page_template_count,
             deck_recipe_count = excluded.deck_recipe_count,
             block_template_count = excluded.block_template_count,
             document_recipe_count = excluded.document_recipe_count,
             sheet_template_count = excluded.sheet_template_count,
             workbook_recipe_count = excluded.workbook_recipe_count,
             page_template_ids_json = excluded.page_template_ids_json,
             deck_recipe_ids_json = excluded.deck_recipe_ids_json,
             block_template_ids_json = excluded.block_template_ids_json,
             document_recipe_ids_json = excluded.document_recipe_ids_json,
             sheet_template_ids_json = excluded.sheet_template_ids_json,
             workbook_recipe_ids_json = excluded.workbook_recipe_ids_json,
             legacy_layout_count = excluded.legacy_layout_count,
             input_formats_json = excluded.input_formats_json,
             input_profile = excluded.input_profile,
             input_profile_name = excluded.input_profile_name,
             input_schema = excluded.input_schema,
             input_schema_id = excluded.input_schema_id,
             input_builtin_schema = excluded.input_builtin_schema,
             md_profile = excluded.md_profile,
             health_status = excluded.health_status,
             health_checked_json = excluded.health_checked_json,
             health_warnings_json = excluded.health_warnings_json,
             warnings_json = excluded.warnings_json,
             last_validated_at = excluded.last_validated_at",
        params![
            &id,
            &path,
            &validation.template_id,
            &validation.name,
            &validation.format,
            &validation.template_type,
            &validation.entry_path,
            validation.renderer_type.as_deref(),
            validation.renderer_entry.as_deref(),
            validation.pipeline_step_count as i64,
            validation.page_template_count as i64,
            validation.deck_recipe_count as i64,
            validation.block_template_count as i64,
            validation.document_recipe_count as i64,
            validation.sheet_template_count as i64,
            validation.workbook_recipe_count as i64,
            &page_template_ids_json,
            &deck_recipe_ids_json,
            &block_template_ids_json,
            &document_recipe_ids_json,
            &sheet_template_ids_json,
            &workbook_recipe_ids_json,
            validation.legacy_layout_count as i64,
            &input_formats_json,
            validation.input_profile.as_deref(),
            validation.input_profile_name.as_deref(),
            validation.input_schema.as_deref(),
            validation.input_schema_id.as_deref(),
            validation.input_builtin_schema as i64,
            validation.md_profile.as_deref(),
            &validation.health.status,
            &health_checked_json,
            &health_warnings_json,
            &warnings_json,
            &linked_at,
            &now,
        ],
    )
    .map_err(|err| format!("failed to save template pack: {err}"))?;

    load_template_pack(&conn, &id)
}

pub fn refresh_template_pack(id: &str) -> Result<TemplatePackRecord, String> {
    let conn = open_storage()?;
    let path = conn
        .query_row(
            "SELECT path FROM template_packs WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| format!("failed to read template pack path: {err}"))?
        .ok_or_else(|| format!("template pack not found: {id}"))?;
    link_template_pack(Path::new(&path))
}

pub fn relink_template_pack(id: &str, template_dir: &Path) -> Result<TemplatePackRecord, String> {
    let template_dir = normalize_template_dir(template_dir)?;
    let validation = crate::template_manifest::validate_template_pack(&template_dir)?;
    let conn = open_storage()?;
    let existing = conn
        .query_row(
            "SELECT path, template_id, linked_at FROM template_packs WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()
        .map_err(|err| format!("failed to read template pack for relink: {err}"))?
        .ok_or_else(|| format!("template pack not found: {id}"))?;
    ensure_relink_identity(&existing.1, &validation.template_id)?;

    let new_path = template_dir.display().to_string();
    if existing.0 == new_path {
        drop(conn);
        return link_template_pack(&template_dir);
    }
    let conflicting_id = conn
        .query_row(
            "SELECT id FROM template_packs WHERE path = ?1",
            params![&new_path],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| format!("failed to inspect relink target: {err}"))?;
    if let Some(conflicting_id) = conflicting_id {
        return Err(format!(
            "template path is already linked by another record: {conflicting_id}"
        ));
    }
    drop(conn);

    let candidate = link_template_pack(&template_dir)?;
    if candidate.id == id {
        return Ok(candidate);
    }

    let mut conn = open_storage()?;
    let transaction = conn
        .transaction()
        .map_err(|err| format!("failed to start template relink transaction: {err}"))?;
    transaction
        .execute("DELETE FROM template_packs WHERE id = ?1", params![id])
        .map_err(|err| format!("failed to replace old template record: {err}"))?;
    transaction
        .execute(
            "UPDATE template_packs SET id = ?1, linked_at = ?2 WHERE id = ?3",
            params![id, &existing.2, &candidate.id],
        )
        .map_err(|err| format!("failed to preserve template record identity: {err}"))?;
    transaction
        .execute(
            "UPDATE render_history SET template_dir = ?1 WHERE template_dir = ?2",
            params![&new_path, &existing.0],
        )
        .map_err(|err| format!("failed to update template paths in render history: {err}"))?;
    transaction
        .execute(
            "UPDATE workflow_tasks SET template_path = ?1 WHERE template_path = ?2",
            params![&new_path, &existing.0],
        )
        .map_err(|err| format!("failed to update template paths in workflow tasks: {err}"))?;
    transaction
        .commit()
        .map_err(|err| format!("failed to commit template relink: {err}"))?;

    load_template_pack(&conn, id)
}

fn ensure_relink_identity(expected: &str, actual: &str) -> Result<(), String> {
    if expected == actual {
        return Ok(());
    }
    Err(format!(
        "selected template does not match the linked template: expected templateId '{expected}', found '{actual}'"
    ))
}

pub fn forget_template_pack(id: &str) -> Result<TemplatePackRemoval, String> {
    let conn = open_storage()?;
    let removed = conn
        .execute("DELETE FROM template_packs WHERE id = ?1", params![id])
        .map_err(|err| format!("failed to forget template pack: {err}"))?
        > 0;
    Ok(TemplatePackRemoval { removed })
}

pub fn record_render_history(
    job: &crate::render_job::RenderJob,
    result: &crate::renderer::PptxRenderResult,
) -> Result<RenderHistoryRecord, String> {
    let conn = open_storage()?;
    let id = uuid::Uuid::new_v4().to_string();
    let rendered_at = Utc::now().to_rfc3339();
    let result_data = result.sidecar.get("data").cloned().unwrap_or_default();
    let content_check = result_data.get("contentCheck").cloned().unwrap_or_default();
    let cleanup = result_data.get("cleanup").cloned().unwrap_or_default();
    let warnings = result_data
        .get("warnings")
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let visible_slide_count = cleanup
        .get("visibleSlideCount")
        .and_then(serde_json::Value::as_u64)
        .map(|value| value as usize);
    let content_check_status = content_check
        .get("status")
        .and_then(serde_json::Value::as_str)
        .map(ToString::to_string);
    let checked_bindings = content_check
        .get("checkedBindings")
        .and_then(serde_json::Value::as_u64)
        .map(|value| value as usize);
    let missing_bindings = content_check
        .get("missing")
        .and_then(serde_json::Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let warnings_json = serde_json::to_string(&warnings)
        .map_err(|err| format!("failed to serialize render warnings: {err}"))?;
    let job_json = serde_json::to_string(job)
        .map_err(|err| format!("failed to serialize render history job: {err}"))?;
    let result_json = serde_json::to_string(result)
        .map_err(|err| format!("failed to serialize render history result: {err}"))?;

    conn.execute(
        "INSERT INTO render_history (
            id, job_id, created_at, rendered_at, format, deck_recipe, template_dir,
            content_file, markdown_file, output_file, job_file, sidecar_file,
            planned_page_count, visible_slide_count, content_check_status,
            checked_bindings, missing_bindings, warnings_json, job_json, result_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
        params![
            &id,
            &job.job_id,
            &job.created_at,
            &rendered_at,
            &job.format,
            job.deck_recipe.as_deref(),
            &job.template_dir,
            &job.content_file,
            job.markdown_file.as_deref(),
            &job.output_file,
            &result.job_file,
            &result.sidecar_file,
            job.planned_pages.len() as i64,
            visible_slide_count.map(|value| value as i64),
            content_check_status.as_deref(),
            checked_bindings.map(|value| value as i64),
            missing_bindings as i64,
            &warnings_json,
            &job_json,
            &result_json,
        ],
    )
    .map_err(|err| format!("failed to save render history: {err}"))?;

    load_render_history_record(&conn, &id)
}

pub fn record_accepted_render_history(
    format: &str,
    recipe: Option<&str>,
    template_dir: &Path,
    content_file: &Path,
    output_file: &Path,
    planned_page_count: Option<usize>,
    acceptance: &crate::content_ir::AcceptanceSummary,
    render_result: &serde_json::Value,
) -> Result<RenderHistoryRecord, String> {
    let conn = open_storage()?;
    let id = uuid::Uuid::new_v4().to_string();
    let job_id = format!("job_{}", uuid::Uuid::new_v4());
    let created_at = Utc::now().to_rfc3339();
    let rendered_at = created_at.clone();
    let renderer_result = render_result.get("renderer").unwrap_or(render_result);
    let warnings = renderer_result
        .get("warnings")
        .or_else(|| renderer_result.get("templateWarnings"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(ToString::to_string)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let visible_item_count = renderer_result
        .get("sheetCount")
        .or_else(|| renderer_result.get("plannedPageCount"))
        .and_then(serde_json::Value::as_u64)
        .map(|value| value as usize);
    let content_check_status = match acceptance.status {
        crate::content_ir::AcceptanceStatus::Pass => "passed",
        crate::content_ir::AcceptanceStatus::Warn => "passed_with_warnings",
        crate::content_ir::AcceptanceStatus::Fail => "failed",
    };
    let warnings_json = serde_json::to_string(&warnings)
        .map_err(|err| format!("failed to serialize render warnings: {err}"))?;
    let job_json = serde_json::to_string(&serde_json::json!({
        "jobId": job_id,
        "createdAt": created_at,
        "format": format,
        "recipe": recipe,
        "templateDir": absolutize_history_path(template_dir),
        "contentFile": absolutize_history_path(content_file),
        "outputFile": absolutize_history_path(output_file),
    }))
    .map_err(|err| format!("failed to serialize render history job: {err}"))?;
    let result_json = serde_json::to_string(render_result)
        .map_err(|err| format!("failed to serialize render history result: {err}"))?;
    let template_dir = absolutize_history_path(template_dir);
    let content_file = absolutize_history_path(content_file);
    let output_file = absolutize_history_path(output_file);

    conn.execute(
        "INSERT INTO render_history (
            id, job_id, created_at, rendered_at, format, deck_recipe, template_dir,
            content_file, markdown_file, output_file, job_file, sidecar_file,
            planned_page_count, visible_slide_count, content_check_status,
            checked_bindings, missing_bindings, warnings_json, job_json, result_json
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, '', '', ?10, ?11, ?12, NULL, ?13, ?14, ?15, ?16)",
        params![
            &id,
            &job_id,
            &created_at,
            &rendered_at,
            format,
            recipe,
            &template_dir,
            &content_file,
            &output_file,
            planned_page_count.unwrap_or(0) as i64,
            visible_item_count.map(|value| value as i64),
            content_check_status,
            acceptance.schema_error_count as i64,
            &warnings_json,
            &job_json,
            &result_json,
        ],
    )
    .map_err(|err| format!("failed to save render history: {err}"))?;

    load_render_history_record(&conn, &id)
}

pub fn list_render_history(limit: usize) -> Result<Vec<RenderHistoryRecord>, String> {
    let conn = open_storage()?;
    let limit = limit.clamp(1, 200);
    let mut statement = conn
        .prepare(
            "SELECT id, job_id, created_at, rendered_at, format, deck_recipe, template_dir,
                    content_file, markdown_file, output_file, job_file, sidecar_file,
                    planned_page_count, visible_slide_count, content_check_status,
                    checked_bindings, missing_bindings, warnings_json
             FROM render_history
             ORDER BY rendered_at DESC
             LIMIT ?1",
        )
        .map_err(|err| format!("failed to prepare render history query: {err}"))?;
    let rows = statement
        .query_map(params![limit as i64], row_to_render_history_record)
        .map_err(|err| format!("failed to query render history: {err}"))?;

    let mut records = Vec::new();
    for row in rows {
        records.push(row.map_err(|err| format!("failed to read render history row: {err}"))?);
    }
    Ok(records)
}

pub fn get_render_history(id: &str) -> Result<RenderHistoryRecord, String> {
    let conn = open_storage()?;
    load_render_history_record(&conn, id)
}

pub fn rerender_history(
    id: &str,
) -> Result<crate::accepted_render::AcceptedOfficeRenderResult, String> {
    let record = get_render_history(id)?;
    crate::accepted_render::render_accepted_office(
        &PathBuf::from(record.content_file),
        &PathBuf::from(record.template_dir),
        record.deck_recipe.as_deref(),
        &PathBuf::from(record.output_file),
    )
}

pub fn list_workflow_tasks(limit: usize) -> Result<Vec<WorkflowTaskRecord>, String> {
    let conn = open_storage()?;
    list_workflow_tasks_with_conn(&conn, limit)
}

pub fn save_workflow_task(input: &WorkflowTaskInput) -> Result<WorkflowTaskRecord, String> {
    let conn = open_storage()?;
    save_workflow_task_with_conn(&conn, input)
}

pub fn delete_workflow_task(id: &str) -> Result<WorkflowTaskRemoval, String> {
    let conn = open_storage()?;
    let removed = conn
        .execute("DELETE FROM workflow_tasks WHERE id = ?1", params![id])
        .map_err(|err| format!("failed to delete workflow task: {err}"))?
        > 0;
    Ok(WorkflowTaskRemoval { removed })
}

fn open_storage() -> Result<Connection, String> {
    let path = default_storage_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create app storage directory: {err}"))?;
    }
    let conn = Connection::open(&path)
        .map_err(|err| format!("failed to open app storage '{}': {err}", path.display()))?;
    ensure_schema(&conn)?;
    Ok(conn)
}

fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS template_packs (
            id TEXT PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            template_id TEXT NOT NULL,
            name TEXT NOT NULL,
            format TEXT NOT NULL,
            entry_path TEXT NOT NULL,
            page_template_count INTEGER NOT NULL,
            deck_recipe_count INTEGER NOT NULL,
            block_template_count INTEGER NOT NULL DEFAULT 0,
            document_recipe_count INTEGER NOT NULL DEFAULT 0,
            sheet_template_count INTEGER NOT NULL DEFAULT 0,
            workbook_recipe_count INTEGER NOT NULL DEFAULT 0,
            page_template_ids_json TEXT NOT NULL DEFAULT '[]',
            deck_recipe_ids_json TEXT NOT NULL DEFAULT '[]',
            block_template_ids_json TEXT NOT NULL DEFAULT '[]',
            document_recipe_ids_json TEXT NOT NULL DEFAULT '[]',
            sheet_template_ids_json TEXT NOT NULL DEFAULT '[]',
            workbook_recipe_ids_json TEXT NOT NULL DEFAULT '[]',
            legacy_layout_count INTEGER NOT NULL,
            input_formats_json TEXT NOT NULL DEFAULT '[]',
            input_profile TEXT,
            input_profile_name TEXT,
            input_schema TEXT,
            input_schema_id TEXT,
            input_builtin_schema INTEGER NOT NULL DEFAULT 0,
            md_profile TEXT,
            health_status TEXT NOT NULL DEFAULT 'unchecked',
            health_checked_json TEXT NOT NULL DEFAULT '[]',
            health_warnings_json TEXT NOT NULL DEFAULT '[]',
            template_type TEXT NOT NULL DEFAULT 'declarative',
            renderer_type TEXT,
            renderer_entry TEXT,
            pipeline_step_count INTEGER NOT NULL DEFAULT 0,
            warnings_json TEXT NOT NULL,
            linked_at TEXT NOT NULL,
            last_validated_at TEXT NOT NULL
        );",
    )
    .map_err(|err| format!("failed to initialize template pack storage: {err}"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS render_history (
            id TEXT PRIMARY KEY,
            job_id TEXT NOT NULL,
            created_at TEXT NOT NULL,
            rendered_at TEXT NOT NULL,
            format TEXT NOT NULL,
            deck_recipe TEXT,
            template_dir TEXT NOT NULL,
            content_file TEXT NOT NULL,
            markdown_file TEXT,
            output_file TEXT NOT NULL,
            job_file TEXT NOT NULL,
            sidecar_file TEXT NOT NULL,
            planned_page_count INTEGER NOT NULL,
            visible_slide_count INTEGER,
            content_check_status TEXT,
            checked_bindings INTEGER,
            missing_bindings INTEGER NOT NULL,
            warnings_json TEXT NOT NULL,
            job_json TEXT NOT NULL,
            result_json TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_render_history_rendered_at
            ON render_history(rendered_at DESC);",
    )
    .map_err(|err| format!("failed to initialize render history storage: {err}"))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS workflow_tasks (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            status TEXT NOT NULL,
            stage TEXT NOT NULL,
            source TEXT NOT NULL,
            template_path TEXT NOT NULL,
            content_path TEXT NOT NULL,
            output_path TEXT,
            format TEXT,
            recipe TEXT,
            brief_path TEXT,
            prompt_path TEXT,
            validation_status TEXT,
            last_error TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_workflow_tasks_updated_at
            ON workflow_tasks(updated_at DESC);",
    )
    .map_err(|err| format!("failed to initialize workflow task storage: {err}"))?;
    ensure_column(
        conn,
        "template_packs",
        "page_template_ids_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "deck_recipe_ids_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "block_template_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "document_recipe_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "sheet_template_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "workbook_recipe_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "block_template_ids_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "document_recipe_ids_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "sheet_template_ids_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "workbook_recipe_ids_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "input_formats_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_column(conn, "template_packs", "input_profile", "TEXT")?;
    ensure_column(conn, "template_packs", "input_profile_name", "TEXT")?;
    ensure_column(conn, "template_packs", "input_schema", "TEXT")?;
    ensure_column(conn, "template_packs", "input_schema_id", "TEXT")?;
    ensure_column(
        conn,
        "template_packs",
        "input_builtin_schema",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(conn, "template_packs", "md_profile", "TEXT")?;
    ensure_column(
        conn,
        "template_packs",
        "template_type",
        "TEXT NOT NULL DEFAULT 'declarative'",
    )?;
    ensure_column(conn, "template_packs", "renderer_type", "TEXT")?;
    ensure_column(conn, "template_packs", "renderer_entry", "TEXT")?;
    ensure_column(
        conn,
        "template_packs",
        "pipeline_step_count",
        "INTEGER NOT NULL DEFAULT 0",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "health_status",
        "TEXT NOT NULL DEFAULT 'unchecked'",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "health_checked_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    ensure_column(
        conn,
        "template_packs",
        "health_warnings_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )
}

fn list_workflow_tasks_with_conn(
    conn: &Connection,
    limit: usize,
) -> Result<Vec<WorkflowTaskRecord>, String> {
    let limit = limit.clamp(1, 200);
    let mut statement = conn
        .prepare(
            "SELECT id, name, status, stage, source, template_path, content_path,
                    output_path, format, recipe, brief_path, prompt_path,
                    validation_status, last_error, created_at, updated_at
             FROM workflow_tasks
             ORDER BY updated_at DESC
             LIMIT ?1",
        )
        .map_err(|err| format!("failed to prepare workflow task list query: {err}"))?;
    let rows = statement
        .query_map(params![limit as i64], row_to_workflow_task)
        .map_err(|err| format!("failed to query workflow tasks: {err}"))?;
    let mut records = Vec::new();
    for row in rows {
        records.push(row.map_err(|err| format!("failed to read workflow task: {err}"))?);
    }
    Ok(records)
}

fn save_workflow_task_with_conn(
    conn: &Connection,
    input: &WorkflowTaskInput,
) -> Result<WorkflowTaskRecord, String> {
    let id = input
        .id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let existing_created_at = conn
        .query_row(
            "SELECT created_at FROM workflow_tasks WHERE id = ?1",
            params![&id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .map_err(|err| format!("failed to inspect workflow task: {err}"))?;
    let now = Utc::now().to_rfc3339();
    let created_at = existing_created_at.unwrap_or_else(|| now.clone());
    let template_path = input.template_path.trim();
    let content_path = input.content_path.trim();
    let output_path = trimmed_option(input.output_path.as_deref());
    let format = trimmed_option(input.format.as_deref());
    let recipe = trimmed_option(input.recipe.as_deref());
    let brief_path = trimmed_option(input.brief_path.as_deref());
    let prompt_path = trimmed_option(input.prompt_path.as_deref());
    let validation_status = trimmed_option(input.validation_status.as_deref());
    let last_error = trimmed_option(input.last_error.as_deref());
    let name = if input.name.trim().is_empty() {
        derive_workflow_task_name(output_path, content_path, template_path)
    } else {
        input.name.trim().to_string()
    };
    let status = normalize_task_status(&input.status);
    let stage = normalize_task_stage(&input.stage);
    let source = if input.source.trim().is_empty() {
        "app".to_string()
    } else {
        input.source.trim().to_string()
    };

    conn.execute(
        "INSERT INTO workflow_tasks (
            id, name, status, stage, source, template_path, content_path,
            output_path, format, recipe, brief_path, prompt_path,
            validation_status, last_error, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
        ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            status = excluded.status,
            stage = excluded.stage,
            source = excluded.source,
            template_path = excluded.template_path,
            content_path = excluded.content_path,
            output_path = excluded.output_path,
            format = excluded.format,
            recipe = excluded.recipe,
            brief_path = excluded.brief_path,
            prompt_path = excluded.prompt_path,
            validation_status = excluded.validation_status,
            last_error = excluded.last_error,
            updated_at = excluded.updated_at",
        params![
            &id,
            &name,
            &status,
            &stage,
            &source,
            template_path,
            content_path,
            output_path,
            format,
            recipe,
            brief_path,
            prompt_path,
            validation_status,
            last_error,
            &created_at,
            &now,
        ],
    )
    .map_err(|err| format!("failed to save workflow task: {err}"))?;
    load_workflow_task(conn, &id)
}

fn load_workflow_task(conn: &Connection, id: &str) -> Result<WorkflowTaskRecord, String> {
    conn.query_row(
        "SELECT id, name, status, stage, source, template_path, content_path,
                output_path, format, recipe, brief_path, prompt_path,
                validation_status, last_error, created_at, updated_at
         FROM workflow_tasks WHERE id = ?1",
        params![id],
        row_to_workflow_task,
    )
    .map_err(|err| format!("failed to load workflow task: {err}"))
}

fn row_to_workflow_task(row: &Row<'_>) -> rusqlite::Result<WorkflowTaskRecord> {
    let template_path: String = row.get(5)?;
    let content_path: String = row.get(6)?;
    let output_path: Option<String> = row.get(7)?;
    Ok(WorkflowTaskRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        status: row.get(2)?,
        stage: row.get(3)?,
        source: row.get(4)?,
        template_available: !template_path.is_empty() && Path::new(&template_path).is_dir(),
        content_available: !content_path.is_empty() && Path::new(&content_path).exists(),
        output_available: output_path
            .as_deref()
            .is_some_and(|path| Path::new(path).exists()),
        template_path,
        content_path,
        output_path,
        format: row.get(8)?,
        recipe: row.get(9)?,
        brief_path: row.get(10)?,
        prompt_path: row.get(11)?,
        validation_status: row.get(12)?,
        last_error: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}

fn trimmed_option(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|value| !value.is_empty())
}

fn normalize_task_status(value: &str) -> String {
    match value.trim() {
        "draft" | "content_ready" | "validated" | "rendered" | "error" => value.trim().to_string(),
        _ => "draft".to_string(),
    }
}

fn normalize_task_stage(value: &str) -> String {
    match value.trim() {
        "handoff" | "validation" | "render" => value.trim().to_string(),
        _ => "render".to_string(),
    }
}

fn derive_workflow_task_name(
    output_path: Option<&str>,
    content_path: &str,
    template_path: &str,
) -> String {
    [output_path.unwrap_or(""), content_path, template_path]
        .into_iter()
        .find_map(|value| {
            let path = Path::new(value);
            path.file_stem()
                .or_else(|| path.file_name())
                .and_then(|name| name.to_str())
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "未命名任务".to_string())
}

fn load_render_history_record(conn: &Connection, id: &str) -> Result<RenderHistoryRecord, String> {
    conn.query_row(
        "SELECT id, job_id, created_at, rendered_at, format, deck_recipe, template_dir,
                content_file, markdown_file, output_file, job_file, sidecar_file,
                planned_page_count, visible_slide_count, content_check_status,
                checked_bindings, missing_bindings, warnings_json
         FROM render_history
         WHERE id = ?1",
        params![id],
        row_to_render_history_record,
    )
    .map_err(|err| format!("failed to load render history record: {err}"))
}

fn load_template_pack(conn: &Connection, id: &str) -> Result<TemplatePackRecord, String> {
    conn.query_row(
        "SELECT id, path, template_id, name, format, entry_path, page_template_count,
                deck_recipe_count, block_template_count, document_recipe_count,
                sheet_template_count, workbook_recipe_count, page_template_ids_json,
                deck_recipe_ids_json, block_template_ids_json, document_recipe_ids_json,
                sheet_template_ids_json, workbook_recipe_ids_json, legacy_layout_count,
                input_formats_json, input_profile, input_profile_name, input_schema,
                input_schema_id, input_builtin_schema, md_profile, health_status,
                health_checked_json, health_warnings_json,
                warnings_json, linked_at, last_validated_at, template_type, renderer_type,
                renderer_entry, pipeline_step_count
         FROM template_packs
         WHERE id = ?1",
        params![id],
        row_to_template_pack,
    )
    .map_err(|err| format!("failed to load saved template pack: {err}"))
}

fn row_to_template_pack(row: &Row<'_>) -> rusqlite::Result<TemplatePackRecord> {
    let path: String = row.get(1)?;
    let format: String = row.get(4)?;
    let page_template_ids_json: String = row.get(12)?;
    let deck_recipe_ids_json: String = row.get(13)?;
    let block_template_ids_json: String = row.get(14)?;
    let document_recipe_ids_json: String = row.get(15)?;
    let sheet_template_ids_json: String = row.get(16)?;
    let workbook_recipe_ids_json: String = row.get(17)?;
    let input_formats_json: String = row.get(19)?;
    let health_checked_json: String = row.get(27)?;
    let health_warnings_json: String = row.get(28)?;
    let warnings_json: String = row.get(29)?;
    let page_template_ids =
        serde_json::from_str::<Vec<String>>(&page_template_ids_json).unwrap_or_default();
    let deck_recipe_ids =
        serde_json::from_str::<Vec<String>>(&deck_recipe_ids_json).unwrap_or_default();
    let block_template_ids =
        serde_json::from_str::<Vec<String>>(&block_template_ids_json).unwrap_or_default();
    let document_recipe_ids =
        serde_json::from_str::<Vec<String>>(&document_recipe_ids_json).unwrap_or_default();
    let sheet_template_ids =
        serde_json::from_str::<Vec<String>>(&sheet_template_ids_json).unwrap_or_default();
    let workbook_recipe_ids =
        serde_json::from_str::<Vec<String>>(&workbook_recipe_ids_json).unwrap_or_default();
    let mut input_formats =
        serde_json::from_str::<Vec<String>>(&input_formats_json).unwrap_or_default();
    if input_formats.is_empty() {
        input_formats.push("json".to_string());
    }
    let health_checked =
        serde_json::from_str::<Vec<String>>(&health_checked_json).unwrap_or_default();
    let health_warnings =
        serde_json::from_str::<Vec<String>>(&health_warnings_json).unwrap_or_default();
    let warnings = serde_json::from_str::<Vec<String>>(&warnings_json).unwrap_or_default();
    let preview = crate::template_manifest::template_preview_for_dir(Path::new(&path));
    let (family_id, role) = template_family_metadata(Path::new(&path), &format);
    Ok(TemplatePackRecord {
        id: row.get(0)?,
        path_available: Path::new(&path).is_dir(),
        path,
        template_id: row.get(2)?,
        name: row.get(3)?,
        family_id,
        role,
        format,
        entry_path: row.get(5)?,
        template_type: row
            .get::<_, Option<String>>(32)?
            .unwrap_or_else(|| "declarative".to_string()),
        renderer_type: row.get(33)?,
        renderer_entry: row.get(34)?,
        pipeline_step_count: row.get::<_, Option<i64>>(35)?.unwrap_or_default() as usize,
        page_template_count: row.get::<_, i64>(6)? as usize,
        deck_recipe_count: row.get::<_, i64>(7)? as usize,
        block_template_count: row.get::<_, i64>(8)? as usize,
        document_recipe_count: row.get::<_, i64>(9)? as usize,
        sheet_template_count: row.get::<_, i64>(10)? as usize,
        workbook_recipe_count: row.get::<_, i64>(11)? as usize,
        page_template_ids,
        deck_recipe_ids,
        block_template_ids,
        document_recipe_ids,
        sheet_template_ids,
        workbook_recipe_ids,
        legacy_layout_count: row.get::<_, i64>(18)? as usize,
        input_formats,
        input_profile: row.get(20)?,
        input_profile_name: row.get(21)?,
        input_schema: row.get(22)?,
        input_schema_id: row.get(23)?,
        input_builtin_schema: row.get::<_, i64>(24)? != 0,
        md_profile: row.get(25)?,
        preview_cover: preview.cover,
        preview_slides: preview.slides,
        health_status: row.get(26)?,
        health_checked,
        health_warnings,
        warnings,
        linked_at: row.get(30)?,
        last_validated_at: row.get(31)?,
    })
}

fn template_family_metadata(path: &Path, format: &str) -> (Option<String>, String) {
    match crate::template_manifest::load_manifest(path) {
        Ok(manifest) => (
            manifest
                .family_id
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string),
            crate::template_manifest::normalized_template_role(&manifest),
        ),
        Err(_) => (None, fallback_role_for_format(format).to_string()),
    }
}

fn fallback_role_for_format(format: &str) -> &'static str {
    match format {
        "docx" => "document",
        "xlsx" => "workbook",
        _ => "slides",
    }
}

fn row_to_render_history_record(row: &Row<'_>) -> rusqlite::Result<RenderHistoryRecord> {
    let warnings_json: String = row.get(17)?;
    let warnings = serde_json::from_str::<Vec<String>>(&warnings_json).unwrap_or_default();
    Ok(RenderHistoryRecord {
        id: row.get(0)?,
        job_id: row.get(1)?,
        created_at: row.get(2)?,
        rendered_at: row.get(3)?,
        format: row.get(4)?,
        deck_recipe: row.get(5)?,
        template_dir: row.get(6)?,
        content_file: row.get(7)?,
        markdown_file: row.get(8)?,
        output_file: normalize_stored_path(row.get(9)?),
        job_file: normalize_stored_path(row.get(10)?),
        sidecar_file: row.get(11)?,
        planned_page_count: row.get::<_, i64>(12)? as usize,
        visible_slide_count: row.get::<_, Option<i64>>(13)?.map(|value| value as usize),
        content_check_status: row.get(14)?,
        checked_bindings: row.get::<_, Option<i64>>(15)?.map(|value| value as usize),
        missing_bindings: row.get::<_, i64>(16)? as usize,
        warnings,
    })
}

fn normalize_stored_path(path: String) -> String {
    if path.trim().is_empty() {
        return path;
    }
    if Path::new(&path).is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
            .display()
            .to_string()
    }
}

fn absolutize_history_path(path: &Path) -> String {
    if path.is_absolute() {
        return path.display().to_string();
    }
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
        .display()
        .to_string()
}

fn ensure_column(
    conn: &Connection,
    table_name: &str,
    column_name: &str,
    column_definition: &str,
) -> Result<(), String> {
    let mut statement = conn
        .prepare(&format!("PRAGMA table_info({table_name})"))
        .map_err(|err| format!("failed to inspect table '{table_name}': {err}"))?;
    let rows = statement
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|err| format!("failed to inspect columns for '{table_name}': {err}"))?;
    let mut columns = BTreeSet::new();
    for row in rows {
        columns.insert(row.map_err(|err| format!("failed to read table column: {err}"))?);
    }
    if columns.contains(column_name) {
        return Ok(());
    }
    conn.execute(
        &format!("ALTER TABLE {table_name} ADD COLUMN {column_name} {column_definition}"),
        [],
    )
    .map_err(|err| format!("failed to migrate template pack storage: {err}"))?;
    Ok(())
}

fn normalize_template_dir(template_dir: &Path) -> Result<PathBuf, String> {
    let absolute = if template_dir.is_absolute() {
        template_dir.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|err| format!("failed to resolve current directory: {err}"))?
            .join(template_dir)
    };
    crate::validator::require_dir(&absolute, "template pack")?;
    absolute
        .canonicalize()
        .map_err(|err| format!("failed to normalize template pack path: {err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_paths_follow_each_desktop_platform() {
        assert_eq!(
            test_storage_path("macos", &[("HOME", "/Users/example")]),
            PathBuf::from(
                "/Users/example/Library/Application Support/rDeckForge/rdeckforge.sqlite3"
            )
        );
        assert_eq!(
            test_storage_path(
                "windows",
                &[("LOCALAPPDATA", r"C:\Users\Example\AppData\Local")]
            ),
            PathBuf::from(r"C:\Users\Example\AppData\Local")
                .join("rDeckForge")
                .join("rdeckforge.sqlite3")
        );
        assert_eq!(
            test_storage_path("linux", &[("XDG_DATA_HOME", "/home/example/.data")]),
            PathBuf::from("/home/example/.data/rDeckForge/rdeckforge.sqlite3")
        );
    }

    #[test]
    fn storage_path_supports_override_and_windows_fallback() {
        assert_eq!(
            test_storage_path("macos", &[("RDECKFORGE_DATA_DIR", "/data/rdeckforge")]),
            PathBuf::from("/data/rdeckforge/rdeckforge.sqlite3")
        );
        assert_eq!(
            test_storage_path("windows", &[("USERPROFILE", r"C:\Users\Example")]),
            PathBuf::from(r"C:\Users\Example")
                .join("AppData")
                .join("Local")
                .join("rDeckForge")
                .join("rdeckforge.sqlite3")
        );
    }

    #[test]
    fn relink_requires_the_same_template_identity() {
        assert!(ensure_relink_identity("teaching-v1", "teaching-v1").is_ok());
        assert_eq!(
            ensure_relink_identity("teaching-v1", "another-template").unwrap_err(),
            "selected template does not match the linked template: expected templateId 'teaching-v1', found 'another-template'"
        );
    }

    #[test]
    fn workflow_tasks_round_trip_and_preserve_created_at() {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        let input = WorkflowTaskInput {
            id: Some("task-1".to_string()),
            name: "护理教学".to_string(),
            status: "content_ready".to_string(),
            stage: "validation".to_string(),
            source: "prompt-ai-output".to_string(),
            template_path: "/tmp/template".to_string(),
            content_path: "/tmp/content.json".to_string(),
            output_path: Some("/tmp/output.pptx".to_string()),
            format: Some("pptx".to_string()),
            recipe: Some("teaching_deck".to_string()),
            brief_path: Some("/tmp/brief.md".to_string()),
            prompt_path: Some("/tmp/prompt.md".to_string()),
            validation_status: None,
            last_error: None,
        };
        let first = save_workflow_task_with_conn(&conn, &input).unwrap();
        let mut updated = input.clone();
        updated.status = "validated".to_string();
        updated.validation_status = Some("pass".to_string());
        let second = save_workflow_task_with_conn(&conn, &updated).unwrap();
        assert_eq!(first.created_at, second.created_at);
        assert_eq!(second.status, "validated");
        assert_eq!(second.validation_status.as_deref(), Some("pass"));
        assert_eq!(list_workflow_tasks_with_conn(&conn, 10).unwrap().len(), 1);
    }

    fn test_storage_path(platform: &str, values: &[(&str, &str)]) -> PathBuf {
        storage_path_for(platform, |key| {
            values
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| OsString::from(value))
        })
    }
}
