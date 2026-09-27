use serde::Serialize;
use serde_json::json;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptRenderResult {
    pub renderer_status: &'static str,
    pub format: String,
    pub input_format: String,
    pub output_file: String,
    pub input_file: String,
    pub template_dir: String,
    pub template_id: String,
    pub renderer_type: String,
    pub renderer_entry: String,
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub pipeline: Vec<ScriptStepResult>,
    pub template_warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_integrity: Option<crate::office_package::OfficePackageIntegrity>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptStepResult {
    pub id: String,
    pub step_type: String,
    pub entry: String,
    pub command: Vec<String>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptTemplateAdapter {
    pub pack_dir: String,
    pub manifest_file: String,
    pub script_file: String,
    pub script_copied: bool,
    pub validation: crate::template_manifest::TemplateValidation,
}

#[derive(Debug, Clone, Copy)]
enum InvocationKind {
    Renderer,
    Pipeline,
}

struct InvocationSpec {
    id: String,
    invocation_type: String,
    runtime: Option<String>,
    entry: String,
    args: Vec<String>,
    kind: InvocationKind,
}

#[derive(Debug)]
pub struct ScriptTemplateAdapterOptions<'a> {
    pub script: &'a Path,
    pub out_dir: &'a Path,
    pub template_id: &'a str,
    pub name: &'a str,
    pub format: &'a str,
    pub input_format: &'a str,
    pub renderer_type: Option<&'a str>,
    pub runtime: Option<&'a str>,
    pub copy_script: bool,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeCommandSpec {
    pub command: String,
    pub prefix_args: Vec<String>,
}

pub fn create_script_template_adapter(
    options: ScriptTemplateAdapterOptions<'_>,
) -> Result<ScriptTemplateAdapter, String> {
    crate::validator::require_file(options.script, "script renderer")?;
    let format = options.format.trim();
    if !matches!(format, "pptx" | "docx" | "xlsx") {
        return Err(format!(
            "script adapter format '{format}' is not supported; expected pptx, docx, or xlsx"
        ));
    }
    let input_format = crate::content_source::normalize_format(options.input_format);
    if !matches!(input_format.as_str(), "json" | "md") {
        return Err(format!(
            "script adapter input format '{input_format}' is not supported; expected json or md"
        ));
    }
    let template_id = options.template_id.trim();
    if template_id.is_empty() {
        return Err("script adapter id is required".to_string());
    }
    let name = options.name.trim();
    if name.is_empty() {
        return Err("script adapter name is required".to_string());
    }

    fs::create_dir_all(options.out_dir)
        .map_err(|err| format!("failed to create adapter directory: {err}"))?;
    let manifest_file = options.out_dir.join("template.manifest.json");
    if manifest_file.exists() && !options.force {
        return Err(format!(
            "script adapter manifest already exists: {}; pass --force to overwrite",
            manifest_file.display()
        ));
    }

    let source_script = absolutize(options.script);
    let renderer_type = normalized_renderer_type(options.renderer_type, options.script)?;
    let runtime = runtime_command_for(&renderer_type, options.runtime)?;
    let (script_file, manifest_entry, script_copied) = if options.copy_script {
        let renderer_dir = options.out_dir.join("renderer");
        fs::create_dir_all(&renderer_dir)
            .map_err(|err| format!("failed to create renderer directory: {err}"))?;
        let file_name = options
            .script
            .file_name()
            .ok_or_else(|| "script renderer file name is required".to_string())?;
        let destination = renderer_dir.join(file_name);
        let destination_absolute = absolutize(&destination);
        if source_script != destination_absolute {
            if destination.exists() && !options.force {
                return Err(format!(
                    "script renderer already exists: {}; pass --force to overwrite",
                    destination.display()
                ));
            }
            fs::copy(&source_script, &destination).map_err(|err| {
                format!(
                    "failed to copy script renderer '{}' to '{}': {err}",
                    source_script.display(),
                    destination.display()
                )
            })?;
        }
        (
            destination_absolute,
            PathBuf::from("renderer")
                .join(file_name)
                .display()
                .to_string(),
            true,
        )
    } else {
        (
            source_script.clone(),
            source_script.display().to_string(),
            false,
        )
    };
    let manifest = json!({
        "schemaVersion": "1.0",
        "templateId": template_id,
        "name": name,
        "format": format,
        "templateType": "script",
        "input": {
            "formats": [input_format]
        },
        "renderer": {
            "type": renderer_type,
            "runtime": runtime.command,
            "entry": manifest_entry,
            "args": ["{input}", "--output", "{output}"]
        }
    });
    let raw = serde_json::to_string_pretty(&manifest)
        .map_err(|err| format!("failed to serialize script adapter manifest: {err}"))?;
    fs::write(&manifest_file, raw)
        .map_err(|err| format!("failed to write script adapter manifest: {err}"))?;
    let validation = crate::template_manifest::validate_template_pack(options.out_dir)?;

    Ok(ScriptTemplateAdapter {
        pack_dir: absolutize(options.out_dir).display().to_string(),
        manifest_file: absolutize(&manifest_file).display().to_string(),
        script_file: script_file.display().to_string(),
        script_copied,
        validation,
    })
}

pub fn render_script_template(
    input: &Path,
    template_dir: &Path,
    out: &Path,
) -> Result<ScriptRenderResult, String> {
    crate::validator::require_dir(template_dir, "script template pack")?;
    let validation = crate::template_manifest::validate_template_pack(template_dir)?;
    if validation.template_type != "script" && validation.template_type != "hybrid" {
        return Err(format!(
            "script renderer only accepts templateType 'script' or 'hybrid'; got '{}'",
            validation.template_type
        ));
    }

    let manifest = crate::template_manifest::load_manifest(template_dir)?;
    let input_spec = crate::template_manifest::normalized_input_spec(&manifest);
    let source = crate::content_source::load_content_source(input)?;
    if !input_spec.formats.contains(&source.input_format) {
        return Err(format!(
            "content input format '{}' is not accepted by template '{}'; expected one of: {}",
            source.input_format,
            manifest.template_id,
            input_spec.formats.join(", ")
        ));
    }
    let profiled_source =
        crate::content_source::apply_md_profile(source.clone(), input_spec.md_profile.as_deref())?;
    crate::schema_validation::enforce_input_schema(
        template_dir,
        &input_spec,
        &profiled_source.value,
    )?;

    let output_path = absolutize(out);
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create output directory '{}': {err}",
                parent.display()
            )
        })?;
    }

    let renderer = manifest
        .renderer
        .as_ref()
        .ok_or_else(|| "script template renderer is missing".to_string())?;
    let renderer_type = renderer
        .renderer_type
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "renderer.type is required for script templates".to_string())?
        .to_string();
    let renderer_entry = renderer
        .entry
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "renderer.entry is required for script templates".to_string())?
        .to_string();
    let input_path = source
        .source_path
        .as_deref()
        .map(absolutize)
        .unwrap_or_else(|| absolutize(input));
    let template_path = absolutize(template_dir);

    let renderer_spec = InvocationSpec {
        id: "renderer".to_string(),
        invocation_type: renderer_type.clone(),
        runtime: renderer.runtime.clone(),
        entry: renderer_entry.clone(),
        args: renderer.args.clone(),
        kind: InvocationKind::Renderer,
    };
    let renderer_result =
        run_invocation(&renderer_spec, &template_path, &input_path, &output_path)?;
    crate::validator::require_file(&output_path, "script renderer output")?;

    let pipeline = run_pipeline_steps_with_paths(
        &template_path,
        &manifest.pipeline,
        &input_path,
        &output_path,
    )?;

    Ok(ScriptRenderResult {
        renderer_status: "rendered",
        format: manifest.format,
        input_format: source.input_format,
        output_file: output_path.display().to_string(),
        input_file: input_path.display().to_string(),
        template_dir: template_path.display().to_string(),
        template_id: manifest.template_id,
        renderer_type,
        renderer_entry,
        command: renderer_result.command,
        exit_code: renderer_result.exit_code,
        stdout: renderer_result.stdout,
        stderr: renderer_result.stderr,
        pipeline,
        template_warnings: validation.warnings,
        output_integrity: None,
    })
}

pub fn run_pipeline_steps(
    template_dir: &Path,
    steps: &[crate::template_manifest::TemplatePipelineStep],
    input: &Path,
    out: &Path,
) -> Result<Vec<ScriptStepResult>, String> {
    if steps.is_empty() {
        return Ok(Vec::new());
    }
    let template_path = absolutize(template_dir);
    let input_path = crate::content_source::resolve_content_source_path(input)
        .map(|path| absolutize(&path))
        .unwrap_or_else(|_| absolutize(input));
    let output_path = absolutize(out);
    run_pipeline_steps_with_paths(&template_path, steps, &input_path, &output_path)
}

fn run_pipeline_steps_with_paths(
    template_dir: &Path,
    steps: &[crate::template_manifest::TemplatePipelineStep],
    input: &Path,
    out: &Path,
) -> Result<Vec<ScriptStepResult>, String> {
    crate::validator::require_file(out, "pipeline output")?;
    let mut pipeline = Vec::new();
    for (index, step) in steps.iter().enumerate() {
        let step_id = step
            .id
            .clone()
            .unwrap_or_else(|| format!("step_{}", index + 1));
        let step_type = step
            .step_type
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("pipeline.{step_id}.type is required"))?
            .to_string();
        let entry = step
            .entry
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| format!("pipeline.{step_id}.entry is required"))?
            .to_string();
        let step_spec = InvocationSpec {
            id: step_id,
            invocation_type: step_type,
            runtime: step.runtime.clone(),
            entry,
            args: step.args.clone(),
            kind: InvocationKind::Pipeline,
        };
        pipeline.push(run_invocation(&step_spec, template_dir, input, out)?);
    }
    Ok(pipeline)
}

fn run_invocation(
    spec: &InvocationSpec,
    template_dir: &Path,
    input: &Path,
    output: &Path,
) -> Result<ScriptStepResult, String> {
    let runtime = runtime_command_for(&spec.invocation_type, spec.runtime.as_deref())?;
    let entry_path = template_dir.join(&spec.entry);
    crate::validator::require_file(&entry_path, &format!("{}.entry", spec.id))?;
    let args = invocation_args(spec, input, output, template_dir);
    let command_preview = std::iter::once(runtime.command.clone())
        .chain(runtime.prefix_args.iter().cloned())
        .chain(std::iter::once(entry_path.display().to_string()))
        .chain(args.iter().cloned())
        .collect::<Vec<_>>();

    let output_result = Command::new(&runtime.command)
        .args(&runtime.prefix_args)
        .arg(&entry_path)
        .args(&args)
        .current_dir(template_dir)
        .output()
        .map_err(|err| {
            format!(
                "failed to run {} script '{}': {err}",
                spec.id,
                entry_path.display()
            )
        })?;
    let stdout = String::from_utf8_lossy(&output_result.stdout)
        .trim_end()
        .to_string();
    let stderr = String::from_utf8_lossy(&output_result.stderr)
        .trim_end()
        .to_string();
    if !output_result.status.success() {
        return Err(format!(
            "{} script failed with status {:?}: {}{}{}",
            spec.id,
            output_result.status.code(),
            stderr,
            if stderr.is_empty() || stdout.is_empty() {
                ""
            } else {
                "\n"
            },
            stdout
        ));
    }

    Ok(ScriptStepResult {
        id: spec.id.clone(),
        step_type: spec.invocation_type.clone(),
        entry: spec.entry.clone(),
        command: command_preview,
        exit_code: output_result.status.code(),
        stdout,
        stderr,
    })
}

pub fn runtime_command_for(
    invocation_type: &str,
    runtime: Option<&str>,
) -> Result<RuntimeCommandSpec, String> {
    let runtime = runtime.filter(|value| !value.trim().is_empty());
    let command = match invocation_type {
        "script.python" | "python" => runtime.unwrap_or("python3"),
        "script.node" | "node" | "javascript" => runtime.unwrap_or("node"),
        "script.shell" | "shell" | "sh" => runtime.unwrap_or(if cfg!(target_os = "windows") {
            "powershell.exe"
        } else {
            "sh"
        }),
        "script.powershell" | "powershell" | "pwsh" => {
            runtime.unwrap_or(if cfg!(target_os = "windows") {
                "powershell.exe"
            } else {
                "pwsh"
            })
        }
        "script.command" | "command" => runtime.ok_or_else(|| {
            "script.command requires renderer.runtime or pipeline.runtime".to_string()
        })?,
        other => {
            return Err(format!(
                "unsupported script type '{other}'; expected script.python, script.node, script.shell, script.powershell, or script.command"
            ));
        }
    };
    let command_name = Path::new(command)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or(command)
        .to_ascii_lowercase();
    let prefix_args = if matches!(invocation_type, "script.powershell" | "powershell" | "pwsh")
        || (matches!(invocation_type, "script.shell" | "shell" | "sh")
            && (command_name.starts_with("powershell") || command_name == "pwsh"))
    {
        let mut args = vec!["-NoProfile".to_string()];
        if command_name.starts_with("powershell") {
            args.extend(["-ExecutionPolicy".to_string(), "Bypass".to_string()]);
        }
        args.push("-File".to_string());
        args
    } else {
        Vec::new()
    };
    Ok(RuntimeCommandSpec {
        command: command.to_string(),
        prefix_args,
    })
}

fn normalized_renderer_type(explicit: Option<&str>, script: &Path) -> Result<String, String> {
    if let Some(explicit) = explicit.filter(|value| !value.trim().is_empty()) {
        runtime_command_for(explicit, Some("runtime-probe"))?;
        return Ok(explicit.to_string());
    }
    match script
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "py" => Ok("script.python".to_string()),
        "js" | "mjs" | "cjs" => Ok("script.node".to_string()),
        "sh" | "bash" | "zsh" => Ok("script.shell".to_string()),
        "ps1" => Ok("script.powershell".to_string()),
        extension => Err(format!(
            "cannot infer renderer type from script extension '{extension}'; pass renderer_type explicitly"
        )),
    }
}

fn invocation_args(
    spec: &InvocationSpec,
    input: &Path,
    output: &Path,
    template_dir: &Path,
) -> Vec<String> {
    let raw_args = if spec.args.is_empty() {
        match spec.kind {
            InvocationKind::Renderer => vec![
                "{input}".to_string(),
                "--output".to_string(),
                "{output}".to_string(),
            ],
            InvocationKind::Pipeline => vec!["{output}".to_string()],
        }
    } else {
        spec.args.clone()
    };
    raw_args
        .into_iter()
        .map(|arg| expand_tokens(&arg, input, output, template_dir))
        .collect()
}

fn expand_tokens(raw: &str, input: &Path, output: &Path, template_dir: &Path) -> String {
    let output_dir = output
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let output_stem = output
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    let output_base_name = output
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("output");
    raw.replace("{input}", &input.display().to_string())
        .replace("{output}", &output.display().to_string())
        .replace("{templateDir}", &template_dir.display().to_string())
        .replace("{outputDir}", &output_dir.display().to_string())
        .replace("{outputStem}", output_stem)
        .replace("{outputBaseName}", output_base_name)
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
    use std::fs;

    #[test]
    fn runtime_command_specs_cover_supported_script_types() {
        assert_eq!(
            runtime_command_for("script.python", None).unwrap(),
            RuntimeCommandSpec {
                command: "python3".to_string(),
                prefix_args: Vec::new(),
            }
        );
        assert_eq!(
            runtime_command_for("script.node", None).unwrap(),
            RuntimeCommandSpec {
                command: "node".to_string(),
                prefix_args: Vec::new(),
            }
        );
        assert!(runtime_command_for("script.command", None).is_err());
        assert_eq!(
            runtime_command_for("script.command", Some("my-runtime"))
                .unwrap()
                .command,
            "my-runtime"
        );
    }

    #[test]
    fn render_script_template_runs_node_renderer_when_available() -> Result<(), String> {
        if Command::new("node").arg("--version").output().is_err() {
            return Ok(());
        }
        let root =
            std::env::temp_dir().join(format!("rdeckforge-node-render-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("scripts"))
            .map_err(|err| format!("failed to create scripts: {err}"))?;
        fs::write(
            root.join("scripts/build.mjs"),
            r#"import fs from "node:fs";
const input = process.argv[2];
const output = process.argv[4];
fs.writeFileSync(output, "node:" + fs.readFileSync(input, "utf8"));
"#,
        )
        .map_err(|err| format!("failed to write renderer: {err}"))?;
        fs::write(
            root.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "node-render-test",
  "name": "Node Render Test",
  "format": "pptx",
  "templateType": "script",
  "input": { "formats": ["md"] },
  "renderer": {
    "type": "script.node",
    "entry": "scripts/build.mjs"
  }
}"#,
        )
        .map_err(|err| format!("failed to write manifest: {err}"))?;
        let input = root.join("content.md");
        let out = root.join("out.pptx");
        fs::write(&input, "# Node").map_err(|err| format!("failed to write input: {err}"))?;

        let result = render_script_template(&input, &root, &out)?;

        assert_eq!(result.renderer_type, "script.node");
        assert_eq!(
            fs::read_to_string(&out).map_err(|err| format!("failed to read output: {err}"))?,
            "node:# Node"
        );
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn render_script_template_runs_shell_renderer() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-shell-render-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(root.join("scripts"))
            .map_err(|err| format!("failed to create scripts: {err}"))?;
        fs::write(
            root.join("scripts/build.sh"),
            "#!/bin/sh\nprintf 'shell:' > \"$3\"\ncat \"$1\" >> \"$3\"\n",
        )
        .map_err(|err| format!("failed to write renderer: {err}"))?;
        fs::write(
            root.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "shell-render-test",
  "name": "Shell Render Test",
  "format": "pptx",
  "templateType": "script",
  "input": { "formats": ["md"] },
  "renderer": {
    "type": "script.shell",
    "entry": "scripts/build.sh"
  }
}"#,
        )
        .map_err(|err| format!("failed to write manifest: {err}"))?;
        let input = root.join("content.md");
        let out = root.join("out.pptx");
        fs::write(&input, "# Shell").map_err(|err| format!("failed to write input: {err}"))?;

        let result = render_script_template(&input, &root, &out)?;

        assert_eq!(result.renderer_type, "script.shell");
        assert_eq!(
            fs::read_to_string(&out).map_err(|err| format!("failed to read output: {err}"))?,
            "shell:# Shell"
        );
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn render_script_template_runs_python_renderer() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-script-render-{}", uuid::Uuid::new_v4()));
        let scripts = root.join("scripts");
        fs::create_dir_all(&scripts).map_err(|err| format!("failed to create scripts: {err}"))?;
        fs::write(
            scripts.join("build.py"),
            r#"import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("input")
parser.add_argument("--output", required=True)
args = parser.parse_args()
Path(args.output).write_text(Path(args.input).read_text(encoding="utf-8"), encoding="utf-8")
print(args.output)
"#,
        )
        .map_err(|err| format!("failed to write renderer: {err}"))?;
        fs::write(
            root.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "script-render-test",
  "name": "Script Render Test",
  "format": "pptx",
  "templateType": "script",
  "input": { "formats": ["md"], "schemaId": "chapter_markdown_v1" },
  "renderer": {
    "type": "script.python",
    "runtime": "python3",
    "entry": "scripts/build.py",
    "args": ["{input}", "--output", "{output}"]
  }
}"#,
        )
        .map_err(|err| format!("failed to write manifest: {err}"))?;
        let input = root.join("content.md");
        let out = root.join("out.pptx");
        fs::write(&input, "# Test\n\nBody")
            .map_err(|err| format!("failed to write input: {err}"))?;

        let result = render_script_template(&input, &root, &out)?;

        assert_eq!(result.renderer_type, "script.python");
        assert_eq!(result.input_format, "md");
        assert!(out.is_file());
        assert!(result.stdout.ends_with("out.pptx"));
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn render_script_template_accepts_hybrid_renderer() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-hybrid-render-{}", uuid::Uuid::new_v4()));
        let scripts = root.join("scripts");
        fs::create_dir_all(&scripts).map_err(|err| format!("failed to create scripts: {err}"))?;
        fs::write(
            scripts.join("build.py"),
            r#"import argparse
from pathlib import Path

parser = argparse.ArgumentParser()
parser.add_argument("input")
parser.add_argument("--output", required=True)
args = parser.parse_args()
Path(args.output).write_text("hybrid:" + Path(args.input).read_text(encoding="utf-8"), encoding="utf-8")
"#,
        )
        .map_err(|err| format!("failed to write renderer: {err}"))?;
        fs::write(
            root.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "hybrid-render-test",
  "name": "Hybrid Render Test",
  "format": "pptx",
  "templateType": "hybrid",
  "input": { "formats": ["md"], "schemaId": "chapter_markdown_v1" },
  "renderer": {
    "type": "script.python",
    "runtime": "python3",
    "entry": "scripts/build.py"
  }
}"#,
        )
        .map_err(|err| format!("failed to write manifest: {err}"))?;
        let input = root.join("content.md");
        let out = root.join("out.pptx");
        fs::write(&input, "# Test").map_err(|err| format!("failed to write input: {err}"))?;

        let result = render_script_template(&input, &root, &out)?;

        assert_eq!(result.template_id, "hybrid-render-test");
        assert_eq!(
            fs::read_to_string(&out).map_err(|err| format!("failed to read output: {err}"))?,
            "hybrid:# Test"
        );
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn run_pipeline_steps_uses_output_as_default_argument() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-pipeline-{}", uuid::Uuid::new_v4()));
        let scripts = root.join("scripts");
        fs::create_dir_all(&scripts).map_err(|err| format!("failed to create scripts: {err}"))?;
        fs::write(
            scripts.join("check.py"),
            r#"import sys
from pathlib import Path

target = Path(sys.argv[1])
print("checked:" + target.name)
"#,
        )
        .map_err(|err| format!("failed to write pipeline script: {err}"))?;
        let input = root.join("content.md");
        let out = root.join("out.pptx");
        fs::write(&input, "# Test").map_err(|err| format!("failed to write input: {err}"))?;
        fs::write(&out, "fake pptx").map_err(|err| format!("failed to write output: {err}"))?;
        let steps = vec![crate::template_manifest::TemplatePipelineStep {
            id: Some("check_output".to_string()),
            step_type: Some("script.python".to_string()),
            runtime: Some("python3".to_string()),
            entry: Some("scripts/check.py".to_string()),
            args: Vec::new(),
        }];

        let result = run_pipeline_steps(&root, &steps, &input, &out)?;

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].id, "check_output");
        assert_eq!(result[0].stdout, "checked:out.pptx");
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn create_script_template_adapter_writes_manifest() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-script-adapter-{}",
            uuid::Uuid::new_v4()
        ));
        let script = root.join("external_build.py");
        let out_dir = root.join("adapter");
        fs::create_dir_all(&root).map_err(|err| format!("failed to create temp root: {err}"))?;
        fs::write(
            &script,
            r#"from pathlib import Path
Path("ok.txt").write_text("ok", encoding="utf-8")
"#,
        )
        .map_err(|err| format!("failed to write external script: {err}"))?;

        let result = create_script_template_adapter(ScriptTemplateAdapterOptions {
            script: &script,
            out_dir: &out_dir,
            template_id: "external-script-test",
            name: "External Script Test",
            format: "pptx",
            input_format: "md",
            renderer_type: None,
            runtime: Some("python3"),
            copy_script: true,
            force: false,
        })?;

        assert_eq!(result.validation.template_id, "external-script-test");
        assert_eq!(result.validation.template_type, "script");
        let script_label = script.to_string_lossy().to_string();
        assert_eq!(
            result.validation.renderer_entry.as_deref(),
            Some("renderer/external_build.py")
        );
        assert!(result.script_copied);
        assert_ne!(result.script_file, script_label);
        assert!(out_dir.join("template.manifest.json").is_file());
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn create_script_template_adapter_infers_node_renderer() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-node-adapter-{}", uuid::Uuid::new_v4()));
        let script = root.join("build.mjs");
        let out_dir = root.join("adapter");
        fs::create_dir_all(&root).map_err(|err| format!("failed to create temp root: {err}"))?;
        fs::write(&script, "console.log('ok');\n")
            .map_err(|err| format!("failed to write external script: {err}"))?;

        let result = create_script_template_adapter(ScriptTemplateAdapterOptions {
            script: &script,
            out_dir: &out_dir,
            template_id: "external-node-test",
            name: "External Node Test",
            format: "pptx",
            input_format: "json",
            renderer_type: None,
            runtime: None,
            copy_script: true,
            force: false,
        })?;
        let manifest = crate::template_manifest::load_manifest(&out_dir)?;

        assert_eq!(
            result.validation.renderer_type.as_deref(),
            Some("script.node")
        );
        assert_eq!(
            manifest.renderer.and_then(|renderer| renderer.runtime),
            Some("node".to_string())
        );
        fs::remove_dir_all(&root).ok();
        Ok(())
    }
}
