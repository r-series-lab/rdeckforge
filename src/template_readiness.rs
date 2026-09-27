use chrono::Utc;
use serde::Serialize;
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use crate::environment::DiagnosticStatus;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateDependencyCheck {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub status: DiagnosticStatus,
    pub required: bool,
    pub detected: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub message: String,
    pub fix_hint: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateReadinessReport {
    pub template_id: String,
    pub template_name: String,
    pub template_dir: String,
    pub format: String,
    pub template_type: String,
    pub checked_at: String,
    pub readiness: DiagnosticStatus,
    pub can_render: bool,
    pub summary: String,
    pub components: Vec<TemplateDependencyCheck>,
    pub warnings: Vec<String>,
}

pub fn diagnose_template_readiness(template_dir: &Path) -> Result<TemplateReadinessReport, String> {
    let template_dir = normalize_template_dir(template_dir)?;
    let validation = crate::template_manifest::validate_template_pack(&template_dir)?;
    let manifest = crate::template_manifest::load_manifest(&template_dir)?;
    let mut components = Vec::new();

    components.push(pass_check(
        "manifest",
        "模板清单",
        "manifest",
        true,
        Some(template_dir.join("template.manifest.json")),
        "template.manifest.json 已解析，结构协议可用。",
    ));

    let uses_script_renderer = validation.template_type == "script"
        || (validation.template_type == "hybrid" && manifest.renderer.is_some());
    if !uses_script_renderer {
        components.push(office_entry_check(&validation));
    }
    components.push(engine_check(
        &template_dir,
        &validation.format,
        uses_script_renderer,
    ));

    if let Some(renderer) = manifest.renderer.as_ref() {
        components.extend(invocation_checks(
            &template_dir,
            "renderer",
            "脚本渲染器",
            renderer.renderer_type.as_deref(),
            renderer.runtime.as_deref(),
            renderer.entry.as_deref(),
        ));
    } else if validation.template_type == "script" {
        components.push(fail_check(
            "renderer",
            "脚本渲染器",
            "renderer",
            true,
            None,
            "脚本型模板没有声明 renderer。",
            "在 template.manifest.json 中声明 renderer.type、runtime 和 entry。",
        ));
    }

    for (index, step) in manifest.pipeline.iter().enumerate() {
        let step_id = step
            .id
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(ToString::to_string)
            .unwrap_or_else(|| format!("step_{}", index + 1));
        components.extend(invocation_checks(
            &template_dir,
            &format!("pipeline.{step_id}"),
            &format!("流水线 {step_id}"),
            step.step_type.as_deref(),
            step.runtime.as_deref(),
            step.entry.as_deref(),
        ));
    }

    components.extend(declared_dependency_checks(
        &template_dir,
        &manifest.dependencies,
    ));

    components.push(health_check(&validation));
    if !validation.warnings.is_empty() {
        components.push(TemplateDependencyCheck {
            id: "template_warnings".to_string(),
            label: "模板提示".to_string(),
            kind: "validation".to_string(),
            status: DiagnosticStatus::Warn,
            required: false,
            detected: true,
            path: None,
            version: None,
            message: format!("模板校验返回 {} 条提示。", validation.warnings.len()),
            fix_hint: Some("检查 warnings，确认这些提示不会影响当前生成场景。".to_string()),
        });
    }

    let readiness = readiness_for(&components);
    let can_render = !components
        .iter()
        .any(|component| component.required && component.status == DiagnosticStatus::Fail);
    let summary = readiness_summary(readiness, can_render, &components);

    Ok(TemplateReadinessReport {
        template_id: validation.template_id,
        template_name: validation.name,
        template_dir: template_dir.display().to_string(),
        format: validation.format,
        template_type: validation.template_type,
        checked_at: Utc::now().to_rfc3339(),
        readiness,
        can_render,
        summary,
        components,
        warnings: validation.warnings,
    })
}

fn office_entry_check(
    validation: &crate::template_manifest::TemplateValidation,
) -> TemplateDependencyCheck {
    let path = PathBuf::from(&validation.entry_path);
    if !validation.entry_path.is_empty() && path.is_file() {
        pass_check(
            "office_entry",
            "Office 模板源文件",
            "file",
            true,
            Some(path),
            "模板源文件存在，可交给内置声明式引擎处理。",
        )
    } else {
        fail_check(
            "office_entry",
            "Office 模板源文件",
            "file",
            true,
            (!validation.entry_path.is_empty()).then_some(path),
            "没有找到模板声明的 Office 源文件。",
            "检查 manifest.entry，并确认 PPTX/DOCX/XLSX 文件仍位于模板包内。",
        )
    }
}

fn engine_check(
    template_dir: &Path,
    format: &str,
    uses_script_renderer: bool,
) -> TemplateDependencyCheck {
    if uses_script_renderer {
        return pass_check(
            "render_engine",
            "渲染调度",
            "engine",
            true,
            None,
            "模板将使用脚本渲染器；运行时和脚本入口会单独检查。",
        );
    }
    match format {
        "docx" | "xlsx" => pass_check(
            "render_engine",
            "内置 Office 引擎",
            "engine",
            true,
            None,
            &format!("内置 {} 声明式渲染器可用。", format.to_uppercase()),
        ),
        "pptx" => {
            let candidate = crate::renderer::pptx_sidecar_candidate();
            if !candidate.is_file() {
                return fail_check(
                    "render_engine",
                    "PPTX 声明式渲染器",
                    "engine",
                    true,
                    Some(candidate),
                    "没有找到 PPTX sidecar。",
                    "正式版应重新安装应用；源码环境运行 npm --prefix renderers/pptx-node run build。",
                );
            }
            if crate::renderer::pptx_renderer_requires_node(&candidate) {
                if let Some(node) = find_command("node", template_dir) {
                    pass_check(
                        "render_engine",
                        "PPTX 声明式渲染器",
                        "engine",
                        true,
                        Some(candidate),
                        &format!(
                            "PPTX JavaScript sidecar 与 Node.js 可用：{}",
                            node.display()
                        ),
                    )
                } else {
                    fail_check(
                        "render_engine",
                        "PPTX 声明式渲染器",
                        "engine",
                        true,
                        Some(candidate),
                        "当前 PPTX sidecar 需要 Node.js，但 PATH 中没有找到 node。",
                        "安装 Node.js，或使用包含独立 PPTX 运行时的正式安装包。",
                    )
                }
            } else {
                pass_check(
                    "render_engine",
                    "PPTX 声明式渲染器",
                    "engine",
                    true,
                    Some(candidate),
                    "PPTX sidecar 已包含独立运行时。",
                )
            }
        }
        other => fail_check(
            "render_engine",
            "渲染引擎",
            "engine",
            true,
            None,
            &format!("模板格式 {other} 没有可用的内置引擎。"),
            "将模板格式改为 pptx、docx 或 xlsx。",
        ),
    }
}

fn invocation_checks(
    template_dir: &Path,
    id: &str,
    label: &str,
    invocation_type: Option<&str>,
    runtime: Option<&str>,
    entry: Option<&str>,
) -> Vec<TemplateDependencyCheck> {
    let mut checks = Vec::new();
    let normalized_type = invocation_type.unwrap_or("").trim();
    let runtime_spec = crate::script_render::runtime_command_for(normalized_type, runtime);
    checks.push(if runtime_spec.is_ok() {
        pass_check(
            &format!("{id}.type"),
            &format!("{label}类型"),
            "script_type",
            true,
            None,
            "脚本类型受当前引擎支持。",
        )
    } else {
        fail_check(
            &format!("{id}.type"),
            &format!("{label}类型"),
            "script_type",
            true,
            None,
            if normalized_type.is_empty() {
                "没有声明脚本类型。"
            } else {
                "当前引擎不支持该脚本类型。"
            },
            "支持的类型为 script.python、script.node、script.shell、script.powershell 和 script.command。",
        )
    });

    let entry_path = entry
        .filter(|value| !value.trim().is_empty())
        .map(|value| resolve_template_path(template_dir, value));
    checks.push(match entry_path {
        Some(path) if path.is_file() => pass_check(
            &format!("{id}.entry"),
            &format!("{label}入口"),
            "file",
            true,
            Some(path),
            "脚本入口文件存在。",
        ),
        path => fail_check(
            &format!("{id}.entry"),
            &format!("{label}入口"),
            "file",
            true,
            path,
            "没有找到脚本入口文件。",
            "检查 entry 路径；便携模板包应把脚本放在模板包目录内。",
        ),
    });

    checks.push(runtime_check(
        template_dir,
        id,
        label,
        normalized_type,
        runtime,
    ));
    checks
}

fn runtime_check(
    template_dir: &Path,
    id: &str,
    label: &str,
    invocation_type: &str,
    runtime: Option<&str>,
) -> TemplateDependencyCheck {
    let runtime = match crate::script_render::runtime_command_for(invocation_type, runtime) {
        Ok(runtime) => runtime,
        Err(error) => {
            return fail_check(
                &format!("{id}.runtime"),
                &format!("{label}运行时"),
                "runtime",
                true,
                None,
                &error,
                "修改脚本类型或为 script.command 显式声明 runtime。",
            );
        }
    };
    let runtime_name = runtime.command.as_str();
    let Some(path) = find_command(runtime_name, template_dir) else {
        return fail_check(
            &format!("{id}.runtime"),
            &format!("{label}运行时"),
            "runtime",
            true,
            Some(PathBuf::from(runtime_name)),
            &format!("没有找到运行时 {runtime_name}。"),
            "安装模板声明的运行时，或修改 runtime 指向可用的本地程序。",
        );
    };
    let version = command_version(&path, template_dir);
    TemplateDependencyCheck {
        id: format!("{id}.runtime"),
        label: format!("{label}运行时"),
        kind: "runtime".to_string(),
        status: DiagnosticStatus::Pass,
        required: true,
        detected: true,
        path: Some(path.display().to_string()),
        version,
        message: format!("运行时 {runtime_name} 可执行。"),
        fix_hint: None,
    }
}

fn health_check(
    validation: &crate::template_manifest::TemplateValidation,
) -> TemplateDependencyCheck {
    let status = if validation.health.status == "warning" {
        DiagnosticStatus::Warn
    } else if validation.health.status == "ok" {
        DiagnosticStatus::Pass
    } else {
        DiagnosticStatus::Info
    };
    TemplateDependencyCheck {
        id: "template_health".to_string(),
        label: "模板结构健康".to_string(),
        kind: "health".to_string(),
        status,
        required: false,
        detected: validation.health.status != "unchecked",
        path: None,
        version: None,
        message: match status {
            DiagnosticStatus::Pass => {
                "Office 模板中的页面、占位符或工作表结构检查通过。".to_string()
            }
            DiagnosticStatus::Warn => format!(
                "Office 模板结构有 {} 条提示。",
                validation.health.warnings.len()
            ),
            _ => "该模板模式没有可执行的 Office 结构检查。".to_string(),
        },
        fix_hint: (status == DiagnosticStatus::Warn)
            .then(|| "检查模板健康提示，确认绑定目标仍存在。".to_string()),
    }
}

fn declared_dependency_checks(
    template_dir: &Path,
    dependencies: &crate::template_manifest::TemplateDependencySpec,
) -> Vec<TemplateDependencyCheck> {
    let mut checks = Vec::new();
    for (index, dependency) in dependencies.files.iter().enumerate() {
        let path = template_dir.join(&dependency.path);
        let label = match dependency.kind.as_deref() {
            Some("python_requirements") => "Python 依赖清单",
            Some("node_package") => "Node.js 依赖清单",
            Some("config") => "配置文件",
            Some("resource") => "依赖资源",
            _ => "依赖文件",
        };
        if path.is_file() {
            checks.push(pass_check(
                &format!("dependency.file.{}", index + 1),
                label,
                "dependency_file",
                dependency.required,
                Some(path),
                &with_description(
                    "声明的依赖文件存在；rDeckForge 不会自动安装其中的依赖。",
                    dependency.description.as_deref(),
                ),
            ));
        } else {
            checks.push(missing_check(
                &format!("dependency.file.{}", index + 1),
                label,
                "dependency_file",
                dependency.required,
                Some(path),
                &with_description("声明的依赖文件不存在。", dependency.description.as_deref()),
                "把依赖清单或资源放回模板包，或将该项标记为可选。",
            ));
        }
    }

    for dependency in &dependencies.commands {
        let id = format!("dependency.command.{}", dependency.id);
        let label = dependency
            .description
            .as_deref()
            .unwrap_or(&dependency.id)
            .to_string();
        if !platform_applies(&dependency.platforms) {
            checks.push(skipped_platform_check(
                &id,
                &label,
                "dependency_command",
                &dependency.platforms,
            ));
            continue;
        }
        let Some(path) = find_command(&dependency.command, template_dir) else {
            checks.push(missing_check(
                &id,
                &label,
                "dependency_command",
                dependency.required,
                Some(PathBuf::from(&dependency.command)),
                &format!("没有找到命令 {}。", dependency.command),
                "安装模板声明的命令，或修改 dependencies.commands[].command。",
            ));
            continue;
        };
        let args = if dependency.args.is_empty() {
            vec!["--version".to_string()]
        } else {
            dependency.args.clone()
        };
        match Command::new(&path)
            .args(&args)
            .current_dir(template_dir)
            .output()
        {
            Ok(output) if output.status.success() => {
                checks.push(TemplateDependencyCheck {
                    id,
                    label,
                    kind: "dependency_command".to_string(),
                    status: DiagnosticStatus::Pass,
                    required: dependency.required,
                    detected: true,
                    path: Some(path.display().to_string()),
                    version: output_line(&output.stdout).or_else(|| output_line(&output.stderr)),
                    message: "依赖命令存在且检查命令执行成功。".to_string(),
                    fix_hint: None,
                });
            }
            Ok(output) => checks.push(missing_check(
                &id,
                &label,
                "dependency_command",
                dependency.required,
                Some(path),
                &format!("依赖命令存在，但检查返回 {}。", output.status),
                "检查 dependencies.commands[].args 或修复命令自身环境。",
            )),
            Err(error) => checks.push(missing_check(
                &id,
                &label,
                "dependency_command",
                dependency.required,
                Some(path),
                &format!("依赖命令无法执行：{error}"),
                "检查命令权限和当前用户环境。",
            )),
        }
    }

    for dependency in &dependencies.environment {
        let id = format!("dependency.environment.{}", dependency.name);
        let label = dependency
            .description
            .as_deref()
            .unwrap_or(&dependency.name)
            .to_string();
        if !platform_applies(&dependency.platforms) {
            checks.push(skipped_platform_check(
                &id,
                &label,
                "dependency_environment",
                &dependency.platforms,
            ));
            continue;
        }
        let value = env::var_os(&dependency.name);
        let detected = value.as_ref().is_some_and(|value| {
            dependency.allow_empty || !value.to_string_lossy().trim().is_empty()
        });
        if detected {
            checks.push(TemplateDependencyCheck {
                id,
                label,
                kind: "dependency_environment".to_string(),
                status: DiagnosticStatus::Pass,
                required: dependency.required,
                detected: true,
                path: None,
                version: None,
                message: format!(
                    "环境变量 {} 已设置；诊断不会读取或输出变量值。",
                    dependency.name
                ),
                fix_hint: None,
            });
        } else {
            checks.push(missing_check(
                &id,
                &label,
                "dependency_environment",
                dependency.required,
                None,
                &format!("环境变量 {} 未设置或为空。", dependency.name),
                "在启动 rDeckForge 的用户环境中设置该变量；不要把密钥写进模板包。",
            ));
        }
    }
    checks
}

fn missing_check(
    id: &str,
    label: &str,
    kind: &str,
    required: bool,
    path: Option<PathBuf>,
    message: &str,
    fix_hint: &str,
) -> TemplateDependencyCheck {
    TemplateDependencyCheck {
        id: id.to_string(),
        label: label.to_string(),
        kind: kind.to_string(),
        status: if required {
            DiagnosticStatus::Fail
        } else {
            DiagnosticStatus::Warn
        },
        required,
        detected: false,
        path: path.map(|value| value.display().to_string()),
        version: None,
        message: message.to_string(),
        fix_hint: Some(fix_hint.to_string()),
    }
}

fn skipped_platform_check(
    id: &str,
    label: &str,
    kind: &str,
    platforms: &[String],
) -> TemplateDependencyCheck {
    TemplateDependencyCheck {
        id: id.to_string(),
        label: label.to_string(),
        kind: kind.to_string(),
        status: DiagnosticStatus::Info,
        required: false,
        detected: false,
        path: None,
        version: None,
        message: format!("该依赖仅适用于 {}，当前平台跳过。", platforms.join(", ")),
        fix_hint: None,
    }
}

fn platform_applies(platforms: &[String]) -> bool {
    platforms.is_empty() || platforms.iter().any(|platform| platform == env::consts::OS)
}

fn with_description(message: &str, description: Option<&str>) -> String {
    match description.filter(|value| !value.trim().is_empty()) {
        Some(description) => format!("{message} {description}"),
        None => message.to_string(),
    }
}

fn output_line(bytes: &[u8]) -> Option<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn readiness_for(components: &[TemplateDependencyCheck]) -> DiagnosticStatus {
    if components
        .iter()
        .any(|component| component.required && component.status == DiagnosticStatus::Fail)
    {
        return DiagnosticStatus::Fail;
    }
    if components.iter().any(|component| {
        matches!(
            component.status,
            DiagnosticStatus::Fail | DiagnosticStatus::Warn
        )
    }) {
        return DiagnosticStatus::Warn;
    }
    DiagnosticStatus::Pass
}

fn readiness_summary(
    readiness: DiagnosticStatus,
    can_render: bool,
    components: &[TemplateDependencyCheck],
) -> String {
    let failed = components
        .iter()
        .filter(|component| component.status == DiagnosticStatus::Fail)
        .count();
    let warnings = components
        .iter()
        .filter(|component| component.status == DiagnosticStatus::Warn)
        .count();
    if !can_render {
        return format!("模板当前不可执行：{failed} 个必需依赖未就绪。");
    }
    match readiness {
        DiagnosticStatus::Warn => format!("模板可以执行，但有 {warnings} 项提示需要确认。"),
        _ => "模板文件、渲染引擎和运行时均已就绪。".to_string(),
    }
}

fn pass_check(
    id: &str,
    label: &str,
    kind: &str,
    required: bool,
    path: Option<PathBuf>,
    message: &str,
) -> TemplateDependencyCheck {
    TemplateDependencyCheck {
        id: id.to_string(),
        label: label.to_string(),
        kind: kind.to_string(),
        status: DiagnosticStatus::Pass,
        required,
        detected: true,
        path: path.map(|value| value.display().to_string()),
        version: None,
        message: message.to_string(),
        fix_hint: None,
    }
}

fn fail_check(
    id: &str,
    label: &str,
    kind: &str,
    required: bool,
    path: Option<PathBuf>,
    message: &str,
    fix_hint: &str,
) -> TemplateDependencyCheck {
    TemplateDependencyCheck {
        id: id.to_string(),
        label: label.to_string(),
        kind: kind.to_string(),
        status: DiagnosticStatus::Fail,
        required,
        detected: false,
        path: path.map(|value| value.display().to_string()),
        version: None,
        message: message.to_string(),
        fix_hint: Some(fix_hint.to_string()),
    }
}

fn resolve_template_path(template_dir: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        template_dir.join(path)
    }
}

fn find_command(command: &str, template_dir: &Path) -> Option<PathBuf> {
    let command_path = PathBuf::from(command);
    if command_path.components().count() > 1 {
        let candidate = if command_path.is_absolute() {
            command_path
        } else {
            template_dir.join(command_path)
        };
        return candidate.is_file().then_some(candidate);
    }
    let path = env::var_os("PATH")?;
    for directory in env::split_paths(&path) {
        let candidate = directory.join(command);
        if candidate.is_file() {
            return Some(candidate);
        }
        if cfg!(target_os = "windows") && !command.contains('.') {
            for extension in ["exe", "cmd", "bat"] {
                let candidate = directory.join(format!("{command}.{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn command_version(command: &Path, template_dir: &Path) -> Option<String> {
    let output = Command::new(command)
        .arg("--version")
        .current_dir(template_dir)
        .output()
        .ok()?;
    String::from_utf8_lossy(if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    })
    .lines()
    .map(str::trim)
    .find(|line| !line.is_empty())
    .map(ToString::to_string)
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
    use std::fs;

    #[test]
    fn reports_missing_script_runtime_as_blocking() {
        let root = script_template("missing-runtime", "missing-rdeckforge-runtime", "build.py");
        fs::write(root.join("build.py"), "print('ok')\n").unwrap();

        let report = diagnose_template_readiness(&root).unwrap();

        assert!(!report.can_render);
        assert_eq!(report.readiness, DiagnosticStatus::Fail);
        assert!(report.components.iter().any(|component| {
            component.id == "renderer.runtime" && component.status == DiagnosticStatus::Fail
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_missing_script_entry_as_blocking() {
        let runtime = std::env::current_exe().unwrap();
        let root = script_template(
            "missing-entry",
            runtime.to_string_lossy().as_ref(),
            "missing.py",
        );

        let report = diagnose_template_readiness(&root).unwrap();

        assert!(!report.can_render);
        assert!(report.components.iter().any(|component| {
            component.id == "renderer.entry" && component.status == DiagnosticStatus::Fail
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reports_declared_files_commands_and_environment_without_values() {
        let runtime = std::env::current_exe().unwrap();
        let root = script_template(
            "declared-dependencies",
            runtime.to_string_lossy().as_ref(),
            "build.py",
        );
        fs::write(root.join("build.py"), "print('ok')\n").unwrap();
        fs::write(root.join("requirements.txt"), "python-pptx==1.0.0\n").unwrap();
        let manifest_file = root.join("template.manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_file).unwrap()).unwrap();
        manifest["dependencies"] = serde_json::json!({
            "files": [
                { "path": "requirements.txt", "kind": "python_requirements" }
            ],
            "commands": [
                {
                    "id": "self-check",
                    "command": runtime,
                    "args": ["--list"],
                    "required": true
                },
                {
                    "id": "optional-tool",
                    "command": "missing-rdeckforge-optional-tool",
                    "required": false
                }
            ],
            "environment": [
                { "name": "RDECKFORGE_TEST_SECRET_THAT_IS_NOT_SET", "required": true }
            ]
        });
        fs::write(
            &manifest_file,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let report = diagnose_template_readiness(&root).unwrap();

        assert!(!report.can_render);
        assert!(report.components.iter().any(|component| {
            component.kind == "dependency_file" && component.status == DiagnosticStatus::Pass
        }));
        assert!(report.components.iter().any(|component| {
            component.id == "dependency.command.optional-tool"
                && component.status == DiagnosticStatus::Warn
        }));
        assert!(report.components.iter().any(|component| {
            component.id == "dependency.command.self-check"
                && component.status == DiagnosticStatus::Pass
        }));
        let environment = report
            .components
            .iter()
            .find(|component| {
                component.id == "dependency.environment.RDECKFORGE_TEST_SECRET_THAT_IS_NOT_SET"
            })
            .unwrap();
        assert_eq!(environment.status, DiagnosticStatus::Fail);
        assert!(environment.path.is_none());
        assert!(environment.version.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_dependency_files_outside_template_pack() {
        let runtime = std::env::current_exe().unwrap();
        let root = script_template(
            "unsafe-dependency",
            runtime.to_string_lossy().as_ref(),
            "build.py",
        );
        fs::write(root.join("build.py"), "print('ok')\n").unwrap();
        let manifest_file = root.join("template.manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_file).unwrap()).unwrap();
        manifest["dependencies"] = serde_json::json!({
            "files": [{ "path": "../requirements.txt" }]
        });
        fs::write(
            &manifest_file,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let error = diagnose_template_readiness(&root).unwrap_err();

        assert!(error.contains("must stay inside the template pack"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn skips_required_dependencies_for_other_platforms() {
        let runtime = std::env::current_exe().unwrap();
        let root = script_template(
            "platform-dependencies",
            runtime.to_string_lossy().as_ref(),
            "build.py",
        );
        fs::write(root.join("build.py"), "print('ok')\n").unwrap();
        let other_platform = if std::env::consts::OS == "windows" {
            "macos"
        } else {
            "windows"
        };
        let manifest_file = root.join("template.manifest.json");
        let mut manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(&manifest_file).unwrap()).unwrap();
        manifest["dependencies"] = serde_json::json!({
            "commands": [{
                "id": "other-platform-command",
                "command": "missing-command",
                "platforms": [other_platform]
            }],
            "environment": [{
                "name": "MISSING_OTHER_PLATFORM_SECRET",
                "platforms": [other_platform]
            }]
        });
        fs::write(
            &manifest_file,
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();

        let report = diagnose_template_readiness(&root).unwrap();

        assert!(report.can_render);
        assert_eq!(
            report
                .components
                .iter()
                .filter(|component| component.id.contains("other-platform")
                    || component.id.contains("OTHER_PLATFORM"))
                .filter(|component| component.status == DiagnosticStatus::Info)
                .count(),
            2
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn script_template(label: &str, runtime: &str, entry: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-readiness-{label}-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let manifest = serde_json::json!({
            "schemaVersion": "1.0",
            "templateId": format!("readiness-{label}"),
            "name": "Readiness Test",
            "format": "pptx",
            "templateType": "script",
            "input": { "formats": ["json"] },
            "renderer": {
                "type": "script.python",
                "runtime": runtime,
                "entry": entry
            }
        });
        fs::write(
            root.join("template.manifest.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .unwrap();
        root
    }
}
