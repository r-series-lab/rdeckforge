use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptPackManifest {
    pub schema_version: String,
    pub prompt_pack_id: String,
    pub name: String,
    #[serde(default = "default_prompt_entry")]
    pub entry: String,
    #[serde(default)]
    pub output_schema: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptBuildResult {
    pub output_file: String,
    pub prompt_bytes: usize,
    pub prompt_pack_id: String,
    pub prompt_pack_name: String,
    pub template_id: String,
    pub template_name: String,
    pub input_formats: Vec<String>,
    pub input_profile: Option<String>,
    pub input_schema_label: Option<String>,
    pub schema_included: bool,
    pub template_contract_mode: Option<String>,
    pub content_skeleton_included: bool,
    pub template_authoring_included: bool,
}

pub fn build_prompt(
    pack_dir: &Path,
    brief_file: &Path,
    template_dir: &Path,
    out_file: &Path,
) -> Result<PromptBuildResult, String> {
    crate::validator::require_dir(pack_dir, "prompt pack")?;
    crate::validator::require_file(brief_file, "brief")?;
    let pack_manifest = load_prompt_pack(pack_dir)?;
    let prompt_path = pack_dir.join(&pack_manifest.entry);
    crate::validator::require_file(&prompt_path, "prompt pack entry")?;

    let prompt = fs::read_to_string(&prompt_path).map_err(|err| {
        format!(
            "failed to read prompt entry '{}': {err}",
            prompt_path.display()
        )
    })?;
    let brief =
        fs::read_to_string(brief_file).map_err(|err| format!("failed to read brief: {err}"))?;
    let manifest = crate::template_manifest::load_manifest(template_dir)?;
    let input_spec = crate::template_manifest::normalized_input_spec(&manifest);
    let authoring_context =
        crate::template_authoring::load_template_authoring_context(template_dir, &input_spec)?;
    let authoring_section = build_template_authoring_section(&authoring_context);
    let template_authoring_included = !authoring_context.is_empty();
    let effective_schema =
        crate::schema_validation::effective_input_schema_document(template_dir, &input_spec)?;
    let prompt_pack_schema = match pack_manifest.output_schema.as_deref() {
        Some(output_schema) => Some(read_output_schema(pack_dir, output_schema)?),
        None => None,
    };
    let manifest_summary = serde_json::to_string_pretty(&manifest)
        .map_err(|err| format!("failed to serialize template manifest: {err}"))?;
    let template_contract = crate::template_contract::export_template_contract(template_dir, None)?;
    let template_contract_mode = template_contract
        .contract
        .get("aiOutput")
        .and_then(|value| value.get("preferredMode"))
        .and_then(Value::as_str)
        .map(ToString::to_string);
    let template_contract_raw = serde_json::to_string_pretty(&template_contract.contract)
        .map_err(|err| format!("failed to serialize template contract: {err}"))?;
    let (content_skeleton_section, content_skeleton_included) =
        build_content_skeleton_section(template_dir)?;
    let schema_section =
        build_schema_section(prompt_pack_schema.as_ref(), effective_schema.as_ref())?;
    let validation_input_file = sample_validation_input_file(&input_spec.formats);
    let output_ext = match manifest.format.as_str() {
        "docx" | "xlsx" => manifest.format.as_str(),
        _ => "pptx",
    };

    let built = format!(
        "{prompt}\n\n\
## Brief\n\n\
{brief}\n\n\
{authoring_section}\
## AI Output Boundary\n\n\
- Generate only the content files requested by the prompt pack.\n\
- Do not generate PowerPoint, Word, Excel, Office XML, slide coordinates, or template files.\n\
- Keep local asset references relative to the content pack directory when assets are needed.\n\
- The app will validate the output against the schema and render it through the linked template.\n\n\
## How To Use The Template Contract\n\n\
- Treat `aiOutput.preferredMode` as the source of truth for the output shape.\n\
- If preferredMode is `script_renderer`, generate the JSON or Markdown input accepted by the script template; do not invent pageTemplates or bindings.\n\
- If preferredMode is `explicit_pages`, every `pages[]` item is one final PPTX slide and the app will not infer missing pages.\n\
- If preferredMode is `document_recipe` or `workbook_recipe`, generate content matching the document or workbook recipe bindings.\n\n\
## Template Contract\n\n\
```json\n{template_contract_raw}\n```\n\n\
{content_skeleton_section}\
{schema_section}\
## Template Capabilities\n\n\
```json\n{manifest_summary}\n```\n\n\
## Local Validation And Render Command\n\n\
```bash\n\
rdeckforge workflow run --template {template_dir} --content /path/to/ai-output/{validation_input_file} --normalize --out /path/to/output.{output_ext} --json\n\
```\n\n\
If validation fails, inspect `data.repairHints[]`. If normalization ran, inspect `data.normalization.fixes[]` before asking for another AI revision. Use `code`, `target`, `blocking`, `sourcePath`, and `suggestedAction` for automated repair routing; use `message` for display only.\n",
        template_dir = template_dir.display(),
        output_ext = output_ext
    );
    if let Some(parent) = out_file.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create prompt output directory: {err}"))?;
    }
    fs::write(out_file, &built).map_err(|err| format!("failed to write prompt: {err}"))?;

    let input_schema_label = effective_schema.map(|schema| schema.label);
    let schema_included = prompt_pack_schema.is_some() || input_schema_label.is_some();

    Ok(PromptBuildResult {
        output_file: out_file.display().to_string(),
        prompt_bytes: built.len(),
        prompt_pack_id: pack_manifest.prompt_pack_id,
        prompt_pack_name: pack_manifest.name,
        template_id: manifest.template_id,
        template_name: manifest.name,
        input_formats: input_spec.formats,
        input_profile: input_spec.profile,
        input_schema_label,
        schema_included,
        template_contract_mode,
        content_skeleton_included,
        template_authoring_included,
    })
}

pub fn load_prompt_pack(pack_dir: &Path) -> Result<PromptPackManifest, String> {
    crate::validator::require_dir(pack_dir, "prompt pack")?;
    let manifest_path = pack_dir.join("prompt-pack.json");
    crate::validator::require_file(&manifest_path, "prompt-pack.json")?;
    let raw = fs::read_to_string(&manifest_path)
        .map_err(|err| format!("failed to read prompt-pack.json: {err}"))?;
    let manifest = serde_json::from_str::<PromptPackManifest>(&raw)
        .map_err(|err| format!("invalid prompt-pack.json: {err}"))?;
    if manifest.schema_version != "1.0" {
        return Err(format!(
            "unsupported prompt pack schemaVersion '{}'",
            manifest.schema_version
        ));
    }
    crate::validator::require_file(&pack_dir.join(&manifest.entry), "prompt pack entry")?;
    Ok(manifest)
}

fn default_prompt_entry() -> String {
    "prompt.md".to_string()
}

fn read_output_schema(pack_dir: &Path, output_schema: &str) -> Result<Value, String> {
    let schema_path = pack_dir.join(output_schema);
    crate::validator::require_file(&schema_path, "prompt pack output schema")?;
    let raw = fs::read_to_string(&schema_path).map_err(|err| {
        format!(
            "failed to read prompt pack output schema '{}': {err}",
            schema_path.display()
        )
    })?;
    let value = serde_json::from_str::<Value>(&raw).map_err(|err| {
        format!(
            "invalid prompt pack output schema JSON '{}': {err}",
            schema_path.display()
        )
    })?;
    resolve_top_level_ref(&schema_path, value)
}

fn resolve_top_level_ref(schema_path: &Path, value: Value) -> Result<Value, String> {
    let Some(reference) = value.get("$ref").and_then(Value::as_str) else {
        return Ok(value);
    };
    if reference.starts_with("http://") || reference.starts_with("https://") {
        return Ok(value);
    }
    let Some(parent) = schema_path.parent() else {
        return Ok(value);
    };
    let resolved_path = normalize_join(parent, reference);
    crate::validator::require_file(&resolved_path, "referenced prompt pack output schema")?;
    let raw = fs::read_to_string(&resolved_path).map_err(|err| {
        format!(
            "failed to read referenced prompt pack output schema '{}': {err}",
            resolved_path.display()
        )
    })?;
    serde_json::from_str::<Value>(&raw).map_err(|err| {
        format!(
            "invalid referenced prompt pack output schema JSON '{}': {err}",
            resolved_path.display()
        )
    })
}

fn normalize_join(parent: &Path, reference: &str) -> PathBuf {
    let mut path = parent.to_path_buf();
    for part in Path::new(reference).components() {
        match part {
            std::path::Component::ParentDir => {
                path.pop();
            }
            std::path::Component::CurDir => {}
            other => path.push(other.as_os_str()),
        }
    }
    path
}

fn build_template_authoring_section(
    context: &crate::template_authoring::TemplateAuthoringContext,
) -> String {
    if context.is_empty() {
        return String::new();
    }
    let mut section = String::from(
        "## Template Authoring Instructions\n\nFollow these template-local rules before producing the final content file.\n\n",
    );
    for instruction in &context.instructions {
        section.push_str(&format!(
            "### `{}`\n\n{}\n\n",
            instruction.path,
            instruction.content.trim()
        ));
    }
    if !context.examples.is_empty() {
        section.push_str("## Template Authoring Examples\n\n");
        for example in &context.examples {
            section.push_str(&format!(
                "### `{}`\n\n{}\n\n",
                example.path,
                example.content.trim()
            ));
        }
    }
    section
}

fn build_schema_section(
    prompt_pack_schema: Option<&Value>,
    effective_schema: Option<&crate::schema_validation::InputSchemaDocument>,
) -> Result<String, String> {
    if let Some(schema) = prompt_pack_schema {
        let raw = serde_json::to_string_pretty(schema)
            .map_err(|err| format!("failed to serialize prompt pack output schema: {err}"))?;
        return Ok(format!("## Output JSON Schema\n\n```json\n{raw}\n```\n\n"));
    }
    if let Some(schema) = effective_schema {
        let raw = serde_json::to_string_pretty(&schema.schema)
            .map_err(|err| format!("failed to serialize template input schema: {err}"))?;
        return Ok(format!(
            "## Output JSON Schema\n\nSchema source: `{}`\n\n```json\n{}\n```\n\n",
            schema.label, raw
        ));
    }
    Ok(String::new())
}

fn sample_validation_input_file(formats: &[String]) -> &'static str {
    if formats
        .iter()
        .any(|format| format == "md" || format == "markdown")
        && !formats.iter().any(|format| format == "json")
    {
        "content.md"
    } else {
        "content.json"
    }
}

fn build_content_skeleton_section(template_dir: &Path) -> Result<(String, bool), String> {
    match crate::template_contract::export_content_skeleton(
        template_dir,
        None,
        Some("minimal"),
        None,
    ) {
        Ok(result) => {
            let raw = serde_json::to_string_pretty(&result.content)
                .map_err(|err| format!("failed to serialize content skeleton: {err}"))?;
            let warnings = if result.warnings.is_empty() {
                String::new()
            } else {
                format!(
                    "\nSkeleton warnings:\n{}\n",
                    result
                        .warnings
                        .iter()
                        .map(|warning| format!("- {warning}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            };
            Ok((
                format!(
                    "## Minimal Content Skeleton\n\nUse this as a compact starting point when it matches the brief. Expand repeated arrays and fill real content as needed.{warnings}\n\n```json\n{raw}\n```\n\n"
                ),
                true,
            ))
        }
        Err(err) => Ok((
            format!(
                "## Minimal Content Skeleton\n\nNo declarative skeleton is available for this template: `{err}`. Follow the Template Contract and prompt pack instructions instead.\n\n"
            ),
            false,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prompt_includes_template_contract_and_skeleton() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-prompt-contract-{}",
            uuid::Uuid::new_v4()
        ));
        let pack_dir = root.join("prompt-pack");
        fs::create_dir_all(&pack_dir)
            .map_err(|err| format!("failed to create prompt pack: {err}"))?;
        fs::write(
            pack_dir.join("prompt-pack.json"),
            r#"{
  "schemaVersion": "1.0",
  "promptPackId": "test-contract-handoff",
  "name": "Test Contract Handoff",
  "entry": "prompt.md"
}"#,
        )
        .map_err(|err| format!("failed to write prompt pack manifest: {err}"))?;
        fs::write(
            pack_dir.join("prompt.md"),
            "Generate content from the brief.\n",
        )
        .map_err(|err| format!("failed to write prompt entry: {err}"))?;
        let brief = root.join("brief.md");
        fs::write(&brief, "# Brief\n\nBuild a short nursing deck.\n")
            .map_err(|err| format!("failed to write brief: {err}"))?;
        let out = root.join("prompt.md");

        let result = build_prompt(
            &pack_dir,
            &brief,
            Path::new("examples/templates/demo-medical-teaching-v1"),
            &out,
        )?;

        let raw =
            fs::read_to_string(&out).map_err(|err| format!("failed to read prompt: {err}"))?;
        assert_eq!(
            result.template_contract_mode.as_deref(),
            Some("explicit_pages")
        );
        assert!(result.content_skeleton_included);
        assert!(raw.contains("## Template Contract"));
        assert!(raw.contains("\"preferredMode\": \"explicit_pages\""));
        assert!(raw.contains("## Minimal Content Skeleton"));
        assert!(raw.contains("rdeckforge workflow run"));
        assert!(raw.contains("data.repairHints[]"));
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn build_prompt_uses_script_renderer_contract_for_script_template() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-prompt-script-{}", uuid::Uuid::new_v4()));
        let pack_dir = root.join("prompt-pack");
        let template_dir = root.join("script-template");
        fs::create_dir_all(&pack_dir)
            .map_err(|err| format!("failed to create prompt pack: {err}"))?;
        fs::create_dir_all(template_dir.join("scripts"))
            .map_err(|err| format!("failed to create script dir: {err}"))?;
        fs::create_dir_all(template_dir.join("prompts"))
            .map_err(|err| format!("failed to create template prompt dir: {err}"))?;
        fs::write(
            pack_dir.join("prompt-pack.json"),
            r#"{
  "schemaVersion": "1.0",
  "promptPackId": "test-script-handoff",
  "name": "Test Script Handoff",
  "entry": "prompt.md"
}"#,
        )
        .map_err(|err| format!("failed to write prompt pack manifest: {err}"))?;
        fs::write(
            pack_dir.join("prompt.md"),
            "Generate the script renderer input from the brief.\n",
        )
        .map_err(|err| format!("failed to write prompt entry: {err}"))?;
        fs::write(
            template_dir.join("scripts").join("build.py"),
            "print('ok')\n",
        )
        .map_err(|err| format!("failed to write script: {err}"))?;
        fs::write(
            template_dir.join("prompts").join("authoring.md"),
            "Keep every chapter to four concise teaching points.\n",
        )
        .map_err(|err| format!("failed to write template authoring instructions: {err}"))?;
        fs::write(
            template_dir.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "script-prompt-test",
  "name": "Script Prompt Test",
  "format": "pptx",
  "templateType": "script",
  "input": {
    "formats": ["md"],
    "authoring": {
      "instructions": ["prompts/authoring.md"]
    }
  },
  "renderer": {
    "type": "script.python",
    "runtime": "python3",
    "entry": "scripts/build.py"
  }
}"#,
        )
        .map_err(|err| format!("failed to write template manifest: {err}"))?;
        let brief = root.join("brief.md");
        fs::write(&brief, "# Brief\n\nBuild a script-rendered deck.\n")
            .map_err(|err| format!("failed to write brief: {err}"))?;
        let out = root.join("script-prompt.md");

        let result = build_prompt(&pack_dir, &brief, &template_dir, &out)?;

        let raw =
            fs::read_to_string(&out).map_err(|err| format!("failed to read prompt: {err}"))?;
        assert_eq!(
            result.template_contract_mode.as_deref(),
            Some("script_renderer")
        );
        assert!(result.template_authoring_included);
        assert!(!result.content_skeleton_included);
        assert!(raw.contains("\"preferredMode\": \"script_renderer\""));
        assert!(raw.contains("## Template Authoring Instructions"));
        assert!(raw.contains("Keep every chapter to four concise teaching points."));
        assert!(!raw.contains("\"pptxExplicitPages\""));
        assert!(raw.contains("/path/to/ai-output/content.md"));
        fs::remove_dir_all(&root).ok();
        Ok(())
    }
}
