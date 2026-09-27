use serde::Serialize;
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::FileOptions};

const MAX_ARCHIVE_FILES: usize = 20_000;
const MAX_UNCOMPRESSED_BYTES: u64 = 512 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateArchiveExportResult {
    pub archive_file: String,
    pub template_id: String,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateArchiveImportResult {
    pub archive_file: String,
    pub pack_dir: String,
    pub template_id: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub validation: crate::template_manifest::TemplateValidation,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedTemplateArchiveImport {
    pub archive_file: String,
    pub pack_dir: String,
    pub file_count: usize,
    pub total_bytes: u64,
    pub template: crate::storage::TemplatePackRecord,
}

pub fn export_template_pack_archive(
    template_dir: &Path,
    output: &Path,
    force: bool,
) -> Result<TemplateArchiveExportResult, String> {
    let template_dir = normalize_existing_dir(template_dir, "template pack")?;
    let validation = crate::template_manifest::validate_template_pack(&template_dir)?;
    let output = absolute_path(output)?;
    if output.exists() && !force {
        return Err(format!(
            "archive already exists: {}; use force to replace it",
            output.display()
        ));
    }
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create '{}': {err}", parent.display()))?;
    }

    let mut entries = Vec::new();
    collect_files(&template_dir, &template_dir, &mut entries)?;
    entries.sort_by(|left, right| left.1.cmp(&right.1));
    if entries.len() > MAX_ARCHIVE_FILES {
        return Err(format!(
            "template pack contains too many files: {} (maximum {MAX_ARCHIVE_FILES})",
            entries.len()
        ));
    }

    let temporary = sibling_temporary_path(&output, "export");
    let result = (|| {
        let file = File::create(&temporary)
            .map_err(|err| format!("failed to create '{}': {err}", temporary.display()))?;
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o644);
        let mut file_count = 0usize;
        let mut total_bytes = 0u64;

        for (source, relative) in &entries {
            if source == &output || source == &temporary {
                continue;
            }
            let metadata = fs::symlink_metadata(source)
                .map_err(|err| format!("failed to inspect '{}': {err}", source.display()))?;
            if metadata.file_type().is_symlink() {
                return Err(format!(
                    "template archives do not include symbolic links: {}",
                    source.display()
                ));
            }
            total_bytes = total_bytes.saturating_add(metadata.len());
            if total_bytes > MAX_UNCOMPRESSED_BYTES {
                return Err(format!(
                    "template pack is too large after extraction (maximum {} MiB)",
                    MAX_UNCOMPRESSED_BYTES / 1024 / 1024
                ));
            }
            let archive_name = zip_path(relative)?;
            writer
                .start_file(archive_name, options)
                .map_err(|err| format!("failed to add '{}': {err}", source.display()))?;
            let mut input = File::open(source)
                .map_err(|err| format!("failed to open '{}': {err}", source.display()))?;
            std::io::copy(&mut input, &mut writer)
                .map_err(|err| format!("failed to archive '{}': {err}", source.display()))?;
            file_count += 1;
        }
        writer
            .finish()
            .map_err(|err| format!("failed to finish '{}': {err}", temporary.display()))?;
        Ok((file_count, total_bytes))
    })();

    let (file_count, total_bytes) = match result {
        Ok(result) => result,
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
    };
    if output.exists() {
        fs::remove_file(&output)
            .map_err(|err| format!("failed to replace '{}': {err}", output.display()))?;
    }
    fs::rename(&temporary, &output).map_err(|err| {
        format!(
            "failed to move archive '{}' to '{}': {err}",
            temporary.display(),
            output.display()
        )
    })?;

    Ok(TemplateArchiveExportResult {
        archive_file: output.display().to_string(),
        template_id: validation.template_id,
        file_count,
        total_bytes,
    })
}

pub fn import_template_pack_archive(
    archive_file: &Path,
    output_root: &Path,
    force: bool,
) -> Result<TemplateArchiveImportResult, String> {
    let archive_file = normalize_existing_file(archive_file, "template archive")?;
    let output_root = absolute_path(output_root)?;
    fs::create_dir_all(&output_root)
        .map_err(|err| format!("failed to create '{}': {err}", output_root.display()))?;
    let temporary = output_root.join(format!(".rdeckforge-import-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&temporary)
        .map_err(|err| format!("failed to create '{}': {err}", temporary.display()))?;

    let extraction = extract_archive(&archive_file, &temporary);
    let (file_count, total_bytes) = match extraction {
        Ok(result) => result,
        Err(error) => {
            let _ = fs::remove_dir_all(&temporary);
            return Err(error);
        }
    };
    let validation = match crate::template_manifest::validate_template_pack(&temporary) {
        Ok(validation) => validation,
        Err(error) => {
            let _ = fs::remove_dir_all(&temporary);
            return Err(format!("imported template pack failed validation: {error}"));
        }
    };
    let directory_name = safe_directory_name(&validation.template_id);
    let destination = output_root.join(directory_name);
    if destination.exists() {
        if !force {
            let _ = fs::remove_dir_all(&temporary);
            return Err(format!(
                "template destination already exists: {}; use force to replace it",
                destination.display()
            ));
        }
        fs::remove_dir_all(&destination)
            .map_err(|err| format!("failed to replace '{}': {err}", destination.display()))?;
    }
    fs::rename(&temporary, &destination).map_err(|err| {
        let _ = fs::remove_dir_all(&temporary);
        format!(
            "failed to move imported template to '{}': {err}",
            destination.display()
        )
    })?;
    let validation = crate::template_manifest::validate_template_pack(&destination)?;

    Ok(TemplateArchiveImportResult {
        archive_file: archive_file.display().to_string(),
        pack_dir: destination.display().to_string(),
        template_id: validation.template_id.clone(),
        file_count,
        total_bytes,
        validation,
    })
}

pub fn import_and_link_template_pack_archive(
    archive_file: &Path,
    output_root: &Path,
    force: bool,
) -> Result<LinkedTemplateArchiveImport, String> {
    let imported = import_template_pack_archive(archive_file, output_root, force)?;
    let template = crate::storage::link_template_pack(Path::new(&imported.pack_dir))?;
    Ok(LinkedTemplateArchiveImport {
        archive_file: imported.archive_file,
        pack_dir: imported.pack_dir,
        file_count: imported.file_count,
        total_bytes: imported.total_bytes,
        template,
    })
}

fn extract_archive(archive_file: &Path, output: &Path) -> Result<(usize, u64), String> {
    let file = File::open(archive_file)
        .map_err(|err| format!("failed to open '{}': {err}", archive_file.display()))?;
    let mut archive = ZipArchive::new(file).map_err(|err| {
        format!(
            "invalid template archive '{}': {err}",
            archive_file.display()
        )
    })?;
    if archive.len() > MAX_ARCHIVE_FILES {
        return Err(format!(
            "template archive contains too many entries: {} (maximum {MAX_ARCHIVE_FILES})",
            archive.len()
        ));
    }

    let mut manifest_paths = Vec::new();
    for index in 0..archive.len() {
        let entry = archive
            .by_index(index)
            .map_err(|err| format!("failed to inspect archive entry {index}: {err}"))?;
        let path = entry
            .enclosed_name()
            .ok_or_else(|| format!("unsafe archive entry path: {}", entry.name()))?
            .to_path_buf();
        if path.file_name().and_then(|name| name.to_str()) == Some("template.manifest.json") {
            manifest_paths.push(path);
        }
    }
    if manifest_paths.len() != 1 {
        return Err(format!(
            "template archive must contain exactly one template.manifest.json; found {}",
            manifest_paths.len()
        ));
    }
    let pack_prefix = manifest_paths[0].parent().unwrap_or_else(|| Path::new(""));
    let mut file_count = 0usize;
    let mut total_bytes = 0u64;

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| format!("failed to read archive entry {index}: {err}"))?;
        let enclosed = entry
            .enclosed_name()
            .ok_or_else(|| format!("unsafe archive entry path: {}", entry.name()))?
            .to_path_buf();
        if enclosed.starts_with("__MACOSX") {
            continue;
        }
        let relative = enclosed.strip_prefix(pack_prefix).map_err(|_| {
            format!(
                "archive entry is outside the template pack root: {}",
                enclosed.display()
            )
        })?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        if entry
            .unix_mode()
            .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err(format!(
                "template archives cannot contain symbolic links: {}",
                enclosed.display()
            ));
        }
        total_bytes = total_bytes.saturating_add(entry.size());
        if total_bytes > MAX_UNCOMPRESSED_BYTES {
            return Err(format!(
                "template archive is too large after extraction (maximum {} MiB)",
                MAX_UNCOMPRESSED_BYTES / 1024 / 1024
            ));
        }
        let destination = output.join(relative);
        if entry.is_dir() {
            fs::create_dir_all(&destination)
                .map_err(|err| format!("failed to create '{}': {err}", destination.display()))?;
            continue;
        }
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create '{}': {err}", parent.display()))?;
        }
        let mut target = File::create(&destination)
            .map_err(|err| format!("failed to create '{}': {err}", destination.display()))?;
        std::io::copy(&mut entry, &mut target)
            .map_err(|err| format!("failed to extract '{}': {err}", destination.display()))?;
        target
            .flush()
            .map_err(|err| format!("failed to finish '{}': {err}", destination.display()))?;
        file_count += 1;
    }
    Ok((file_count, total_bytes))
}

fn collect_files(
    root: &Path,
    directory: &Path,
    entries: &mut Vec<(PathBuf, PathBuf)>,
) -> Result<(), String> {
    let mut children = fs::read_dir(directory)
        .map_err(|err| format!("failed to read '{}': {err}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| format!("failed to read '{}': {err}", directory.display()))?;
    children.sort_by_key(|entry| entry.file_name());
    for child in children {
        let path = child.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|err| format!("failed to inspect '{}': {err}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "template archives do not include symbolic links: {}",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_files(root, &path, entries)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .map_err(|_| format!("failed to relativize '{}': not under root", path.display()))?
                .to_path_buf();
            entries.push((path, relative));
        }
    }
    Ok(())
}

fn zip_path(path: &Path) -> Result<String, String> {
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            std::path::Component::Normal(value) => parts.push(value.to_string_lossy()),
            _ => return Err(format!("unsafe template archive path: {}", path.display())),
        }
    }
    Ok(parts.join("/"))
}

fn safe_directory_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if sanitized.is_empty() {
        "imported-template".to_string()
    } else {
        sanitized
    }
}

fn sibling_temporary_path(path: &Path, label: &str) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("template.rdeckpack");
    path.with_file_name(format!(".{file_name}.{label}-{}", uuid::Uuid::new_v4()))
}

fn normalize_existing_dir(path: &Path, label: &str) -> Result<PathBuf, String> {
    let path = absolute_path(path)?;
    crate::validator::require_dir(&path, label)?;
    path.canonicalize()
        .map_err(|err| format!("failed to normalize '{}': {err}", path.display()))
}

fn normalize_existing_file(path: &Path, label: &str) -> Result<PathBuf, String> {
    let path = absolute_path(path)?;
    crate::validator::require_file(&path, label)?;
    path.canonicalize()
        .map_err(|err| format!("failed to normalize '{}': {err}", path.display()))
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|directory| directory.join(path))
            .map_err(|err| format!("failed to resolve current directory: {err}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn archive_round_trip_preserves_a_script_template_pack() {
        let root = test_root("round-trip");
        let source = root.join("source");
        let imports = root.join("imports");
        fs::create_dir_all(source.join("renderer")).unwrap();
        fs::write(
            source.join("template.manifest.json"),
            r#"{
              "schemaVersion": "1.0",
              "templateId": "portable-script-template",
              "name": "Portable Script Template",
              "format": "pptx",
              "templateType": "script",
              "input": { "formats": ["json"] },
              "renderer": {
                "type": "script.python",
                "runtime": "python3",
                "entry": "renderer/build.py"
              }
            }"#,
        )
        .unwrap();
        fs::write(source.join("renderer/build.py"), "print('ok')\n").unwrap();
        let archive = root.join("portable.rdeckpack");

        let exported = export_template_pack_archive(&source, &archive, false).unwrap();
        assert_eq!(exported.file_count, 2);
        let imported = import_template_pack_archive(&archive, &imports, false).unwrap();
        assert_eq!(imported.template_id, "portable-script-template");
        assert!(
            Path::new(&imported.pack_dir)
                .join("renderer/build.py")
                .is_file()
        );
        assert_eq!(imported.file_count, 2);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn archive_import_rejects_parent_directory_entries() {
        let root = test_root("unsafe-entry");
        fs::create_dir_all(&root).unwrap();
        let archive_path = root.join("unsafe.rdeckpack");
        let file = File::create(&archive_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Stored);
        writer
            .start_file("template.manifest.json", options)
            .unwrap();
        writer.write_all(b"{}").unwrap();
        writer.start_file("../outside.txt", options).unwrap();
        writer.write_all(b"bad").unwrap();
        writer.finish().unwrap();

        let error =
            import_template_pack_archive(&archive_path, &root.join("imports"), false).unwrap_err();
        assert!(error.contains("unsafe archive entry path"));
        assert!(!root.join("outside.txt").exists());

        fs::remove_dir_all(root).unwrap();
    }

    fn test_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rdeckforge-template-archive-{label}-{}",
            uuid::Uuid::new_v4()
        ))
    }
}
