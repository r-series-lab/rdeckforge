use serde::Serialize;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

const MAX_AUTHORING_FILE_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateAuthoringFile {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateAuthoringContext {
    pub instructions: Vec<TemplateAuthoringFile>,
    pub examples: Vec<TemplateAuthoringFile>,
}

impl TemplateAuthoringContext {
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty() && self.examples.is_empty()
    }
}

pub fn load_template_authoring_context(
    template_dir: &Path,
    input: &crate::template_manifest::TemplateInputSpec,
) -> Result<TemplateAuthoringContext, String> {
    let Some(authoring) = input.authoring.as_ref() else {
        return Ok(TemplateAuthoringContext::default());
    };
    Ok(TemplateAuthoringContext {
        instructions: load_declared_files(
            template_dir,
            &authoring.instructions,
            "authoring instruction",
        )?,
        examples: load_declared_files(template_dir, &authoring.examples, "authoring example")?,
    })
}

fn load_declared_files(
    template_dir: &Path,
    declared_paths: &[String],
    label: &str,
) -> Result<Vec<TemplateAuthoringFile>, String> {
    declared_paths
        .iter()
        .map(|declared_path| {
            let path = resolve_template_relative_path(template_dir, declared_path, label)?;
            let metadata = fs::metadata(&path)
                .map_err(|err| format!("failed to inspect {label} '{}': {err}", path.display()))?;
            if metadata.len() > MAX_AUTHORING_FILE_BYTES {
                return Err(format!(
                    "{label} is larger than {} bytes: {}",
                    MAX_AUTHORING_FILE_BYTES,
                    path.display()
                ));
            }
            let content = fs::read_to_string(&path)
                .map_err(|err| format!("failed to read {label} '{}': {err}", path.display()))?;
            Ok(TemplateAuthoringFile {
                path: declared_path.clone(),
                content,
            })
        })
        .collect()
}

fn resolve_template_relative_path(
    template_dir: &Path,
    declared_path: &str,
    label: &str,
) -> Result<PathBuf, String> {
    let relative = Path::new(declared_path);
    if declared_path.trim().is_empty()
        || relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(format!(
            "{label} must be a non-empty path inside the template pack: {declared_path}"
        ));
    }
    let path = template_dir.join(relative);
    crate::validator::require_file(&path, label)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_declared_authoring_files_and_rejects_parent_paths() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-template-authoring-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(root.join("prompts"))
            .map_err(|err| format!("failed to create authoring test dir: {err}"))?;
        fs::write(
            root.join("prompts").join("rules.md"),
            "Use concise chapters.\n",
        )
        .map_err(|err| format!("failed to write authoring test file: {err}"))?;
        let mut input = crate::template_manifest::TemplateInputSpec::default();
        input.authoring = Some(crate::template_manifest::TemplateAuthoringSpec {
            instructions: vec!["prompts/rules.md".to_string()],
            examples: Vec::new(),
        });

        let context = load_template_authoring_context(&root, &input)?;
        assert_eq!(context.instructions.len(), 1);
        assert!(context.instructions[0].content.contains("concise chapters"));

        input.authoring = Some(crate::template_manifest::TemplateAuthoringSpec {
            instructions: vec!["../outside.md".to_string()],
            examples: Vec::new(),
        });
        let error = load_template_authoring_context(&root, &input)
            .expect_err("parent path should not be accepted");
        assert!(error.contains("inside the template pack"));
        fs::remove_dir_all(&root).ok();
        Ok(())
    }
}
