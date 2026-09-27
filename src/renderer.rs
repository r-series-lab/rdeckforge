use serde::Serialize;
use serde_json::Value;
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    sync::OnceLock,
};

static PPTX_RENDERER_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DryRunRenderResult {
    pub renderer_status: &'static str,
    pub job_file: String,
    pub message: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PptxRenderResult {
    pub renderer_status: &'static str,
    pub job_file: String,
    pub sidecar_file: String,
    pub output_file: String,
    pub sidecar: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_integrity: Option<crate::office_package::OfficePackageIntegrity>,
}

pub fn write_dry_run_job(
    job: &crate::render_job::RenderJob,
    output_file: &Path,
) -> Result<DryRunRenderResult, String> {
    let job_file = write_job_file(job, output_file)?;
    Ok(DryRunRenderResult {
        renderer_status: "dry_run_only",
        job_file: job_file.display().to_string(),
        message: "PPTX sidecar is scaffolded but not wired yet.",
    })
}

pub fn render_pptx_job(
    job: &crate::render_job::RenderJob,
    output_file: &Path,
) -> Result<PptxRenderResult, String> {
    let job_file = write_job_file(job, output_file)?;
    let sidecar_file = resolve_pptx_sidecar()?;
    let mut command = pptx_sidecar_command(&sidecar_file);
    let output = command
        .arg("--job")
        .arg(&job_file)
        .output()
        .map_err(|err| format!("failed to run PPTX renderer sidecar: {err}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !output.status.success() {
        return Err(format!(
            "PPTX renderer sidecar failed with status {}. stdout: {} stderr: {}",
            output.status, stdout, stderr
        ));
    }

    let sidecar: Value = serde_json::from_str(&stdout).map_err(|err| {
        format!("PPTX renderer sidecar returned invalid JSON: {err}; stdout: {stdout}")
    })?;
    if sidecar.get("ok").and_then(Value::as_bool) != Some(true) {
        return Err(format!("PPTX renderer sidecar reported failure: {sidecar}"));
    }

    Ok(PptxRenderResult {
        renderer_status: "rendered",
        job_file: job_file.display().to_string(),
        sidecar_file: sidecar_file.display().to_string(),
        output_file: output_file.display().to_string(),
        sidecar,
        output_integrity: None,
    })
}

pub fn render_pptx_job_atomic(
    job: &crate::render_job::RenderJob,
    output_file: &Path,
) -> Result<PptxRenderResult, String> {
    let mut transaction = crate::office_package::AtomicOfficeOutput::new(output_file, "pptx")?;
    let mut staged_job = job.clone();
    staged_job.output_file = transaction.staging_path().display().to_string();
    let mut result = render_pptx_job(&staged_job, transaction.staging_path())?;
    let integrity = transaction.validate()?;
    transaction.commit()?;
    let final_job_file = write_job_file(job, transaction.final_path())?;
    replace_output_path(
        &mut result.sidecar,
        &transaction.staging_path().display().to_string(),
        &transaction.final_path().display().to_string(),
    );
    result.job_file = final_job_file.display().to_string();
    result.output_file = transaction.final_path().display().to_string();
    result.output_integrity = Some(integrity);
    Ok(result)
}

fn replace_output_path(value: &mut Value, staged: &str, final_output: &str) {
    match value {
        Value::String(text) => {
            if text.contains(staged) {
                *text = text.replace(staged, final_output);
            }
        }
        Value::Array(values) => {
            for value in values {
                replace_output_path(value, staged, final_output);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                replace_output_path(value, staged, final_output);
            }
        }
        _ => {}
    }
}

fn write_job_file(
    job: &crate::render_job::RenderJob,
    output_file: &Path,
) -> Result<PathBuf, String> {
    let job_file = output_file.with_extension("pptx.render-job.json");
    if let Some(parent) = job_file.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create render job directory: {err}"))?;
    }
    let raw = serde_json::to_string_pretty(job)
        .map_err(|err| format!("failed to serialize render job: {err}"))?;
    fs::write(&job_file, raw).map_err(|err| format!("failed to write render job: {err}"))?;
    Ok(job_file)
}

pub fn write_render_job_metadata(
    job: &crate::render_job::RenderJob,
    output_file: &Path,
) -> Result<PathBuf, String> {
    write_job_file(job, output_file)
}

pub fn pptx_sidecar_candidate() -> PathBuf {
    if let Ok(path) = env::var("RDECKFORGE_PPTX_RENDERER") {
        return PathBuf::from(path);
    }

    if let Some(path) = PPTX_RENDERER_OVERRIDE.get() {
        return path.clone();
    }

    if let Some(path) = bundled_pptx_sidecar_candidate().filter(|path| path.is_file()) {
        return path;
    }

    env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("renderers")
        .join("pptx-node")
        .join("dist")
        .join("index.js")
}

fn bundled_pptx_sidecar_candidate() -> Option<PathBuf> {
    let executable = env::current_exe().ok()?;
    let executable = fs::canonicalize(&executable).unwrap_or(executable);
    Some(executable.parent()?.join(pptx_sidecar_name()))
}

fn pptx_sidecar_name() -> &'static str {
    if cfg!(windows) {
        "rdeckforge-pptx.exe"
    } else {
        "rdeckforge-pptx"
    }
}

pub fn set_pptx_renderer_override(path: PathBuf) -> Result<(), String> {
    if let Some(existing) = PPTX_RENDERER_OVERRIDE.get() {
        if existing == &path {
            return Ok(());
        }
        return Err(format!(
            "PPTX renderer override is already set to {}",
            existing.display()
        ));
    }
    PPTX_RENDERER_OVERRIDE
        .set(path)
        .map_err(|path| format!("failed to set PPTX renderer override: {}", path.display()))
}

pub fn pptx_renderer_requires_node(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension, "js" | "cjs" | "mjs"))
}

fn pptx_sidecar_command(path: &Path) -> Command {
    if pptx_renderer_requires_node(path) {
        let mut command = Command::new("node");
        command.arg(path);
        command
    } else {
        Command::new(path)
    }
}

pub fn resolve_pptx_sidecar() -> Result<PathBuf, String> {
    let sidecar = pptx_sidecar_candidate();
    if sidecar.is_file() {
        return Ok(sidecar);
    }
    if env::var("RDECKFORGE_PPTX_RENDERER").is_ok() {
        return Err(format!(
            "RDECKFORGE_PPTX_RENDERER points to a missing file: {}",
            sidecar.display()
        ));
    }
    Err(format!(
        "PPTX renderer sidecar is not built: {}. Run `npm --prefix ./renderers/pptx-node run build` first.",
        sidecar.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn javascript_renderers_require_node() {
        assert!(pptx_renderer_requires_node(Path::new("renderer.js")));
        assert!(pptx_renderer_requires_node(Path::new("renderer.cjs")));
        assert!(pptx_renderer_requires_node(Path::new("renderer.mjs")));
        assert!(!pptx_renderer_requires_node(Path::new("rdeckforge-pptx")));
        assert!(!pptx_renderer_requires_node(Path::new(
            "rdeckforge-pptx.exe"
        )));
    }

    #[test]
    fn bundled_sidecar_uses_platform_executable_name() {
        let expected = if cfg!(windows) {
            "rdeckforge-pptx.exe"
        } else {
            "rdeckforge-pptx"
        };
        assert_eq!(pptx_sidecar_name(), expected);
    }
}
