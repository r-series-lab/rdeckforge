use clap::{Parser, Subcommand, error::ErrorKind};
use serde_json::{Value, json};
use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::ExitCode,
};

#[derive(Parser)]
#[command(name = "rdeckforge")]
#[command(bin_name = "rdeckforge")]
#[command(about = "Private template pack renderer workbench.")]
#[command(version)]
struct Cli {
    #[arg(long, global = true)]
    json: bool,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Info,
    Capabilities,
    Prompt {
        #[command(subcommand)]
        command: PromptCommands,
    },
    Template {
        #[command(subcommand)]
        command: TemplateCommands,
    },
    Content {
        #[command(subcommand)]
        command: ContentCommands,
    },
    Profile {
        #[command(subcommand)]
        command: ProfileCommands,
    },
    Render {
        #[command(subcommand)]
        command: RenderCommands,
    },
    Workflow {
        #[command(subcommand)]
        command: WorkflowCommands,
    },
    Diagnostics {
        #[command(subcommand)]
        command: DiagnosticsCommands,
    },
    History {
        #[command(subcommand)]
        command: HistoryCommands,
    },
}

#[derive(Subcommand)]
enum PromptCommands {
    Build {
        #[arg(long)]
        pack: PathBuf,
        #[arg(long)]
        brief: PathBuf,
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum TemplateCommands {
    #[command(name = "list", alias = "list-linked")]
    ListLinked,
    Link {
        #[arg(long)]
        template: PathBuf,
    },
    #[command(name = "export", alias = "export-pack")]
    ExportArchive {
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        force: bool,
    },
    #[command(name = "import", alias = "import-pack")]
    ImportArchive {
        #[arg(long)]
        archive: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
        #[arg(long)]
        force: bool,
    },
    Refresh {
        #[arg(long)]
        id: String,
    },
    Relink {
        #[arg(long)]
        id: String,
        #[arg(long)]
        template: PathBuf,
    },
    #[command(name = "remove", alias = "forget", alias = "delete", alias = "unlink")]
    Forget {
        #[arg(long)]
        id: String,
    },
    Validate {
        #[arg(long)]
        template: PathBuf,
    },
    #[command(name = "readiness", alias = "diagnose")]
    Readiness {
        #[arg(long)]
        template: PathBuf,
    },
    Test {
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        case: Option<String>,
        #[arg(long)]
        out_dir: Option<PathBuf>,
    },
    Contract {
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    ContentSkeleton {
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        out: Option<PathBuf>,
        #[arg(long, default_value = "minimal")]
        mode: String,
        #[arg(long)]
        recipe: Option<String>,
    },
    CreateScriptAdapter {
        #[arg(long)]
        script: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
        #[arg(long)]
        id: String,
        #[arg(long)]
        name: String,
        #[arg(long, default_value = "pptx")]
        format: String,
        #[arg(long, default_value = "md")]
        input_format: String,
        #[arg(long)]
        renderer_type: Option<String>,
        #[arg(long)]
        runtime: Option<String>,
        #[arg(long)]
        link_script: bool,
        #[arg(long)]
        force: bool,
    },
    InspectPptx {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    CreatePptxDraft {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
    },
    InspectDocx {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    CreateDocxDraft {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
    },
    InspectXlsx {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    CreateXlsxDraft {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
    },
}

#[derive(Subcommand)]
enum ContentCommands {
    Validate {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        template: Option<PathBuf>,
        #[arg(long)]
        recipe: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        out: Option<PathBuf>,
    },
    Normalize {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        template: Option<PathBuf>,
        #[arg(long)]
        recipe: Option<String>,
        #[arg(long)]
        profile: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum ProfileCommands {
    List,
}

#[derive(Subcommand)]
enum RenderCommands {
    Office {
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        recipe: Option<String>,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Family {
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out_dir: PathBuf,
        #[arg(long)]
        base_name: Option<String>,
        #[arg(long)]
        recipe: Option<String>,
        #[arg(long)]
        fail_fast: bool,
    },
    Pptx {
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        recipe: Option<String>,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        md: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        dry_run: bool,
    },
    Docx {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        template: Option<PathBuf>,
        #[arg(long)]
        recipe: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
    Xlsx {
        #[arg(long)]
        template: Option<PathBuf>,
        #[arg(long)]
        recipe: Option<String>,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum WorkflowCommands {
    Prepare {
        #[arg(long)]
        prompt_pack: PathBuf,
        #[arg(long)]
        brief: PathBuf,
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        prompt_out: PathBuf,
        #[arg(long)]
        contract_out: Option<PathBuf>,
        #[arg(long)]
        skeleton_out: Option<PathBuf>,
        #[arg(long, default_value = "minimal")]
        skeleton_mode: String,
        #[arg(long)]
        recipe: Option<String>,
    },
    Handoff {
        #[arg(long)]
        prompt_pack: PathBuf,
        #[arg(long)]
        brief: PathBuf,
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        out: PathBuf,
    },
    Render {
        #[arg(long)]
        template: PathBuf,
        #[arg(long)]
        content: PathBuf,
        #[arg(long)]
        recipe: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
    Run {
        #[arg(long)]
        template: PathBuf,
        #[arg(long, alias = "input", alias = "ai-output")]
        content: PathBuf,
        #[arg(long)]
        recipe: Option<String>,
        #[arg(long)]
        normalize: bool,
        #[arg(long)]
        normalize_out: Option<PathBuf>,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        validation_out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum DiagnosticsCommands {
    Env,
    Xlsx {
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        template: Option<PathBuf>,
        #[arg(long)]
        recipe: Option<String>,
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum HistoryCommands {
    List {
        #[arg(long, default_value_t = 50)]
        limit: usize,
    },
    Rerender {
        #[arg(long)]
        id: String,
    },
}

#[derive(Debug)]
struct CommandError {
    code: &'static str,
    message: String,
    exit_code: u8,
    data: Option<Value>,
}

impl CommandError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            code: "invalid_arguments",
            message: message.into(),
            exit_code: 2,
            data: None,
        }
    }

    fn missing(message: impl Into<String>) -> Self {
        Self {
            code: "missing_resource",
            message: message.into(),
            exit_code: 3,
            data: None,
        }
    }

    fn partial(message: impl Into<String>, data: Value) -> Self {
        Self {
            code: "partial_failure",
            message: message.into(),
            exit_code: 1,
            data: Some(data),
        }
    }

    fn content_validation(message: impl Into<String>, data: Value) -> Self {
        Self {
            code: "content_validation_failed",
            message: message.into(),
            exit_code: 2,
            data: Some(data),
        }
    }
}

fn main() -> ExitCode {
    let args = env::args_os().collect::<Vec<_>>();
    let json_mode = json_requested(&args);
    let requested_command = requested_command_from_args(&args);
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) => return handle_parse_error(error, json_mode, &requested_command),
    };
    let command = command_name(&cli.command);
    match run(&cli) {
        Ok(data) => {
            print_success(cli.json, command, data);
            ExitCode::SUCCESS
        }
        Err(error) => {
            print_error(cli.json, command, &error);
            ExitCode::from(error.exit_code)
        }
    }
}

fn json_requested(args: &[OsString]) -> bool {
    args.iter().any(|arg| arg.to_string_lossy() == "--json")
}

fn requested_command_from_args(args: &[OsString]) -> String {
    if args
        .iter()
        .any(|arg| matches!(arg.to_string_lossy().as_ref(), "--version" | "-V"))
    {
        return "version".to_string();
    }

    let positional = args
        .iter()
        .skip(1)
        .map(|arg| arg.to_string_lossy())
        .filter(|arg| !arg.starts_with('-'))
        .take(2)
        .map(|arg| arg.into_owned())
        .collect::<Vec<_>>();

    match positional.as_slice() {
        [root, nested] if command_has_nested_verbs(root) => format!("{root}.{nested}"),
        [root, ..] => root.clone(),
        [] => "help".to_string(),
    }
}

fn command_has_nested_verbs(command: &str) -> bool {
    matches!(
        command,
        "prompt"
            | "template"
            | "content"
            | "profile"
            | "render"
            | "workflow"
            | "diagnostics"
            | "history"
    )
}

fn handle_parse_error(error: clap::Error, json_mode: bool, target: &str) -> ExitCode {
    let is_help = matches!(
        error.kind(),
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
    );
    if json_mode {
        let message = error.to_string().trim().to_string();
        if is_help {
            println!(
                "{}",
                json!({
                    "ok": true,
                    "command": if error.kind() == ErrorKind::DisplayVersion { "version" } else { "help" },
                    "data": {
                        "target": target,
                        "message": message,
                    }
                })
            );
            return ExitCode::SUCCESS;
        }

        println!(
            "{}",
            json!({
                "ok": false,
                "command": target,
                "error": {
                    "code": "invalid_arguments",
                    "message": message,
                }
            })
        );
        return ExitCode::from(2);
    }

    let _ = error.print();
    if is_help {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(2)
    }
}

fn command_name(command: &Commands) -> &'static str {
    match command {
        Commands::Info => "info",
        Commands::Capabilities => "capabilities",
        Commands::Prompt { command } => match command {
            PromptCommands::Build { .. } => "prompt.build",
        },
        Commands::Template { command } => match command {
            TemplateCommands::ListLinked => "template.list",
            TemplateCommands::Link { .. } => "template.link",
            TemplateCommands::ExportArchive { .. } => "template.export",
            TemplateCommands::ImportArchive { .. } => "template.import",
            TemplateCommands::Refresh { .. } => "template.refresh",
            TemplateCommands::Relink { .. } => "template.relink",
            TemplateCommands::Forget { .. } => "template.remove",
            TemplateCommands::Validate { .. } => "template.validate",
            TemplateCommands::Readiness { .. } => "template.readiness",
            TemplateCommands::Test { .. } => "template.test",
            TemplateCommands::Contract { .. } => "template.contract",
            TemplateCommands::ContentSkeleton { .. } => "template.content-skeleton",
            TemplateCommands::CreateScriptAdapter { .. } => "template.create-script-adapter",
            TemplateCommands::InspectPptx { .. } => "template.inspect-pptx",
            TemplateCommands::CreatePptxDraft { .. } => "template.create-pptx-draft",
            TemplateCommands::InspectDocx { .. } => "template.inspect-docx",
            TemplateCommands::CreateDocxDraft { .. } => "template.create-docx-draft",
            TemplateCommands::InspectXlsx { .. } => "template.inspect-xlsx",
            TemplateCommands::CreateXlsxDraft { .. } => "template.create-xlsx-draft",
        },
        Commands::Content { command } => match command {
            ContentCommands::Validate { .. } => "content.validate",
            ContentCommands::Normalize { .. } => "content.normalize",
        },
        Commands::Profile { command } => match command {
            ProfileCommands::List => "profile.list",
        },
        Commands::Render { command } => match command {
            RenderCommands::Office { .. } => "render.office",
            RenderCommands::Family { .. } => "render.family",
            RenderCommands::Pptx { .. } => "render.pptx",
            RenderCommands::Docx { .. } => "render.docx",
            RenderCommands::Xlsx { .. } => "render.xlsx",
        },
        Commands::Workflow { command } => match command {
            WorkflowCommands::Prepare { .. } => "workflow.prepare",
            WorkflowCommands::Handoff { .. } => "workflow.handoff",
            WorkflowCommands::Render { .. } => "workflow.render",
            WorkflowCommands::Run { .. } => "workflow.run",
        },
        Commands::Diagnostics { command } => match command {
            DiagnosticsCommands::Env => "diagnostics.env",
            DiagnosticsCommands::Xlsx { .. } => "diagnostics.xlsx",
        },
        Commands::History { command } => match command {
            HistoryCommands::List { .. } => "history.list",
            HistoryCommands::Rerender { .. } => "history.rerender",
        },
    }
}

fn run(cli: &Cli) -> Result<Value, CommandError> {
    match &cli.command {
        Commands::Info => Ok(json!(rdeckforge_core::capabilities::app_info())),
        Commands::Capabilities => Ok(json!(rdeckforge_core::capabilities::capabilities())),
        Commands::Prompt { command } => match command {
            PromptCommands::Build {
                pack,
                brief,
                template,
                out,
            } => rdeckforge_core::prompt_pack::build_prompt(pack, brief, template, out)
                .map(|result| json!(result))
                .map_err(map_validation_error),
        },
        Commands::Template { command } => match command {
            TemplateCommands::ListLinked => rdeckforge_core::storage::list_template_packs()
                .map(|result| json!(result))
                .map_err(map_validation_error),
            TemplateCommands::Link { template } => {
                rdeckforge_core::storage::link_template_pack(template)
                    .map(|result| json!(result))
                    .map_err(map_validation_error)
            }
            TemplateCommands::ExportArchive {
                template,
                out,
                force,
            } => rdeckforge_core::template_archive::export_template_pack_archive(
                template, out, *force,
            )
            .map(|result| json!(result))
            .map_err(map_validation_error),
            TemplateCommands::ImportArchive {
                archive,
                out_dir,
                force,
            } => rdeckforge_core::template_archive::import_and_link_template_pack_archive(
                archive, out_dir, *force,
            )
            .map(|result| json!(result))
            .map_err(map_validation_error),
            TemplateCommands::Refresh { id } => rdeckforge_core::storage::refresh_template_pack(id)
                .map(|result| json!(result))
                .map_err(map_validation_error),
            TemplateCommands::Relink { id, template } => {
                rdeckforge_core::storage::relink_template_pack(id, template)
                    .map(|result| json!(result))
                    .map_err(map_validation_error)
            }
            TemplateCommands::Forget { id } => rdeckforge_core::storage::forget_template_pack(id)
                .map(|result| json!(result))
                .map_err(map_validation_error),
            TemplateCommands::Validate { template } => {
                rdeckforge_core::template_manifest::validate_template_pack(template)
                    .map(|result| json!(result))
                    .map_err(map_validation_error)
            }
            TemplateCommands::Readiness { template } => {
                rdeckforge_core::template_readiness::diagnose_template_readiness(template)
                    .map(|result| json!(result))
                    .map_err(map_validation_error)
            }
            TemplateCommands::Test {
                template,
                case,
                out_dir,
            } => {
                let report = rdeckforge_core::template_self_test::run_template_tests(
                    template,
                    case.as_deref(),
                    out_dir.as_deref(),
                )
                .map_err(map_validation_error)?;
                let passed = report.passed;
                let failed_count = report.failed_count;
                let selected_count = report.selected_count;
                let value = json!(report);
                if passed {
                    Ok(value)
                } else {
                    Err(CommandError::partial(
                        format!("{failed_count} of {selected_count} template tests failed"),
                        value,
                    ))
                }
            }
            TemplateCommands::Contract { template, out } => {
                rdeckforge_core::template_contract::export_template_contract(
                    template,
                    out.as_deref(),
                )
                .map(|result| json!(result))
                .map_err(map_validation_error)
            }
            TemplateCommands::ContentSkeleton {
                template,
                out,
                mode,
                recipe,
            } => rdeckforge_core::template_contract::export_content_skeleton(
                template,
                out.as_deref(),
                Some(mode),
                recipe.as_deref(),
            )
            .map(|result| json!(result))
            .map_err(map_validation_error),
            TemplateCommands::CreateScriptAdapter {
                script,
                out_dir,
                id,
                name,
                format,
                input_format,
                renderer_type,
                runtime,
                link_script,
                force,
            } => rdeckforge_core::script_render::create_script_template_adapter(
                rdeckforge_core::script_render::ScriptTemplateAdapterOptions {
                    script,
                    out_dir,
                    template_id: id,
                    name,
                    format,
                    input_format,
                    renderer_type: renderer_type.as_deref(),
                    runtime: runtime.as_deref(),
                    copy_script: !*link_script,
                    force: *force,
                },
            )
            .map(|result| json!(result))
            .map_err(map_validation_error),
            TemplateCommands::InspectPptx { input, out } => {
                rdeckforge_core::template_manifest::inspect_pptx(input, out)
                    .map(|result| json!(result))
                    .map_err(map_validation_error)
            }
            TemplateCommands::CreatePptxDraft { input, out_dir } => {
                rdeckforge_core::template_manifest::create_pptx_template_pack_draft(input, out_dir)
                    .map(|result| json!(result))
                    .map_err(map_validation_error)
            }
            TemplateCommands::InspectDocx { input, out } => {
                rdeckforge_core::template_manifest::inspect_docx(input, out)
                    .map(|result| json!(result))
                    .map_err(map_validation_error)
            }
            TemplateCommands::CreateDocxDraft { input, out_dir } => {
                rdeckforge_core::template_manifest::create_docx_template_pack_draft(input, out_dir)
                    .map(|result| json!(result))
                    .map_err(map_validation_error)
            }
            TemplateCommands::InspectXlsx { input, out } => {
                rdeckforge_core::template_manifest::inspect_xlsx(input, out)
                    .map(|result| json!(result))
                    .map_err(map_validation_error)
            }
            TemplateCommands::CreateXlsxDraft { input, out_dir } => {
                rdeckforge_core::template_manifest::create_xlsx_template_pack_draft(input, out_dir)
                    .map(|result| json!(result))
                    .map_err(map_validation_error)
            }
        },
        Commands::Content { command } => match command {
            ContentCommands::Validate {
                input,
                template,
                recipe,
                profile,
                out,
            } => {
                let validation =
                    rdeckforge_core::content_ir::validate_content_workspace_file_with_profile(
                        input,
                        template.as_deref(),
                        recipe.as_deref(),
                        profile.as_deref(),
                    )
                    .map_err(map_validation_error)?;
                let value = json!(validation);
                if let Some(out) = out {
                    let report_file = write_json_file(out, &value, "content validation report")?;
                    Ok(json!({
                        "reportFile": report_file,
                        "validation": value
                    }))
                } else {
                    Ok(value)
                }
            }
            ContentCommands::Normalize {
                input,
                template,
                recipe,
                profile,
                out,
            } => rdeckforge_core::content_normalize::normalize_content_file(
                input,
                template.as_deref(),
                recipe.as_deref(),
                profile.as_deref(),
                out,
            )
            .map(|result| json!(result))
            .map_err(map_validation_error),
        },
        Commands::Profile { command } => match command {
            ProfileCommands::List => Ok(json!(rdeckforge_core::profiles::list_profiles())),
        },
        Commands::Render { command } => match command {
            RenderCommands::Office {
                template,
                recipe,
                input,
                out,
            } => rdeckforge_core::accepted_render::render_accepted_office(
                input,
                template,
                recipe.as_deref(),
                out,
            )
            .map(|result| json!(result))
            .map_err(map_validation_error),
            RenderCommands::Family {
                template,
                input,
                out_dir,
                base_name,
                recipe,
                fail_fast,
            } => {
                let result = rdeckforge_core::template_family::render_linked_template_family(
                    template,
                    input,
                    out_dir,
                    base_name.as_deref(),
                    recipe.as_deref(),
                    *fail_fast,
                )
                .map_err(map_validation_error)?;
                let failed = result.failed;
                let value = json!(result);
                if failed > 0 {
                    Err(CommandError::partial(
                        format!("template family rendering failed for {failed} template(s)"),
                        value,
                    ))
                } else {
                    Ok(value)
                }
            }
            RenderCommands::Pptx {
                template,
                recipe,
                input,
                md,
                out,
                dry_run,
            } => {
                let job = rdeckforge_core::render_job::build_pptx_job(
                    template,
                    input,
                    md.as_deref(),
                    out,
                    recipe.as_deref(),
                )
                .map_err(map_validation_error)?;
                if *dry_run {
                    let result = rdeckforge_core::renderer::write_dry_run_job(&job, out)
                        .map_err(map_validation_error)?;
                    Ok(json!({
                        "job": job,
                        "result": result
                    }))
                } else {
                    let result = rdeckforge_core::renderer::render_pptx_job_atomic(&job, out)
                        .map_err(map_validation_error)?;
                    let history = rdeckforge_core::storage::record_render_history(&job, &result)
                        .map_err(map_validation_error)?;
                    Ok(json!({
                        "job": job,
                        "result": result,
                        "history": history
                    }))
                }
            }
            RenderCommands::Docx {
                input,
                template,
                recipe,
                out,
            } => rdeckforge_core::office_export::render_docx_atomic(
                input,
                template.as_deref(),
                recipe.as_deref(),
                out,
            )
            .map(|result| json!(result))
            .map_err(map_validation_error),
            RenderCommands::Xlsx {
                template,
                recipe,
                input,
                out,
            } => rdeckforge_core::office_export::render_xlsx_atomic(
                input,
                template.as_deref(),
                recipe.as_deref(),
                out,
            )
            .map(|result| json!(result))
            .map_err(map_validation_error),
        },
        Commands::Workflow { command } => match command {
            WorkflowCommands::Prepare {
                prompt_pack,
                brief,
                template,
                prompt_out,
                contract_out,
                skeleton_out,
                skeleton_mode,
                recipe,
            } => {
                let prompt = rdeckforge_core::prompt_pack::build_prompt(
                    prompt_pack,
                    brief,
                    template,
                    prompt_out,
                )
                .map_err(map_validation_error)?;
                let contract = rdeckforge_core::template_contract::export_template_contract(
                    template,
                    contract_out.as_deref(),
                )
                .map_err(map_validation_error)?;
                let skeleton = match rdeckforge_core::template_contract::export_content_skeleton(
                    template,
                    skeleton_out.as_deref(),
                    Some(skeleton_mode),
                    recipe.as_deref(),
                ) {
                    Ok(result) => json!({
                        "ok": true,
                        "result": result,
                    }),
                    Err(message) => json!({
                        "ok": false,
                        "error": {
                            "code": "skeleton_unavailable",
                            "message": message,
                        }
                    }),
                };
                Ok(json!({
                    "workflow": "prepare",
                    "prompt": prompt,
                    "contract": contract,
                    "skeleton": skeleton,
                }))
            }
            WorkflowCommands::Handoff {
                prompt_pack,
                brief,
                template,
                out,
            } => rdeckforge_core::prompt_pack::build_prompt(prompt_pack, brief, template, out)
                .map(|result| {
                    json!({
                        "workflow": "handoff",
                        "prompt": result
                    })
                })
                .map_err(map_validation_error),
            WorkflowCommands::Render {
                template,
                content,
                recipe,
                out,
            } => rdeckforge_core::accepted_render::render_accepted_office(
                content,
                template,
                recipe.as_deref(),
                out,
            )
            .map(|result| {
                json!({
                    "workflow": "render",
                    "render": result
                })
            })
            .map_err(map_validation_error),
            WorkflowCommands::Run {
                template,
                content,
                recipe,
                normalize,
                normalize_out,
                out,
                validation_out,
            } => {
                let mut normalization_value = None;
                let content_for_run = if *normalize {
                    let normalize_out_path = normalize_out
                        .clone()
                        .unwrap_or_else(|| default_normalized_output_path(out));
                    let normalization = rdeckforge_core::content_normalize::normalize_content_file(
                        content,
                        Some(template),
                        recipe.as_deref(),
                        None,
                        &normalize_out_path,
                    )
                    .map_err(map_validation_error)?;
                    normalization_value = Some(json!(normalization));
                    normalize_out_path
                } else {
                    content.clone()
                };
                let validation =
                    rdeckforge_core::content_ir::validate_content_workspace_file_with_profile(
                        &content_for_run,
                        Some(template),
                        recipe.as_deref(),
                        None,
                    )
                    .map_err(map_validation_error)?;
                let can_render = validation.acceptance_summary.can_render;
                let message = validation.acceptance_summary.message.clone();
                let repair_hints = rdeckforge_core::workflow::repair_hints(&validation);
                let validation_value = json!(validation);
                let validation_report_file = validation_out
                    .as_deref()
                    .map(|path| {
                        write_json_file(path, &validation_value, "workflow validation report")
                    })
                    .transpose()?;

                if !can_render {
                    let mut data = json!({
                        "workflow": "run",
                        "stage": "validate",
                        "inputFile": content,
                        "contentFile": content_for_run,
                        "validation": validation_value,
                        "repairHints": repair_hints,
                    });
                    if let Some(normalization) = normalization_value {
                        data["normalization"] = normalization;
                    }
                    if let Some(report_file) = validation_report_file {
                        data["validationReportFile"] = json!(report_file);
                    }
                    return Err(CommandError::content_validation(message, data));
                }

                let render = rdeckforge_core::accepted_render::render_accepted_office(
                    &content_for_run,
                    template,
                    recipe.as_deref(),
                    out,
                )
                .map_err(map_validation_error)?;
                let mut data = json!({
                    "workflow": "run",
                    "stage": "rendered",
                    "inputFile": content,
                    "contentFile": content_for_run,
                    "validation": validation_value,
                    "repairHints": repair_hints,
                    "render": render,
                });
                if let Some(normalization) = normalization_value {
                    data["normalization"] = normalization;
                }
                if let Some(report_file) = validation_report_file {
                    data["validationReportFile"] = json!(report_file);
                }
                Ok(data)
            }
        },
        Commands::Diagnostics { command } => match command {
            DiagnosticsCommands::Env => Ok(json!(
                rdeckforge_core::environment::environment_diagnostics()
            )),
            DiagnosticsCommands::Xlsx {
                input,
                template,
                recipe,
                out,
            } => rdeckforge_core::office_export::export_xlsx_report(
                input,
                template.as_deref(),
                recipe.as_deref(),
                out,
            )
            .map(|result| json!(result))
            .map_err(map_validation_error),
        },
        Commands::History { command } => match command {
            HistoryCommands::List { limit } => {
                rdeckforge_core::storage::list_render_history(*limit)
                    .map(|result| json!(result))
                    .map_err(map_validation_error)
            }
            HistoryCommands::Rerender { id } => rdeckforge_core::storage::rerender_history(id)
                .map(|result| json!(result))
                .map_err(map_validation_error),
        },
    }
}

fn map_validation_error(message: String) -> CommandError {
    if message.contains("not found") {
        CommandError::missing(message)
    } else {
        CommandError::invalid(message)
    }
}

fn write_json_file(out: &Path, value: &Value, label: &str) -> Result<String, CommandError> {
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            CommandError::invalid(format!(
                "failed to create {label} output directory '{}': {err}",
                parent.display()
            ))
        })?;
    }
    let raw = serde_json::to_string_pretty(value)
        .map_err(|err| CommandError::invalid(format!("failed to serialize {label}: {err}")))?;
    fs::write(out, raw).map_err(|err| {
        CommandError::invalid(format!(
            "failed to write {label} '{}': {err}",
            out.display()
        ))
    })?;
    Ok(out.display().to_string())
}

fn default_normalized_output_path(out: &Path) -> PathBuf {
    let parent = out.parent().unwrap_or_else(|| Path::new(""));
    let stem = out
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .unwrap_or("output");
    parent.join(format!("{stem}.normalized.json"))
}

fn print_success(json_mode: bool, command: &str, data: Value) {
    if json_mode {
        println!(
            "{}",
            json!({ "ok": true, "command": command, "data": data })
        );
    } else {
        println!("{data}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_list_aliases_parse_to_list_command() {
        for command in ["list", "list-linked"] {
            let cli = Cli::try_parse_from(["rdeckforge", "template", command]).unwrap();
            assert!(matches!(
                cli.command,
                Commands::Template {
                    command: TemplateCommands::ListLinked
                }
            ));
        }
    }

    #[test]
    fn template_remove_aliases_parse_to_forget_command() {
        for command in ["remove", "forget", "delete", "unlink"] {
            let cli = Cli::try_parse_from([
                "rdeckforge",
                "template",
                command,
                "--id",
                "template-record-id",
            ])
            .unwrap();
            assert!(matches!(
                cli.command,
                Commands::Template {
                    command: TemplateCommands::Forget { .. }
                }
            ));
        }
    }

    #[test]
    fn template_archive_commands_parse() {
        let export = Cli::try_parse_from([
            "rdeckforge",
            "template",
            "export",
            "--template",
            "template-pack",
            "--out",
            "template.rdeckpack",
        ])
        .unwrap();
        assert!(matches!(
            export.command,
            Commands::Template {
                command: TemplateCommands::ExportArchive { .. }
            }
        ));

        let import = Cli::try_parse_from([
            "rdeckforge",
            "template",
            "import",
            "--archive",
            "template.rdeckpack",
            "--out-dir",
            "templates",
        ])
        .unwrap();
        assert!(matches!(
            import.command,
            Commands::Template {
                command: TemplateCommands::ImportArchive { .. }
            }
        ));
    }

    #[test]
    fn template_readiness_aliases_parse() {
        for command in ["readiness", "diagnose"] {
            let cli = Cli::try_parse_from([
                "rdeckforge",
                "template",
                command,
                "--template",
                "template-pack",
            ])
            .unwrap();
            assert!(matches!(
                cli.command,
                Commands::Template {
                    command: TemplateCommands::Readiness { .. }
                }
            ));
        }
    }

    #[test]
    fn template_test_parses_case_and_output_directory() {
        let cli = Cli::try_parse_from([
            "rdeckforge",
            "template",
            "test",
            "--template",
            "template-pack",
            "--case",
            "smoke",
            "--out-dir",
            "test-outputs",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Template {
                command: TemplateCommands::Test {
                    case: Some(ref case),
                    out_dir: Some(ref out_dir),
                    ..
                }
            } if case == "smoke" && out_dir == &PathBuf::from("test-outputs")
        ));
    }

    #[test]
    fn script_adapter_accepts_explicit_renderer_type() {
        let cli = Cli::try_parse_from([
            "rdeckforge",
            "template",
            "create-script-adapter",
            "--script",
            "build.mjs",
            "--out-dir",
            "node-pack",
            "--id",
            "node-pack",
            "--name",
            "Node Pack",
            "--renderer-type",
            "script.node",
            "--runtime",
            "node",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Template {
                command: TemplateCommands::CreateScriptAdapter {
                    renderer_type: Some(ref value),
                    ..
                }
            } if value == "script.node"
        ));
    }

    #[test]
    fn template_office_draft_commands_parse_for_all_formats() {
        for (command, input, expected_format) in [
            ("create-pptx-draft", "template.pptx", "pptx"),
            ("create-docx-draft", "template.docx", "docx"),
            ("create-xlsx-draft", "template.xlsx", "xlsx"),
        ] {
            let cli = Cli::try_parse_from([
                "rdeckforge",
                "template",
                command,
                "--input",
                input,
                "--out-dir",
                "draft-pack",
            ])
            .unwrap();
            let parsed_format = match cli.command {
                Commands::Template {
                    command: TemplateCommands::CreatePptxDraft { .. },
                } => "pptx",
                Commands::Template {
                    command: TemplateCommands::CreateDocxDraft { .. },
                } => "docx",
                Commands::Template {
                    command: TemplateCommands::CreateXlsxDraft { .. },
                } => "xlsx",
                _ => "unknown",
            };
            assert_eq!(parsed_format, expected_format);
        }
    }

    #[test]
    fn workflow_prepare_parses_ai_handoff_outputs() {
        let cli = Cli::try_parse_from([
            "rdeckforge",
            "workflow",
            "prepare",
            "--prompt-pack",
            "examples/prompt-packs/template-contract-handoff-v1",
            "--brief",
            "examples/briefs/demo-teaching-brief.md",
            "--template",
            "examples/templates/demo-medical-teaching-v1",
            "--prompt-out",
            "target/ai-handoff-prompt.md",
            "--contract-out",
            "target/template-contract.json",
            "--skeleton-out",
            "target/content.skeleton.json",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Workflow {
                command: WorkflowCommands::Prepare { .. }
            }
        ));
    }

    #[test]
    fn workflow_run_parses_agent_friendly_content_aliases() {
        for content_arg in ["--content", "--input", "--ai-output"] {
            let cli = Cli::try_parse_from([
                "rdeckforge",
                "workflow",
                "run",
                "--template",
                "template-pack",
                content_arg,
                "content.json",
                "--out",
                "output.pptx",
                "--validation-out",
                "validation.json",
            ])
            .unwrap();
            assert!(matches!(
                cli.command,
                Commands::Workflow {
                    command: WorkflowCommands::Run {
                        validation_out: Some(ref validation_out),
                        ..
                    }
                } if validation_out == Path::new("validation.json")
            ));
            assert_eq!(command_name(&cli.command), "workflow.run");
        }
    }

    #[test]
    fn workflow_run_parses_optional_normalize_step() {
        let cli = Cli::try_parse_from([
            "rdeckforge",
            "workflow",
            "run",
            "--template",
            "template-pack",
            "--input",
            "ai-output.json",
            "--recipe",
            "teaching_deck",
            "--normalize",
            "--normalize-out",
            "normalized.json",
            "--out",
            "output.pptx",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Workflow {
                command: WorkflowCommands::Run {
                    normalize: true,
                    normalize_out: Some(ref normalize_out),
                    ..
                }
            } if normalize_out == Path::new("normalized.json")
        ));
    }

    #[test]
    fn default_normalized_output_path_uses_render_stem() {
        assert_eq!(
            default_normalized_output_path(Path::new("/tmp/output.pptx")),
            PathBuf::from("/tmp/output.normalized.json")
        );
    }

    #[test]
    fn content_normalize_parses_template_and_output() {
        let cli = Cli::try_parse_from([
            "rdeckforge",
            "content",
            "normalize",
            "--input",
            "ai-output.json",
            "--template",
            "template-pack",
            "--recipe",
            "teaching_deck",
            "--out",
            "normalized.json",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Content {
                command: ContentCommands::Normalize {
                    ref input,
                    ref out,
                    ..
                }
            } if input == Path::new("ai-output.json") && out == Path::new("normalized.json")
        ));
        assert_eq!(command_name(&cli.command), "content.normalize");
    }

    #[test]
    fn diagnostics_env_parses() {
        let cli = Cli::try_parse_from(["rdeckforge", "diagnostics", "env"]).unwrap();
        assert!(matches!(
            cli.command,
            Commands::Diagnostics {
                command: DiagnosticsCommands::Env
            }
        ));
    }

    #[test]
    fn history_rerender_parses_record_id() {
        let cli = Cli::try_parse_from([
            "rdeckforge",
            "history",
            "rerender",
            "--id",
            "history-record-id",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::History {
                command: HistoryCommands::Rerender { ref id }
            } if id == "history-record-id"
        ));
    }

    #[test]
    fn template_relink_parses_record_and_new_path() {
        let cli = Cli::try_parse_from([
            "rdeckforge",
            "template",
            "relink",
            "--id",
            "template-record-id",
            "--template",
            "/templates/moved-pack",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Template {
                command: TemplateCommands::Relink { ref id, ref template }
            } if id == "template-record-id" && template == Path::new("/templates/moved-pack")
        ));
    }

    #[test]
    fn json_intent_and_requested_command_survive_parser_errors() {
        let args = ["rdeckforge", "--json", "render", "office"]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        assert!(json_requested(&args));
        assert_eq!(requested_command_from_args(&args), "render.office");

        let error = match Cli::try_parse_from(args) {
            Ok(_) => panic!("missing render arguments should fail parsing"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument);
    }

    #[test]
    fn parsed_commands_have_stable_machine_names() {
        let cli = Cli::try_parse_from([
            "rdeckforge",
            "render",
            "docx",
            "--input",
            "content.md",
            "--out",
            "output.docx",
        ])
        .unwrap();
        assert_eq!(command_name(&cli.command), "render.docx");
    }

    #[test]
    fn render_family_parses_ai_friendly_named_arguments() {
        let cli = Cli::try_parse_from([
            "rdeckforge",
            "render",
            "family",
            "--template",
            "/templates/nursing-classic",
            "--input",
            "/content/complete-deck.json",
            "--out-dir",
            "/outputs",
            "--base-name",
            "nursing-demo",
            "--fail-fast",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Commands::Render {
                command: RenderCommands::Family {
                    fail_fast: true,
                    ..
                }
            }
        ));
        assert_eq!(command_name(&cli.command), "render.family");
    }

    #[test]
    fn json_help_is_detectable_before_clap_exits() {
        let args = ["rdeckforge", "--json", "--help"]
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        assert!(json_requested(&args));
        let error = match Cli::try_parse_from(args) {
            Ok(_) => panic!("help should exit through clap display handling"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), ErrorKind::DisplayHelp);
    }
}

fn print_error(json_mode: bool, command: &str, error: &CommandError) {
    if json_mode {
        let mut envelope = json!({
            "ok": false,
            "command": command,
            "error": {
                "code": error.code,
                "message": error.message,
            }
        });
        if let Some(data) = error.data.as_ref() {
            envelope["data"] = data.clone();
        }
        println!("{}", envelope);
    } else {
        eprintln!("{}: {}", error.code, error.message);
        if let Some(data) = error.data.as_ref() {
            println!("{data}");
        }
    }
}
