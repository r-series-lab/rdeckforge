use chrono::Utc;
use serde::Serialize;
use std::{env, fs, path::PathBuf, process::Command};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticStatus {
    Pass,
    Warn,
    Fail,
    Info,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentComponent {
    pub id: &'static str,
    pub label: &'static str,
    pub status: DiagnosticStatus,
    pub required: bool,
    pub detected: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub message: String,
    pub fix_hint: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentDiagnostics {
    pub product_name: &'static str,
    pub version: &'static str,
    pub checked_at: String,
    pub platform: String,
    pub architecture: String,
    pub readiness: DiagnosticStatus,
    pub summary: String,
    pub components: Vec<EnvironmentComponent>,
    pub notes: Vec<String>,
}

pub fn environment_diagnostics() -> EnvironmentDiagnostics {
    let components = vec![
        app_core_component(),
        storage_component(),
        pptx_sidecar_component(),
        node_runtime_component(),
        command_component(
            "python3",
            "Python 3 runtime",
            "python3",
            &["--version"],
            false,
            "脚本型模板包可能需要 Python 3。",
            "安装 Python 3，或使用不依赖 Python 的声明式模板包。",
        ),
        system_opener_component(),
        office_viewer_component(),
    ];
    let readiness = readiness_for(&components);
    let summary = summary_for(readiness, &components);

    EnvironmentDiagnostics {
        product_name: "rDeckForge",
        version: env!("CARGO_PKG_VERSION"),
        checked_at: Utc::now().to_rfc3339(),
        platform: env::consts::OS.to_string(),
        architecture: env::consts::ARCH.to_string(),
        readiness,
        summary,
        components,
        notes: vec![
            "rDeckForge 不做 AI 推理；AI 输出由外部网页或 CLI 工作流生成。".to_string(),
            "Office/WPS/Keynote 只用于打开和编辑生成文件，不是内置 DOCX/XLSX 渲染的硬依赖。"
                .to_string(),
            "脚本型或混合型模板包的额外依赖由 template.manifest.json 的 renderer/pipeline 声明。"
                .to_string(),
        ],
    }
}

fn app_core_component() -> EnvironmentComponent {
    EnvironmentComponent {
        id: "app_core",
        label: "应用核心",
        status: DiagnosticStatus::Pass,
        required: true,
        detected: true,
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        path: env::current_exe()
            .ok()
            .map(|path| path.display().to_string()),
        message: "Rust 核心模块可用。".to_string(),
        fix_hint: None,
    }
}

fn storage_component() -> EnvironmentComponent {
    let storage_path = crate::storage::default_storage_path();
    let Some(parent) = storage_path.parent() else {
        return EnvironmentComponent {
            id: "local_storage",
            label: "本地存储",
            status: DiagnosticStatus::Fail,
            required: true,
            detected: false,
            version: None,
            path: Some(storage_path.display().to_string()),
            message: "无法解析本地存储目录。".to_string(),
            fix_hint: Some("检查系统用户目录权限。".to_string()),
        };
    };

    match fs::create_dir_all(parent) {
        Ok(()) => EnvironmentComponent {
            id: "local_storage",
            label: "本地存储",
            status: DiagnosticStatus::Pass,
            required: true,
            detected: true,
            version: None,
            path: Some(storage_path.display().to_string()),
            message: "模板库记录和历史记录可写入本机。".to_string(),
            fix_hint: None,
        },
        Err(error) => EnvironmentComponent {
            id: "local_storage",
            label: "本地存储",
            status: DiagnosticStatus::Fail,
            required: true,
            detected: false,
            version: None,
            path: Some(storage_path.display().to_string()),
            message: format!("无法创建本地存储目录：{error}"),
            fix_hint: Some("检查应用数据目录权限，或换到可写用户目录运行。".to_string()),
        },
    }
}

fn pptx_sidecar_component() -> EnvironmentComponent {
    let candidate = crate::renderer::pptx_sidecar_candidate();
    let detected = candidate.is_file();
    EnvironmentComponent {
        id: "pptx_sidecar",
        label: "PPTX 声明式渲染器",
        status: if detected {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Warn
        },
        required: false,
        detected,
        version: None,
        path: Some(candidate.display().to_string()),
        message: if detected {
            "PPTX sidecar 已构建，可用于声明式 PPTX 渲染。".to_string()
        } else {
            "未找到 PPTX sidecar；PPTX 声明式生成不可用，DOCX/XLSX 和脚本型模板不受此项直接影响。"
                .to_string()
        },
        fix_hint: if detected {
            None
        } else {
            Some("开发环境运行 `npm --prefix ./renderers/pptx-node run build`；发布版应把 sidecar 打进应用包。".to_string())
        },
    }
}

fn node_runtime_component() -> EnvironmentComponent {
    let renderer = crate::renderer::pptx_sidecar_candidate();
    if renderer.is_file() && !crate::renderer::pptx_renderer_requires_node(&renderer) {
        return EnvironmentComponent {
            id: "node",
            label: "Node.js runtime",
            status: DiagnosticStatus::Info,
            required: false,
            detected: false,
            version: None,
            path: None,
            message: "正式版 PPTX 渲染器已内置独立运行时，无需系统安装 Node.js。".to_string(),
            fix_hint: None,
        };
    }

    command_component(
        "node",
        "Node.js runtime",
        "node",
        &["--version"],
        false,
        "开发态 PPTX JavaScript 渲染器可用。",
        "开发或直接运行源码 CLI 时安装 Node.js；正式安装包已内置独立 PPTX sidecar。",
    )
}

fn command_component(
    id: &'static str,
    label: &'static str,
    command: &str,
    args: &[&str],
    required: bool,
    available_message: &str,
    fix_hint: &str,
) -> EnvironmentComponent {
    match Command::new(command).args(args).output() {
        Ok(output) if output.status.success() => {
            let version =
                command_output_line(&output.stdout).or_else(|| command_output_line(&output.stderr));
            EnvironmentComponent {
                id,
                label,
                status: DiagnosticStatus::Pass,
                required,
                detected: true,
                version,
                path: command_path(command),
                message: available_message.to_string(),
                fix_hint: None,
            }
        }
        Ok(output) => EnvironmentComponent {
            id,
            label,
            status: if required {
                DiagnosticStatus::Fail
            } else {
                DiagnosticStatus::Warn
            },
            required,
            detected: true,
            version: command_output_line(&output.stdout)
                .or_else(|| command_output_line(&output.stderr)),
            path: command_path(command),
            message: format!("{command} 可执行，但版本检查失败：{}", output.status),
            fix_hint: Some(fix_hint.to_string()),
        },
        Err(error) => EnvironmentComponent {
            id,
            label,
            status: if required {
                DiagnosticStatus::Fail
            } else {
                DiagnosticStatus::Warn
            },
            required,
            detected: false,
            version: None,
            path: Some(command.to_string()),
            message: format!("未检测到 {command}：{error}"),
            fix_hint: Some(fix_hint.to_string()),
        },
    }
}

fn system_opener_component() -> EnvironmentComponent {
    let command = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer.exe"
    } else {
        "xdg-open"
    };
    let path = find_command_on_path(command);
    let detected = path.is_some();
    EnvironmentComponent {
        id: "system_opener",
        label: "系统文件打开器",
        status: if detected {
            DiagnosticStatus::Pass
        } else {
            DiagnosticStatus::Warn
        },
        required: false,
        detected,
        version: None,
        path: path.map(|value| value.display().to_string()),
        message: if detected {
            "可调用系统打开或定位生成文件。".to_string()
        } else {
            "未在 PATH 中找到系统文件打开器命令。".to_string()
        },
        fix_hint: if detected {
            None
        } else {
            Some("确认系统文件管理器命令在 PATH 中可用。".to_string())
        },
    }
}

fn office_viewer_component() -> EnvironmentComponent {
    EnvironmentComponent {
        id: "office_viewer",
        label: "Office/WPS 查看器",
        status: DiagnosticStatus::Info,
        required: false,
        detected: false,
        version: None,
        path: None,
        message: "生成文件不依赖 Office；打开、检查和继续编辑 PPTX/DOCX/XLSX 时需要 Office、WPS、Keynote 或同类软件。"
            .to_string(),
        fix_hint: None,
    }
}

fn command_output_line(bytes: &[u8]) -> Option<String> {
    let raw = String::from_utf8_lossy(bytes);
    raw.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn command_path(command: &str) -> Option<String> {
    find_command_on_path(command).map(|path| path.display().to_string())
}

fn find_command_on_path(command: &str) -> Option<PathBuf> {
    let command_path = PathBuf::from(command);
    if command_path.components().count() > 1 && command_path.is_file() {
        return Some(command_path);
    }
    let path = env::var_os("PATH")?;
    for dir in env::split_paths(&path) {
        let candidate = dir.join(command);
        if candidate.is_file() {
            return Some(candidate);
        }
        if cfg!(target_os = "windows") && !command.contains('.') {
            for extension in ["exe", "cmd", "bat"] {
                let candidate = dir.join(format!("{command}.{extension}"));
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }
    None
}

fn readiness_for(components: &[EnvironmentComponent]) -> DiagnosticStatus {
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

fn summary_for(readiness: DiagnosticStatus, components: &[EnvironmentComponent]) -> String {
    match readiness {
        DiagnosticStatus::Pass => "核心依赖可用，当前环境可以完整运行本机模板工作流。".to_string(),
        DiagnosticStatus::Warn => {
            let optional_count = components
                .iter()
                .filter(|component| {
                    !component.required
                        && matches!(
                            component.status,
                            DiagnosticStatus::Fail | DiagnosticStatus::Warn
                        )
                })
                .count();
            format!("核心功能可运行，但有 {optional_count} 个可选能力需要确认。")
        }
        DiagnosticStatus::Fail => "存在必需依赖或本地存储问题，部分核心功能无法运行。".to_string(),
        DiagnosticStatus::Info => "环境信息已收集。".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_diagnostics_reports_expected_components() {
        let diagnostics = environment_diagnostics();
        let ids: Vec<&str> = diagnostics
            .components
            .iter()
            .map(|component| component.id)
            .collect();
        assert!(ids.contains(&"app_core"));
        assert!(ids.contains(&"local_storage"));
        assert!(ids.contains(&"pptx_sidecar"));
        assert!(ids.contains(&"node"));
        assert!(ids.contains(&"python3"));
    }
}
