use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputPathResolution {
    pub requested_path: String,
    pub resolved_path: String,
    pub conflict_policy: String,
    pub renamed: bool,
}

pub fn resolve_output_path(
    requested_path: &Path,
    conflict_policy: &str,
) -> Result<OutputPathResolution, String> {
    if requested_path.as_os_str().is_empty() {
        return Err("output path is required".to_string());
    }
    let policy = match conflict_policy.trim() {
        "" | "increment" => "increment",
        "overwrite" => "overwrite",
        other => {
            return Err(format!(
                "unsupported output conflict policy '{other}'; expected increment or overwrite"
            ));
        }
    };
    let requested = requested_path.to_path_buf();
    let resolved = if policy == "overwrite" || !requested.exists() {
        requested.clone()
    } else {
        incremented_path(&requested)?
    };
    Ok(OutputPathResolution {
        requested_path: requested.display().to_string(),
        resolved_path: resolved.display().to_string(),
        conflict_policy: policy.to_string(),
        renamed: resolved != requested,
    })
}

fn incremented_path(path: &Path) -> Result<PathBuf, String> {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("output path has no usable file name: {}", path.display()))?;
    let extension = path.extension().and_then(|value| value.to_str());
    for index in 2..=9999 {
        let file_name = match extension {
            Some(extension) if !extension.is_empty() => format!("{stem}-{index}.{extension}"),
            _ => format!("{stem}-{index}"),
        };
        let candidate = parent.join(file_name);
        if !candidate.exists() {
            return Ok(candidate);
        }
    }
    Err(format!(
        "could not find an available numbered output path for {}",
        path.display()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn increment_policy_keeps_new_paths_and_numbers_conflicts() {
        let root = std::env::temp_dir().join(format!("rdeckforge-output-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let output = root.join("demo.pptx");
        let first = resolve_output_path(&output, "increment").unwrap();
        assert_eq!(first.resolved_path, output.display().to_string());
        assert!(!first.renamed);

        fs::write(&output, b"existing").unwrap();
        let second = resolve_output_path(&output, "increment").unwrap();
        assert_eq!(
            second.resolved_path,
            root.join("demo-2.pptx").display().to_string()
        );
        assert!(second.renamed);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn overwrite_policy_preserves_the_requested_path() {
        let output = Path::new("/tmp/existing.docx");
        let result = resolve_output_path(output, "overwrite").unwrap();
        assert_eq!(result.resolved_path, output.display().to_string());
        assert!(!result.renamed);
    }
}
