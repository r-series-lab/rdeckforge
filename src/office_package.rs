use roxmltree::Document;
use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};
use uuid::Uuid;
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficePackageIntegrity {
    pub status: &'static str,
    pub format: String,
    pub file_size: u64,
    pub entry_count: usize,
    pub relationship_count: usize,
    pub required_entries: Vec<String>,
}

pub struct AtomicOfficeOutput {
    format: String,
    final_path: PathBuf,
    transaction_dir: PathBuf,
    staging_path: PathBuf,
    committed: bool,
}

impl AtomicOfficeOutput {
    pub fn new(final_path: &Path, format: &str) -> Result<Self, String> {
        let format = normalized_format(format)?;
        let final_path = absolutize(final_path);
        let extension = final_path
            .extension()
            .and_then(|value| value.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default();
        if extension != format {
            return Err(format!(
                "output extension '.{extension}' does not match Office format '{format}'"
            ));
        }
        let file_name = final_path
            .file_name()
            .ok_or_else(|| "output file name is required".to_string())?;
        let parent = final_path
            .parent()
            .ok_or_else(|| "output parent directory is required".to_string())?;
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create output directory '{}': {err}",
                parent.display()
            )
        })?;
        if final_path.exists() && !final_path.is_file() {
            return Err(format!(
                "output path exists but is not a file: {}",
                final_path.display()
            ));
        }
        let transaction_dir = parent.join(format!(".rdeckforge-txn-{}", Uuid::new_v4()));
        fs::create_dir(&transaction_dir).map_err(|err| {
            format!(
                "failed to create output transaction directory '{}': {err}",
                transaction_dir.display()
            )
        })?;
        let staging_path = transaction_dir.join(file_name);
        Ok(Self {
            format,
            final_path,
            transaction_dir,
            staging_path,
            committed: false,
        })
    }

    pub fn staging_path(&self) -> &Path {
        &self.staging_path
    }

    pub fn final_path(&self) -> &Path {
        &self.final_path
    }

    pub fn validate(&self) -> Result<OfficePackageIntegrity, String> {
        validate_office_package(&self.staging_path, &self.format)
    }

    pub fn commit(&mut self) -> Result<(), String> {
        if self.committed {
            return Err("output transaction was already committed".to_string());
        }
        if !self.staging_path.is_file() {
            return Err(format!(
                "renderer did not create the staged output: {}",
                self.staging_path.display()
            ));
        }
        File::open(&self.staging_path)
            .and_then(|file| file.sync_all())
            .map_err(|err| format!("failed to flush staged Office output: {err}"))?;

        let backup_path = self.final_path.with_file_name(format!(
            ".{}.rdeckforge-backup-{}",
            self.final_path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("output"),
            Uuid::new_v4()
        ));
        let had_previous = self.final_path.exists();
        if had_previous {
            fs::rename(&self.final_path, &backup_path).map_err(|err| {
                format!(
                    "failed to preserve existing output '{}': {err}",
                    self.final_path.display()
                )
            })?;
        }
        if let Err(error) = fs::rename(&self.staging_path, &self.final_path) {
            if had_previous {
                let _ = fs::rename(&backup_path, &self.final_path);
            }
            return Err(format!(
                "failed to commit Office output '{}': {error}",
                self.final_path.display()
            ));
        }
        if had_previous {
            let _ = fs::remove_file(&backup_path);
        }
        self.committed = true;
        let _ = fs::remove_dir_all(&self.transaction_dir);
        Ok(())
    }
}

impl Drop for AtomicOfficeOutput {
    fn drop(&mut self) {
        if !self.committed {
            let _ = fs::remove_dir_all(&self.transaction_dir);
        }
    }
}

pub fn validate_office_package(
    path: &Path,
    format: &str,
) -> Result<OfficePackageIntegrity, String> {
    let format = normalized_format(format)?;
    let metadata = fs::metadata(path).map_err(|err| {
        format!(
            "failed to inspect Office output '{}': {err}",
            path.display()
        )
    })?;
    if metadata.len() == 0 {
        return Err(format!("Office output is empty: {}", path.display()));
    }
    let file = File::open(path)
        .map_err(|err| format!("failed to open Office output '{}': {err}", path.display()))?;
    let mut archive = ZipArchive::new(file)
        .map_err(|err| format!("Office output is not a readable ZIP package: {err}"))?;
    let mut entries = BTreeSet::new();
    let mut xml_parts = BTreeMap::new();

    for index in 0..archive.len() {
        let mut part = archive
            .by_index(index)
            .map_err(|err| format!("failed to read Office package entry {index}: {err}"))?;
        let name = normalize_package_name(part.name());
        if name.is_empty() || part.is_dir() {
            continue;
        }
        if !entries.insert(name.clone()) {
            return Err(format!("Office package contains a duplicate entry: {name}"));
        }
        let mut bytes = Vec::new();
        part.read_to_end(&mut bytes)
            .map_err(|err| format!("failed to verify Office package entry '{name}': {err}"))?;
        if name.ends_with(".xml") || name.ends_with(".rels") || name == "[Content_Types].xml" {
            let text = std::str::from_utf8(&bytes)
                .map_err(|err| format!("Office XML entry '{name}' is not UTF-8: {err}"))?
                .trim_start_matches('\u{feff}')
                .to_string();
            Document::parse(&text)
                .map_err(|err| format!("Office XML entry '{name}' is malformed: {err}"))?;
            xml_parts.insert(name, text);
        }
    }

    let required_entries = required_entries(&format, &entries)?;
    validate_content_types(&entries, &xml_parts)?;
    let relationship_count = validate_relationships(&entries, &xml_parts)?;

    Ok(OfficePackageIntegrity {
        status: "passed",
        format,
        file_size: metadata.len(),
        entry_count: entries.len(),
        relationship_count,
        required_entries,
    })
}

fn required_entries(format: &str, entries: &BTreeSet<String>) -> Result<Vec<String>, String> {
    let mut required = vec!["[Content_Types].xml", "_rels/.rels"];
    match format {
        "pptx" => required.extend(["ppt/presentation.xml", "ppt/_rels/presentation.xml.rels"]),
        "docx" => required.push("word/document.xml"),
        "xlsx" => required.extend(["xl/workbook.xml", "xl/_rels/workbook.xml.rels"]),
        _ => unreachable!(),
    }
    for name in &required {
        if !entries.contains(*name) {
            return Err(format!("Office package is missing required entry: {name}"));
        }
    }
    let has_primary_items = match format {
        "pptx" => entries
            .iter()
            .any(|name| name.starts_with("ppt/slides/slide") && name.ends_with(".xml")),
        "xlsx" => entries
            .iter()
            .any(|name| name.starts_with("xl/worksheets/") && name.ends_with(".xml")),
        "docx" => true,
        _ => false,
    };
    if !has_primary_items {
        return Err(format!(
            "Office package does not contain any primary {format} content parts"
        ));
    }
    Ok(required.into_iter().map(str::to_string).collect())
}

fn validate_content_types(
    entries: &BTreeSet<String>,
    xml_parts: &BTreeMap<String, String>,
) -> Result<(), String> {
    let content_types = xml_parts
        .get("[Content_Types].xml")
        .ok_or_else(|| "Office package content types XML was not loaded".to_string())?;
    let document = Document::parse(content_types)
        .map_err(|err| format!("Office content types XML is malformed: {err}"))?;
    for node in document
        .descendants()
        .filter(|node| node.is_element() && node.tag_name().name() == "Override")
    {
        let Some(part_name) = node.attribute("PartName") else {
            continue;
        };
        let normalized = normalize_package_name(&percent_decode(part_name.trim_start_matches('/')));
        if !normalized.is_empty() && !entries.contains(&normalized) {
            return Err(format!(
                "Office content types references a missing part: {normalized}"
            ));
        }
    }
    Ok(())
}

fn validate_relationships(
    entries: &BTreeSet<String>,
    xml_parts: &BTreeMap<String, String>,
) -> Result<usize, String> {
    let mut relationship_count = 0;
    for (name, xml) in xml_parts.iter().filter(|(name, _)| name.ends_with(".rels")) {
        let document = Document::parse(xml)
            .map_err(|err| format!("Office relationships entry '{name}' is malformed: {err}"))?;
        for node in document
            .descendants()
            .filter(|node| node.is_element() && node.tag_name().name() == "Relationship")
        {
            relationship_count += 1;
            if node
                .attribute("TargetMode")
                .is_some_and(|mode| mode.eq_ignore_ascii_case("external"))
            {
                continue;
            }
            let Some(target) = node.attribute("Target") else {
                return Err(format!("Office relationship in '{name}' has no Target"));
            };
            if target.starts_with('#') || target.contains("://") || target.starts_with("mailto:") {
                continue;
            }
            let resolved = resolve_relationship_target(name, target)?;
            if !entries.contains(&resolved) {
                return Err(format!(
                    "Office relationship in '{name}' references a missing part: {resolved}"
                ));
            }
        }
    }
    Ok(relationship_count)
}

fn resolve_relationship_target(rels_name: &str, target: &str) -> Result<String, String> {
    let target = target.split('#').next().unwrap_or_default();
    let target = percent_decode(target).replace('\\', "/");
    let mut parts = if target.starts_with('/') || rels_name == "_rels/.rels" {
        Vec::new()
    } else {
        let parent = rels_name
            .strip_suffix(".rels")
            .and_then(|name| name.rsplit_once("/_rels/").map(|(parent, _)| parent))
            .unwrap_or_default();
        parent
            .split('/')
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    for part in target.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(format!(
                        "Office relationship target escapes package root: {target}"
                    ));
                }
            }
            value => parts.push(value.to_string()),
        }
    }
    Ok(parts.join("/"))
}

fn normalize_package_name(name: &str) -> String {
    name.trim_start_matches('/').replace('\\', "/")
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                output.push(high * 16 + low);
                index += 3;
                continue;
            }
        }
        output.push(bytes[index]);
        index += 1;
    }
    String::from_utf8_lossy(&output).to_string()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn normalized_format(format: &str) -> Result<String, String> {
    let format = format.trim().to_ascii_lowercase();
    if matches!(format.as_str(), "pptx" | "docx" | "xlsx") {
        Ok(format)
    } else {
        Err(format!("unsupported Office package format: {format}"))
    }
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
    use std::io::Write;
    use zip::{CompressionMethod, ZipWriter, write::FileOptions};

    #[test]
    fn validates_minimal_pptx_relationships() -> Result<(), String> {
        let root = test_root();
        let output = root.join("valid.pptx");
        write_package(&output, false)?;
        let report = validate_office_package(&output, "pptx")?;
        assert_eq!(report.status, "passed");
        assert_eq!(report.relationship_count, 2);
        fs::remove_dir_all(root).ok();
        Ok(())
    }

    #[test]
    fn rejects_missing_relationship_targets() -> Result<(), String> {
        let root = test_root();
        let output = root.join("broken.pptx");
        write_package(&output, true)?;
        let error = validate_office_package(&output, "pptx").unwrap_err();
        assert!(error.contains("missing part"));
        fs::remove_dir_all(root).ok();
        Ok(())
    }

    #[test]
    fn failed_staging_keeps_existing_output() -> Result<(), String> {
        let root = test_root();
        let output = root.join("existing.pptx");
        fs::write(&output, b"existing output")
            .map_err(|err| format!("failed to write existing output: {err}"))?;
        {
            let transaction = AtomicOfficeOutput::new(&output, "pptx")?;
            fs::write(transaction.staging_path(), b"not a zip")
                .map_err(|err| format!("failed to write staged output: {err}"))?;
            assert!(transaction.validate().is_err());
        }
        assert_eq!(
            fs::read(&output).map_err(|err| format!("failed to read existing output: {err}"))?,
            b"existing output"
        );
        fs::remove_dir_all(root).ok();
        Ok(())
    }

    fn write_package(path: &Path, broken_relationship: bool) -> Result<(), String> {
        let file =
            File::create(path).map_err(|err| format!("failed to create package fixture: {err}"))?;
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        let entries = [
            (
                "[Content_Types].xml",
                r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Override PartName="/ppt/presentation.xml" ContentType="application/xml"/><Override PartName="/ppt/slides/slide1.xml" ContentType="application/xml"/></Types>"#,
            ),
            (
                "_rels/.rels",
                r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="officeDocument" Target="ppt/presentation.xml"/></Relationships>"#,
            ),
            (
                "ppt/presentation.xml",
                r#"<?xml version="1.0"?><p:presentation xmlns:p="p"/>"#,
            ),
            (
                "ppt/_rels/presentation.xml.rels",
                if broken_relationship {
                    r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="slide" Target="slides/missing.xml"/></Relationships>"#
                } else {
                    r#"<?xml version="1.0"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="slide" Target="slides/slide1.xml"/></Relationships>"#
                },
            ),
            (
                "ppt/slides/slide1.xml",
                r#"<?xml version="1.0"?><p:sld xmlns:p="p"/>"#,
            ),
        ];
        for (name, content) in entries {
            zip.start_file(name, options)
                .map_err(|err| format!("failed to start fixture entry: {err}"))?;
            zip.write_all(content.as_bytes())
                .map_err(|err| format!("failed to write fixture entry: {err}"))?;
        }
        zip.finish()
            .map_err(|err| format!("failed to finish package fixture: {err}"))?;
        Ok(())
    }

    fn test_root() -> PathBuf {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-office-package-{}", Uuid::new_v4()));
        fs::create_dir_all(&root).expect("create office package test root");
        root
    }
}
