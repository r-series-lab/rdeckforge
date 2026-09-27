use serde::Serialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::Path,
};
use zip::ZipArchive;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateHealth {
    pub status: String,
    pub checked: Vec<String>,
    pub warnings: Vec<String>,
}

impl TemplateHealth {
    fn new() -> Self {
        Self {
            status: "unchecked".to_string(),
            checked: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn unchecked() -> Self {
        Self::new()
    }

    fn finish(mut self) -> Self {
        self.status = if self.checked.is_empty() {
            "unchecked".to_string()
        } else if self.warnings.is_empty() {
            "ok".to_string()
        } else {
            "warning".to_string()
        };
        self
    }

    fn check(&mut self, name: &str) {
        self.checked.push(name.to_string());
    }

    fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }
}

pub fn inspect_template_pack(
    manifest: &crate::template_manifest::TemplateManifest,
    entry_path: &Path,
) -> TemplateHealth {
    let mut health = TemplateHealth::new();
    match manifest.format.as_str() {
        "pptx" => inspect_pptx(manifest, entry_path, &mut health),
        "docx" => inspect_docx(manifest, entry_path, &mut health),
        "xlsx" => inspect_xlsx(manifest, entry_path, &mut health),
        _ => {}
    }
    health.finish()
}

fn inspect_pptx(
    manifest: &crate::template_manifest::TemplateManifest,
    entry_path: &Path,
    health: &mut TemplateHealth,
) {
    health.check("pptx_source_slides");
    health.check("pptx_shape_names");
    let Ok(mut archive) = open_archive(entry_path, "PPTX", health) else {
        return;
    };
    let mut slide_shapes = BTreeMap::new();

    for (template_id, page_template) in &manifest.page_templates {
        let slide_entry = format!("ppt/slides/slide{}.xml", page_template.source_slide);
        let shape_names = if let Some(names) = slide_shapes.get(&page_template.source_slide) {
            names
        } else {
            let names = match read_zip_text(&mut archive, &slide_entry) {
                Ok(slide_xml) => extract_tag_attr_values(&slide_xml, "p:cNvPr", "name"),
                Err(err) => {
                    health.warn(format!(
                        "PPTX pageTemplate '{template_id}' sourceSlide {} could not be inspected: {err}",
                        page_template.source_slide
                    ));
                    BTreeSet::new()
                }
            };
            slide_shapes.insert(page_template.source_slide, names);
            slide_shapes.get(&page_template.source_slide).unwrap()
        };

        for (binding_id, binding) in &page_template.bindings {
            if let Some(shape_name) = binding.shape_name.as_deref() {
                if !shape_names.contains(shape_name) {
                    health.warn(format!(
                        "PPTX pageTemplate '{template_id}' binding '{binding_id}' shapeName '{shape_name}' was not found on sourceSlide {}",
                        page_template.source_slide
                    ));
                }
            } else if binding.creation_id.is_none() {
                health.warn(format!(
                    "PPTX pageTemplate '{template_id}' binding '{binding_id}' has no shapeName or creationId"
                ));
            }
        }
    }
}

fn inspect_docx(
    manifest: &crate::template_manifest::TemplateManifest,
    entry_path: &Path,
    health: &mut TemplateHealth,
) {
    health.check("docx_placeholders");
    let Ok(mut archive) = open_archive(entry_path, "DOCX", health) else {
        return;
    };
    let document = match read_zip_text(&mut archive, "word/document.xml") {
        Ok(document) => document,
        Err(err) => {
            health.warn(format!("DOCX document XML could not be inspected: {err}"));
            return;
        }
    };

    for (block_id, block) in &manifest.block_templates {
        for (binding_id, binding) in &block.bindings {
            if let Some(placeholder) = binding.placeholder.as_deref() {
                if !document.contains(placeholder) {
                    health.warn(format!(
                        "DOCX blockTemplate '{block_id}' binding '{binding_id}' placeholder '{placeholder}' was not found in word/document.xml"
                    ));
                }
                continue;
            }
            if binding.bookmark.is_some() || binding.content_control.is_some() {
                health.warn(format!(
                    "DOCX blockTemplate '{block_id}' binding '{binding_id}' uses bookmark/contentControl; placeholder health checks are implemented first"
                ));
            } else {
                health.warn(format!(
                    "DOCX blockTemplate '{block_id}' binding '{binding_id}' has no placeholder, bookmark, or contentControl"
                ));
            }
        }
    }
}

fn inspect_xlsx(
    manifest: &crate::template_manifest::TemplateManifest,
    entry_path: &Path,
    health: &mut TemplateHealth,
) {
    health.check("xlsx_sheets");
    health.check("xlsx_named_ranges");
    health.check("xlsx_cells");
    let Ok(mut archive) = open_archive(entry_path, "XLSX", health) else {
        return;
    };
    let workbook = match read_zip_text(&mut archive, "xl/workbook.xml") {
        Ok(workbook) => workbook,
        Err(err) => {
            health.warn(format!("XLSX workbook XML could not be inspected: {err}"));
            return;
        }
    };
    let sheet_names = extract_tag_attr_values(&workbook, "sheet", "name");
    let named_ranges = extract_tag_attr_values(&workbook, "definedName", "name");

    for (template_id, sheet_template) in &manifest.sheet_templates {
        if let Some(source_sheet) = sheet_template.source_sheet.as_deref() {
            if !sheet_names.contains(source_sheet) {
                health.warn(format!(
                    "XLSX sheetTemplate '{template_id}' sourceSheet '{source_sheet}' was not found in workbook sheets"
                ));
            }
        }
        if let Some(source_sheet_index) = sheet_template.source_sheet_index {
            if source_sheet_index == 0 || source_sheet_index as usize > sheet_names.len() {
                health.warn(format!(
                    "XLSX sheetTemplate '{template_id}' sourceSheetIndex {source_sheet_index} is outside workbook sheet count {}",
                    sheet_names.len()
                ));
            }
        }

        for (binding_id, binding) in &sheet_template.bindings {
            if let Some(named_range) = binding.named_range.as_deref() {
                if !named_ranges.contains(named_range) {
                    health.warn(format!(
                        "XLSX sheetTemplate '{template_id}' binding '{binding_id}' namedRange '{named_range}' was not found in workbook definedNames"
                    ));
                }
                continue;
            }
            if let Some(cell) = binding.cell.as_deref() {
                if !is_valid_cell_ref(cell) {
                    health.warn(format!(
                        "XLSX sheetTemplate '{template_id}' binding '{binding_id}' cell '{cell}' is not a valid single-cell A1 reference"
                    ));
                }
            } else {
                health.warn(format!(
                    "XLSX sheetTemplate '{template_id}' binding '{binding_id}' has neither cell nor namedRange"
                ));
            }
        }
    }
}

fn open_archive(
    path: &Path,
    label: &str,
    health: &mut TemplateHealth,
) -> Result<ZipArchive<File>, ()> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) => {
            health.warn(format!(
                "{label} template entry could not be opened '{}': {err}",
                path.display()
            ));
            return Err(());
        }
    };
    ZipArchive::new(file).map_err(|err| {
        health.warn(format!(
            "{label} template entry is not a readable Office zip '{}': {err}",
            path.display()
        ));
    })
}

fn read_zip_text(archive: &mut ZipArchive<File>, entry_path: &str) -> Result<String, String> {
    let mut entry = archive
        .by_name(entry_path)
        .map_err(|err| format!("{entry_path}: {err}"))?;
    let mut text = String::new();
    entry
        .read_to_string(&mut text)
        .map_err(|err| format!("{entry_path}: {err}"))?;
    Ok(text)
}

fn extract_tag_attr_values(raw: &str, tag: &str, attr: &str) -> BTreeSet<String> {
    let mut values = BTreeSet::new();
    let mut cursor = 0;
    while let Some(offset) = raw[cursor..].find('<') {
        let start = cursor + offset;
        let Some(close_offset) = raw[start..].find('>') else {
            break;
        };
        let tag_text = &raw[start..start + close_offset + 1];
        if open_tag_name(tag_text)
            .is_some_and(|name| name == tag || name.rsplit(':').next() == Some(tag))
        {
            if let Some(value) = extract_attr_value(tag_text, attr) {
                values.insert(xml_unescape(&value));
            }
        }
        cursor = start + close_offset + 1;
    }
    values
}

fn open_tag_name(tag: &str) -> Option<&str> {
    let value = tag.trim().strip_prefix('<')?;
    if value.starts_with('/') || value.starts_with('!') || value.starts_with('?') {
        return None;
    }
    let end = value
        .find(|ch: char| ch.is_whitespace() || ch == '/' || ch == '>')
        .unwrap_or(value.len());
    Some(&value[..end])
}

fn extract_attr_value(raw: &str, attr: &str) -> Option<String> {
    for quote in ['"', '\''] {
        let pattern = format!(r#" {attr}={quote}"#);
        if let Some(start_offset) = raw.find(&pattern) {
            let value_start = start_offset + pattern.len();
            let value_end = raw[value_start..].find(quote)? + value_start;
            return Some(raw[value_start..value_end].to_string());
        }
    }
    None
}

fn is_valid_cell_ref(cell: &str) -> bool {
    let normalized = cell.trim().replace('$', "");
    let mut chars = normalized.chars().peekable();
    let mut has_letters = false;
    while chars
        .peek()
        .is_some_and(|value| value.is_ascii_alphabetic())
    {
        has_letters = true;
        chars.next();
    }
    let digits = chars.collect::<String>();
    has_letters
        && !digits.is_empty()
        && digits.chars().all(|value| value.is_ascii_digit())
        && digits.parse::<usize>().is_ok_and(|row| row > 0)
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, fs, io::Write};
    use zip::{CompressionMethod, ZipWriter, write::FileOptions};

    #[test]
    fn detects_missing_pptx_shape_names() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-health-pptx-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).map_err(|err| format!("failed to create test dir: {err}"))?;
        let entry = root.join("template.pptx");
        write_zip(
            &entry,
            &[(
                "ppt/slides/slide1.xml",
                r#"<p:sld><p:cSld><p:spTree><p:sp><p:nvSpPr><p:cNvPr id="2" name="ph:title"/></p:nvSpPr></p:sp></p:spTree></p:cSld></p:sld>"#,
            )],
        )?;

        let mut page_templates = BTreeMap::new();
        page_templates.insert(
            "cover".to_string(),
            crate::template_manifest::PageTemplate {
                source_slide: 1,
                description: None,
                bindings: BTreeMap::from([
                    ("title".to_string(), page_binding(Some("ph:title"), None)),
                    (
                        "subtitle".to_string(),
                        page_binding(Some("ph:subtitle"), None),
                    ),
                ]),
                constraints: BTreeMap::new(),
            },
        );
        let manifest = manifest_with_format("pptx", "template.pptx", page_templates);

        let health = inspect_template_pack(&manifest, &entry);

        assert_eq!(health.status, "warning");
        assert!(health.checked.contains(&"pptx_shape_names".to_string()));
        assert_eq!(health.warnings.len(), 1);
        assert!(health.warnings[0].contains("ph:subtitle"));
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn detects_missing_docx_placeholders() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-health-docx-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).map_err(|err| format!("failed to create test dir: {err}"))?;
        let entry = root.join("template.docx");
        write_zip(
            &entry,
            &[(
                "word/document.xml",
                r#"<w:document><w:body><w:p><w:r><w:t>{{title}}</w:t></w:r></w:p></w:body></w:document>"#,
            )],
        )?;

        let mut block_templates = BTreeMap::new();
        block_templates.insert(
            "summary".to_string(),
            crate::template_manifest::BlockTemplate {
                description: None,
                bindings: BTreeMap::from([
                    ("title".to_string(), document_binding(Some("{{title}}"))),
                    ("goals".to_string(), document_binding(Some("{{goals}}"))),
                ]),
                constraints: BTreeMap::new(),
            },
        );
        let mut manifest = empty_manifest("docx", "template.docx");
        manifest.block_templates = block_templates;

        let health = inspect_template_pack(&manifest, &entry);

        assert_eq!(health.status, "warning");
        assert!(health.checked.contains(&"docx_placeholders".to_string()));
        assert_eq!(health.warnings.len(), 1);
        assert!(health.warnings[0].contains("{{goals}}"));
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn detects_missing_xlsx_named_ranges() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-health-xlsx-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).map_err(|err| format!("failed to create test dir: {err}"))?;
        let entry = root.join("template.xlsx");
        write_zip(
            &entry,
            &[(
                "xl/workbook.xml",
                r#"<x:workbook xmlns:x="urn:test"><x:sheets><x:sheet name="Report" sheetId="1"/></x:sheets><x:definedNames><x:definedName name="TitleCell">'Report'!$B$2</x:definedName></x:definedNames></x:workbook>"#,
            )],
        )?;

        let mut sheet_templates = BTreeMap::new();
        sheet_templates.insert(
            "report".to_string(),
            crate::template_manifest::SheetTemplate {
                source_sheet: Some("Report".to_string()),
                source_sheet_index: None,
                output_sheet_name: None,
                bindings: BTreeMap::from([
                    ("title".to_string(), sheet_binding(None, Some("TitleCell"))),
                    (
                        "table".to_string(),
                        sheet_binding(None, Some("MissingRange")),
                    ),
                    ("badCell".to_string(), sheet_binding(Some("12A"), None)),
                ]),
                constraints: BTreeMap::new(),
            },
        );
        let mut manifest = empty_manifest("xlsx", "template.xlsx");
        manifest.sheet_templates = sheet_templates;

        let health = inspect_template_pack(&manifest, &entry);

        assert_eq!(health.status, "warning");
        assert!(health.checked.contains(&"xlsx_named_ranges".to_string()));
        assert_eq!(health.warnings.len(), 2);
        assert!(
            health
                .warnings
                .iter()
                .any(|warning| warning.contains("MissingRange"))
        );
        assert!(
            health
                .warnings
                .iter()
                .any(|warning| warning.contains("12A"))
        );
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    fn write_zip(path: &Path, entries: &[(&str, &str)]) -> Result<(), String> {
        let file = File::create(path)
            .map_err(|err| format!("failed to create zip '{}': {err}", path.display()))?;
        let mut zip = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Stored);
        for (name, content) in entries {
            zip.start_file(*name, options)
                .map_err(|err| format!("failed to start zip entry {name}: {err}"))?;
            zip.write_all(content.as_bytes())
                .map_err(|err| format!("failed to write zip entry {name}: {err}"))?;
        }
        zip.finish()
            .map_err(|err| format!("failed to finish zip '{}': {err}", path.display()))?;
        Ok(())
    }

    fn manifest_with_format(
        format: &str,
        entry: &str,
        page_templates: BTreeMap<String, crate::template_manifest::PageTemplate>,
    ) -> crate::template_manifest::TemplateManifest {
        let mut manifest = empty_manifest(format, entry);
        manifest.page_templates = page_templates;
        manifest
    }

    fn empty_manifest(format: &str, entry: &str) -> crate::template_manifest::TemplateManifest {
        crate::template_manifest::TemplateManifest {
            schema_version: "1.0".to_string(),
            template_id: "health-test".to_string(),
            name: "Health Test".to_string(),
            family_id: None,
            role: None,
            format: format.to_string(),
            template_type: Some("declarative".to_string()),
            entry: Some(entry.to_string()),
            content_schema: None,
            input: None,
            preview: None,
            render_mode: None,
            renderer: None,
            pipeline: Vec::new(),
            dependencies: crate::template_manifest::TemplateDependencySpec::default(),
            test_cases: Vec::new(),
            page_templates: BTreeMap::new(),
            deck_recipes: BTreeMap::new(),
            block_templates: BTreeMap::new(),
            document_recipes: BTreeMap::new(),
            sheet_templates: BTreeMap::new(),
            workbook_recipes: BTreeMap::new(),
            layouts: BTreeMap::new(),
        }
    }

    fn page_binding(
        shape_name: Option<&str>,
        creation_id: Option<&str>,
    ) -> crate::template_manifest::PageBinding {
        crate::template_manifest::PageBinding {
            shape_name: shape_name.map(str::to_string),
            creation_id: creation_id.map(str::to_string),
            data_path: Some("$.title".to_string()),
            binding_type: Some("text".to_string()),
            required: false,
            max_length: None,
            max_items: None,
            fit: None,
            chart_mode: None,
        }
    }

    fn document_binding(placeholder: Option<&str>) -> crate::template_manifest::DocumentBinding {
        crate::template_manifest::DocumentBinding {
            placeholder: placeholder.map(str::to_string),
            bookmark: None,
            content_control: None,
            data_path: Some("$.title".to_string()),
            binding_type: Some("text".to_string()),
            required: false,
            max_length: None,
            max_items: None,
            width_inches: None,
            height_inches: None,
            block_styles: std::collections::BTreeMap::new(),
        }
    }

    fn sheet_binding(
        cell: Option<&str>,
        named_range: Option<&str>,
    ) -> crate::template_manifest::SheetBinding {
        crate::template_manifest::SheetBinding {
            cell: cell.map(str::to_string),
            named_range: named_range.map(str::to_string),
            data_path: Some("$.title".to_string()),
            binding_type: Some("text".to_string()),
            required: false,
            max_length: None,
            max_items: None,
            table_columns: Vec::new(),
            table_body_row_offset: None,
        }
    }
}
