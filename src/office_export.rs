use serde::Serialize;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::FileOptions};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficeExportResult {
    pub renderer_status: &'static str,
    pub format: &'static str,
    pub output_file: String,
    pub source_file: String,
    pub content_title: String,
    pub warnings: Vec<String>,
    pub planned_page_count: Option<usize>,
    pub sheet_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_integrity: Option<crate::office_package::OfficePackageIntegrity>,
}

#[derive(Debug, Clone)]
struct WorkbookSheet {
    name: String,
    path: String,
}

#[derive(Debug, Clone)]
struct XlsxWorkbookInfo {
    sheets: Vec<WorkbookSheet>,
    named_ranges: BTreeMap<String, XlsxNamedRange>,
}

#[derive(Debug, Clone)]
struct XlsxNamedRange {
    sheet_path: String,
    cell: String,
}

#[derive(Debug, Clone)]
struct XlsxCellWrite {
    value: String,
    style_cell: Option<String>,
    row_style: Option<u32>,
}

#[derive(Debug, Default)]
struct DocxReplacementPlan {
    replacements: BTreeMap<String, DocxReplacement>,
    images: Vec<DocxImageRequest>,
}

#[derive(Debug, Clone)]
enum DocxReplacement {
    Text(String),
    Blocks(DocxBlockReplacement),
}

#[derive(Debug, Clone)]
enum DocxBlockReplacement {
    Paragraph(String),
    List(Vec<String>),
    Table(Vec<Vec<String>>),
    DocumentBlocks {
        blocks: Vec<Value>,
        styles: BTreeMap<String, String>,
    },
    Image {
        token: String,
        index: usize,
        width_emu: i64,
        height_emu: i64,
    },
    Composite(Vec<DocxBlockReplacement>),
}

#[derive(Debug, Clone, Default)]
struct DocxParagraphContext {
    p_pr: Option<String>,
    r_pr: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct DocxTablePrototype {
    tbl_pr: Option<String>,
    tbl_grid: Option<String>,
    rows: Vec<DocxRowPrototype>,
}

#[derive(Debug, Clone, Default)]
struct DocxRowPrototype {
    tr_pr: Option<String>,
    cells: Vec<DocxCellPrototype>,
}

#[derive(Debug, Clone, Default)]
struct DocxCellPrototype {
    tc_pr: Option<String>,
    paragraph: DocxParagraphContext,
}

#[derive(Debug, Clone)]
struct DocxImageRequest {
    token: String,
    source_path: PathBuf,
}

#[derive(Debug, Clone)]
struct PreparedDocxImage {
    token: String,
    rel_id: String,
    media_name: String,
    content_type: &'static str,
    bytes: Vec<u8>,
}

pub fn render_docx(
    input: &Path,
    template: Option<&Path>,
    recipe: Option<&str>,
    out: &Path,
) -> Result<OfficeExportResult, String> {
    if let Some(template) = template {
        return render_docx_template(input, template, recipe, out);
    }

    let mut source = crate::content_source::load_content_source(input)?;
    if source.input_format == "md" {
        source = crate::content_source::apply_md_profile(source, Some("design_doc_v1"))?;
    }
    ensure_parent(out)?;
    let mut zip = open_zip(out)?;
    let document_type = source
        .value
        .get("documentType")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let (content_title, warnings) = if document_type == "design_doc" {
        let input_spec = crate::profiles::input_spec_for_profile("design_doc_v1")?;
        crate::schema_validation::enforce_input_schema(Path::new("."), &input_spec, &source.value)?;
        write_design_docx_package(&mut zip, &source.value)?;
        (
            content_title(&source.value),
            vec![
                "DOCX export used the built-in design document path because no external DOCX template pack was supplied."
                    .to_string(),
            ],
        )
    } else {
        let content =
            serde_json::from_value::<crate::content_ir::ContentDocument>(source.value.clone())
                .map_err(|err| format!("invalid teaching DOCX content: {err}"))?;
        let content_title = content.title.clone();
        write_docx_package(&mut zip, &content)?;
        (
            content_title,
            vec![
                "DOCX export used the built-in minimal handout path because no external DOCX template pack was supplied."
                    .to_string(),
            ],
        )
    };
    finish_zip(zip, out)?;

    Ok(OfficeExportResult {
        renderer_status: "rendered",
        format: "docx",
        output_file: absolutize(out).display().to_string(),
        source_file: absolutize(input).display().to_string(),
        content_title,
        warnings,
        planned_page_count: None,
        sheet_count: None,
        output_integrity: None,
    })
}

pub fn render_docx_atomic(
    input: &Path,
    template: Option<&Path>,
    recipe: Option<&str>,
    out: &Path,
) -> Result<OfficeExportResult, String> {
    let mut transaction = crate::office_package::AtomicOfficeOutput::new(out, "docx")?;
    let mut result = render_docx(input, template, recipe, transaction.staging_path())?;
    let integrity = transaction.validate()?;
    transaction.commit()?;
    result.output_file = transaction.final_path().display().to_string();
    result.output_integrity = Some(integrity);
    Ok(result)
}

fn render_docx_template(
    input: &Path,
    template_dir: &Path,
    recipe: Option<&str>,
    out: &Path,
) -> Result<OfficeExportResult, String> {
    let template_validation = crate::template_manifest::validate_template_pack(template_dir)?;
    let manifest = crate::template_manifest::load_manifest(template_dir)?;
    if manifest.format != "docx" {
        return Err(format!(
            "template '{}' is format '{}', expected docx",
            manifest.template_id, manifest.format
        ));
    }
    let recipe_id = crate::template_manifest::resolve_document_recipe_id(&manifest, recipe)?
        .ok_or_else(|| "document recipe is required for DOCX template rendering".to_string())?;
    crate::template_manifest::validate_document_recipe(&manifest, &recipe_id)?;
    let input_spec = crate::template_manifest::normalized_input_spec(&manifest);
    let mut source = crate::content_source::load_content_source(input)?;
    let allowed_formats = crate::template_manifest::effective_input_formats(&manifest);
    if !allowed_formats.contains(&source.input_format) {
        return Err(format!(
            "input format '{}' is not declared by template '{}'; allowed formats: {}",
            source.input_format,
            manifest.template_id,
            allowed_formats.join(", ")
        ));
    }
    source = crate::content_source::apply_md_profile(source, input_spec.md_profile.as_deref())?;
    crate::schema_validation::enforce_input_schema(template_dir, &input_spec, &source.value)?;

    let mut warnings = template_validation.warnings;
    let replacement_plan = build_docx_replacements(
        &manifest,
        &source.value,
        source.content_root.as_deref(),
        &recipe_id,
        &mut warnings,
    )?;
    ensure_parent(out)?;
    copy_docx_template_with_replacements(
        Path::new(&template_validation.entry_path),
        out,
        &replacement_plan,
    )?;

    Ok(OfficeExportResult {
        renderer_status: "rendered",
        format: "docx",
        output_file: absolutize(out).display().to_string(),
        source_file: absolutize(input).display().to_string(),
        content_title: content_title(&source.value),
        warnings,
        planned_page_count: None,
        sheet_count: None,
        output_integrity: None,
    })
}

fn build_docx_replacements(
    manifest: &crate::template_manifest::TemplateManifest,
    content: &Value,
    content_root: Option<&Path>,
    recipe_id: &str,
    warnings: &mut Vec<String>,
) -> Result<DocxReplacementPlan, String> {
    let steps = manifest
        .document_recipes
        .get(recipe_id)
        .ok_or_else(|| format!("document recipe not found: {recipe_id}"))?;
    let mut plan = DocxReplacementPlan::default();

    for step in steps {
        let block = manifest
            .block_templates
            .get(&step.use_template)
            .ok_or_else(|| format!("missing block template: {}", step.use_template))?;
        let step_values = document_step_values(content, step)?;
        for step_value in step_values {
            for (binding_id, binding) in &block.bindings {
                let Some(placeholder) = &binding.placeholder else {
                    if binding.bookmark.is_some() || binding.content_control.is_some() {
                        warnings.push(format!(
                            "DOCX binding '{binding_id}' uses bookmark/contentControl; placeholder replacement is implemented first"
                        ));
                    }
                    continue;
                };
                let data_path = binding.data_path.as_deref().unwrap_or("$");
                let value = crate::content_ir::resolve_data_path(
                    &step_value,
                    data_path,
                    &step_value,
                )
                .map_err(|err| {
                    format!("DOCX binding '{binding_id}' has invalid dataPath '{data_path}': {err}")
                })?;
                let Some(value) = value else {
                    if binding.required {
                        warnings.push(format!(
                            "DOCX binding '{binding_id}' is required but resolved empty"
                        ));
                    }
                    merge_docx_replacement(
                        &mut plan.replacements,
                        placeholder,
                        DocxReplacement::Text(String::new()),
                    );
                    continue;
                };
                let replacement = docx_binding_replacement(
                    &value,
                    binding,
                    placeholder,
                    content_root,
                    &mut plan.images,
                )?;
                let text_for_validation =
                    docx_binding_plain_text(&value, binding.binding_type.as_deref());
                if let Some(max_length) = binding.max_length {
                    let length = text_for_validation.chars().count();
                    if length > max_length {
                        warnings.push(format!(
                            "DOCX binding '{binding_id}' length {length} exceeds maxLength {max_length}"
                        ));
                    }
                }
                if let Some(max_items) = binding.max_items {
                    if value.as_array().map(Vec::len).unwrap_or(0) > max_items {
                        warnings.push(format!(
                            "DOCX binding '{binding_id}' has more than {max_items} items"
                        ));
                    }
                }
                merge_docx_replacement(&mut plan.replacements, placeholder, replacement);
            }
        }
    }

    Ok(plan)
}

fn document_step_values(
    content: &Value,
    step: &crate::template_manifest::DocumentStep,
) -> Result<Vec<Value>, String> {
    if let Some(repeat_path) = &step.repeat {
        let value = crate::content_ir::resolve_data_path(content, repeat_path, content)?
            .ok_or_else(|| format!("repeat path does not exist: {repeat_path}"))?;
        let items = value
            .as_array()
            .ok_or_else(|| format!("repeat path must resolve to an array: {repeat_path}"))?;
        return Ok(items.clone());
    }

    let data_path = step.data.as_deref().unwrap_or("$");
    let value = crate::content_ir::resolve_data_path(content, data_path, content)?
        .ok_or_else(|| format!("data path does not exist: {data_path}"))?;
    Ok(vec![value])
}

fn docx_binding_replacement(
    value: &Value,
    binding: &crate::template_manifest::DocumentBinding,
    placeholder: &str,
    content_root: Option<&Path>,
    images: &mut Vec<DocxImageRequest>,
) -> Result<DocxReplacement, String> {
    match binding.binding_type.as_deref() {
        Some("list") => Ok(DocxReplacement::Blocks(DocxBlockReplacement::List(
            docx_list_items(value),
        ))),
        Some("table") => Ok(DocxReplacement::Blocks(DocxBlockReplacement::Table(
            table_rows(value),
        ))),
        Some("documentBlocks") => {
            let blocks = value.as_array().cloned().ok_or_else(|| {
                format!(
                    "DOCX documentBlocks binding '{placeholder}' expects an array of semantic blocks"
                )
            })?;
            Ok(DocxReplacement::Blocks(
                DocxBlockReplacement::DocumentBlocks {
                    blocks,
                    styles: binding.block_styles.clone(),
                },
            ))
        }
        Some("image") => {
            let source = image_source(value).ok_or_else(|| {
                format!(
                    "DOCX image binding '{placeholder}' expects a path string or object with src"
                )
            })?;
            let source_path = resolve_local_asset(&source, content_root)?;
            let image_index = images.len() + 1;
            let token = format!("__RDECKFORGE_DOCX_IMAGE_REL_{image_index}__");
            let width_inches = binding
                .width_inches
                .or_else(|| numeric_field(value, "widthInches"))
                .unwrap_or(5.8);
            let height_inches = binding
                .height_inches
                .or_else(|| numeric_field(value, "heightInches"))
                .unwrap_or(3.2);
            images.push(DocxImageRequest {
                token: token.clone(),
                source_path,
            });
            Ok(DocxReplacement::Blocks(DocxBlockReplacement::Image {
                token,
                index: image_index,
                width_emu: inches_to_emu(width_inches),
                height_emu: inches_to_emu(height_inches),
            }))
        }
        _ => Ok(DocxReplacement::Text(crate::content_ir::stringify_value(
            value,
        ))),
    }
}

fn docx_binding_plain_text(value: &Value, binding_type: Option<&str>) -> String {
    if binding_type == Some("list") {
        return value
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(crate::content_ir::stringify_value)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| crate::content_ir::stringify_value(value));
    }
    if binding_type == Some("table") {
        return table_rows(value)
            .into_iter()
            .map(|row| row.join("\t"))
            .collect::<Vec<_>>()
            .join("\n");
    }
    if binding_type == Some("documentBlocks") {
        return value
            .as_array()
            .map(|blocks| {
                blocks
                    .iter()
                    .filter_map(|block| block.get("text"))
                    .map(crate::content_ir::stringify_value)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| crate::content_ir::stringify_value(value));
    }
    if binding_type == Some("image") {
        return image_source(value).unwrap_or_default();
    }
    crate::content_ir::stringify_value(value)
}

fn merge_docx_replacement(
    replacements: &mut BTreeMap<String, DocxReplacement>,
    placeholder: &str,
    value: DocxReplacement,
) {
    replacements
        .entry(placeholder.to_string())
        .and_modify(|current| {
            let merged = match (current.clone(), value.clone()) {
                (DocxReplacement::Text(mut current), DocxReplacement::Text(value)) => {
                    if !current.is_empty() && !value.is_empty() {
                        current.push('\n');
                    }
                    current.push_str(&value);
                    DocxReplacement::Text(current)
                }
                (DocxReplacement::Blocks(current), DocxReplacement::Blocks(value)) => {
                    DocxReplacement::Blocks(merge_docx_blocks(current, value))
                }
                (DocxReplacement::Text(current), DocxReplacement::Blocks(value)) => {
                    let block = if current.is_empty() {
                        value
                    } else {
                        merge_docx_blocks(DocxBlockReplacement::Paragraph(current), value)
                    };
                    DocxReplacement::Blocks(block)
                }
                (DocxReplacement::Blocks(current), DocxReplacement::Text(value)) => {
                    let block = if value.is_empty() {
                        current
                    } else {
                        merge_docx_blocks(current, DocxBlockReplacement::Paragraph(value))
                    };
                    DocxReplacement::Blocks(block)
                }
            };
            *current = merged;
        })
        .or_insert(value);
}

fn merge_docx_blocks(
    current: DocxBlockReplacement,
    value: DocxBlockReplacement,
) -> DocxBlockReplacement {
    let mut items = docx_block_items(current);
    items.extend(docx_block_items(value));
    DocxBlockReplacement::Composite(items)
}

fn docx_block_items(block: DocxBlockReplacement) -> Vec<DocxBlockReplacement> {
    match block {
        DocxBlockReplacement::Composite(items) => items,
        other => vec![other],
    }
}

fn copy_docx_template_with_replacements(
    template_file: &Path,
    out: &Path,
    plan: &DocxReplacementPlan,
) -> Result<(), String> {
    let prepared_images = prepare_docx_images(out, &plan.images)?;
    let input = File::open(template_file).map_err(|err| {
        format!(
            "failed to open DOCX template '{}': {err}",
            template_file.display()
        )
    })?;
    let mut archive = ZipArchive::new(input).map_err(|err| {
        format!(
            "failed to read DOCX template '{}': {err}",
            template_file.display()
        )
    })?;
    let output = File::create(out)
        .map_err(|err| format!("failed to create DOCX output '{}': {err}", out.display()))?;
    let mut zip = ZipWriter::new(output);
    let mut found_document_xml = false;
    let mut found_document_rels = false;
    let mut found_content_types = false;

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|err| format!("failed to read DOCX template entry #{index}: {err}"))?;
        let name = file.name().to_string();
        let options = FileOptions::default().compression_method(file.compression());
        if file.is_dir() {
            zip.add_directory(&name, options)
                .map_err(|err| format!("failed to add DOCX directory '{name}': {err}"))?;
            continue;
        }

        zip.start_file(&name, options)
            .map_err(|err| format!("failed to add DOCX file '{name}': {err}"))?;
        if name == "word/document.xml" {
            found_document_xml = true;
            let mut raw = String::new();
            file.read_to_string(&mut raw)
                .map_err(|err| format!("failed to read word/document.xml as UTF-8: {err}"))?;
            let rendered = apply_docx_replacements(&raw, &plan.replacements, &prepared_images);
            zip.write_all(rendered.as_bytes())
                .map_err(|err| format!("failed to write rendered word/document.xml: {err}"))?;
        } else if name == "word/_rels/document.xml.rels" {
            found_document_rels = true;
            let mut raw = String::new();
            file.read_to_string(&mut raw).map_err(|err| {
                format!("failed to read word/_rels/document.xml.rels as UTF-8: {err}")
            })?;
            let rendered = add_docx_image_relationships(&raw, &prepared_images);
            zip.write_all(rendered.as_bytes()).map_err(|err| {
                format!("failed to write rendered word/_rels/document.xml.rels: {err}")
            })?;
        } else if name == "[Content_Types].xml" {
            found_content_types = true;
            let mut raw = String::new();
            file.read_to_string(&mut raw)
                .map_err(|err| format!("failed to read [Content_Types].xml as UTF-8: {err}"))?;
            let rendered = add_docx_image_content_types(&raw, &prepared_images);
            zip.write_all(rendered.as_bytes())
                .map_err(|err| format!("failed to write rendered [Content_Types].xml: {err}"))?;
        } else {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .map_err(|err| format!("failed to read DOCX file '{name}': {err}"))?;
            zip.write_all(&bytes)
                .map_err(|err| format!("failed to write DOCX file '{name}': {err}"))?;
        }
    }

    if !found_document_rels && !prepared_images.is_empty() {
        zip_file(
            &mut zip,
            "word/_rels/document.xml.rels",
            &add_docx_image_relationships(
                r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"></Relationships>"#,
                &prepared_images,
            ),
        )?;
    }
    if !found_content_types {
        return Err("DOCX template is missing [Content_Types].xml".to_string());
    }
    for image in &prepared_images {
        zip_bytes(
            &mut zip,
            &format!("word/media/{}", image.media_name),
            &image.bytes,
        )?;
    }

    finish_zip(zip, out)?;
    if !found_document_xml {
        return Err("DOCX template is missing word/document.xml".to_string());
    }
    Ok(())
}

fn apply_docx_replacements(
    raw: &str,
    replacements: &BTreeMap<String, DocxReplacement>,
    images: &[PreparedDocxImage],
) -> String {
    let mut rendered = raw.to_string();
    for (placeholder, value) in replacements {
        match value {
            DocxReplacement::Text(value) => {
                rendered = rendered.replace(placeholder, &escape_xml(value));
            }
            DocxReplacement::Blocks(value) => {
                rendered =
                    replace_docx_placeholder_paragraph(&rendered, placeholder, value, images);
            }
        }
    }
    rendered
}

fn replace_docx_placeholder_paragraph(
    raw: &str,
    placeholder: &str,
    replacement: &DocxBlockReplacement,
    images: &[PreparedDocxImage],
) -> String {
    let mut rendered = raw.to_string();
    while let Some(pos) = rendered.find(placeholder) {
        if let DocxBlockReplacement::Table(rows) = replacement {
            if let Some((start, end)) = find_enclosing_xml_element(&rendered, pos, "w:tbl") {
                let table_xml = &rendered[start..end];
                let replacement_xml = docx_table_xml_from_prototype(rows, table_xml, placeholder);
                rendered.replace_range(start..end, &replacement_xml);
                continue;
            }
        }

        let Some(start) = find_xml_element_start(&rendered, pos, "w:p") else {
            let replacement_xml =
                render_docx_block(replacement, &DocxParagraphContext::default(), images);
            rendered = rendered.replacen(placeholder, &replacement_xml, 1);
            continue;
        };
        let Some(end) = find_xml_element_end(&rendered, start, "w:p") else {
            let replacement_xml =
                render_docx_block(replacement, &DocxParagraphContext::default(), images);
            rendered = rendered.replacen(placeholder, &replacement_xml, 1);
            continue;
        };
        let paragraph_xml = &rendered[start..end];
        let context = docx_paragraph_context(paragraph_xml, placeholder);
        let replacement_xml = render_docx_block(replacement, &context, images);
        rendered.replace_range(start..end, &replacement_xml);
    }
    rendered
}

fn find_enclosing_xml_element(raw: &str, position: usize, tag: &str) -> Option<(usize, usize)> {
    let mut search_end = position;
    while let Some(start) = find_xml_element_start(raw, search_end, tag) {
        let Some(end) = find_xml_element_end(raw, start, tag) else {
            search_end = start;
            continue;
        };
        if end >= position {
            return Some((start, end));
        }
        search_end = start;
    }
    None
}

fn find_xml_element_start(raw: &str, before: usize, tag: &str) -> Option<usize> {
    let mut search_end = before;
    let needle = format!("<{tag}");
    while let Some(start) = raw[..search_end].rfind(&needle) {
        if is_xml_element_start(raw, start, tag) {
            return Some(start);
        }
        search_end = start;
    }
    None
}

fn find_next_xml_element_start(raw: &str, from: usize, tag: &str) -> Option<usize> {
    let needle = format!("<{tag}");
    let mut cursor = from;
    while let Some(offset) = raw[cursor..].find(&needle) {
        let start = cursor + offset;
        if is_xml_element_start(raw, start, tag) {
            return Some(start);
        }
        cursor = start + needle.len();
    }
    None
}

fn find_xml_element_end(raw: &str, start: usize, tag: &str) -> Option<usize> {
    if !is_xml_element_start(raw, start, tag) {
        return None;
    }
    let close_tag = format!("</{tag}>");
    let mut depth = 0usize;
    let mut cursor = start;
    loop {
        let next_open = find_next_xml_element_start(raw, cursor, tag);
        let next_close = raw[cursor..].find(&close_tag).map(|offset| cursor + offset);
        match (next_open, next_close) {
            (Some(open), Some(close)) if open < close => {
                depth += 1;
                let open_end = raw[open..].find('>').map(|offset| open + offset + 1)?;
                if raw[open..open_end].trim_end().ends_with("/>") {
                    depth = depth.saturating_sub(1);
                }
                cursor = open_end;
            }
            (_, Some(close)) => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
                cursor = close + close_tag.len();
                if depth == 0 {
                    return Some(cursor);
                }
            }
            _ => return None,
        }
    }
}

fn is_xml_element_start(raw: &str, start: usize, tag: &str) -> bool {
    let prefix = format!("<{tag}");
    if !raw[start..].starts_with(&prefix) {
        return false;
    }
    let Some(next) = raw.as_bytes().get(start + prefix.len()) else {
        return false;
    };
    matches!(next, b'>' | b'/' | b' ' | b'\t' | b'\n' | b'\r')
}

fn render_docx_block(
    replacement: &DocxBlockReplacement,
    context: &DocxParagraphContext,
    images: &[PreparedDocxImage],
) -> String {
    let raw = match replacement {
        DocxBlockReplacement::Paragraph(value) => {
            docx_paragraph_with_context(value, false, context)
        }
        DocxBlockReplacement::List(items) => docx_list_xml(items, context),
        DocxBlockReplacement::Table(rows) => docx_table_xml(rows, context),
        DocxBlockReplacement::DocumentBlocks { blocks, styles } => {
            docx_document_blocks_xml(blocks, styles)
        }
        DocxBlockReplacement::Image {
            token,
            index,
            width_emu,
            height_emu,
        } => docx_image_xml(token, *index, *width_emu, *height_emu, context),
        DocxBlockReplacement::Composite(items) => items
            .iter()
            .map(|item| render_docx_block(item, context, images))
            .collect::<String>(),
    };
    apply_docx_image_tokens(&raw, images)
}

fn docx_document_blocks_xml(blocks: &[Value], styles: &BTreeMap<String, String>) -> String {
    blocks
        .iter()
        .filter_map(|block| {
            let block_type = block
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or("paragraph");
            if block_type == "divider" {
                return Some(docx_horizontal_rule());
            }
            if block_type == "pageBreak" || block_type == "page_break" {
                return Some(docx_page_break());
            }
            let text = document_block_text(block)?.trim();
            if text.is_empty() {
                return None;
            }
            let level = block.get("level").and_then(Value::as_u64).unwrap_or(0);
            let style_key = document_block_style_key(block_type, level);
            let default_style = default_document_block_style(block_type, level);
            let style = styles
                .get(&style_key)
                .or_else(|| styles.get(block_type))
                .map(String::as_str)
                .unwrap_or(&default_style);
            let context = DocxParagraphContext {
                p_pr: Some(format!(
                    r#"<w:pPr><w:pStyle w:val="{}"/></w:pPr>"#,
                    escape_xml(style)
                )),
                r_pr: None,
            };

            if block_type == "code" {
                return Some(
                    text.lines()
                        .map(|line| docx_paragraph_with_context(line, false, &context))
                        .collect::<String>(),
                );
            }
            Some(docx_paragraph_with_context(text, false, &context))
        })
        .collect::<String>()
}

fn document_block_text(block: &Value) -> Option<&str> {
    ["text", "content", "body", "title"]
        .into_iter()
        .find_map(|key| block.get(key).and_then(Value::as_str))
}

fn document_block_style_key(block_type: &str, level: u64) -> String {
    match block_type {
        "heading" => format!("heading{}", level.clamp(1, 6)),
        "listItem" | "orderedItem" if level > 0 => format!("{block_type}{}", level + 1),
        _ => block_type.to_string(),
    }
}

fn default_document_block_style(block_type: &str, level: u64) -> String {
    match block_type {
        "heading" => format!("Heading{}", level.clamp(1, 6)),
        "listItem" if level > 0 => format!("ListBullet{}", level + 1),
        "listItem" => "ListBullet".to_string(),
        "orderedItem" if level > 0 => format!("ListNumber{}", level + 1),
        "orderedItem" => "ListNumber".to_string(),
        "quote" => "Quote".to_string(),
        "code" => "Code".to_string(),
        "callout" => "IntenseQuote".to_string(),
        _ => "Normal".to_string(),
    }
}

fn docx_horizontal_rule() -> String {
    r#"<w:p><w:pPr><w:pBdr><w:bottom w:val="single" w:sz="6" w:space="1" w:color="auto"/></w:pBdr></w:pPr></w:p>"#
        .to_string()
}

fn docx_page_break() -> String {
    r#"<w:p><w:r><w:br w:type="page"/></w:r></w:p>"#.to_string()
}

fn docx_paragraph_context(paragraph_xml: &str, placeholder: &str) -> DocxParagraphContext {
    docx_paragraph_context_optional(paragraph_xml, Some(placeholder))
}

fn docx_paragraph_context_optional(
    paragraph_xml: &str,
    placeholder: Option<&str>,
) -> DocxParagraphContext {
    DocxParagraphContext {
        p_pr: extract_xml_element(paragraph_xml, "w:pPr"),
        r_pr: placeholder
            .and_then(|placeholder| docx_placeholder_run_props(paragraph_xml, placeholder))
            .or_else(|| extract_xml_element(paragraph_xml, "w:rPr")),
    }
}

fn docx_placeholder_run_props(paragraph_xml: &str, placeholder: &str) -> Option<String> {
    let placeholder_pos = paragraph_xml.find(placeholder)?;
    let run_start = paragraph_xml[..placeholder_pos].rfind("<w:r")?;
    let close_offset = paragraph_xml[placeholder_pos..].find("</w:r>")?;
    let run_end = placeholder_pos + close_offset + "</w:r>".len();
    extract_xml_element(&paragraph_xml[run_start..run_end], "w:rPr")
}

fn extract_xml_element(raw: &str, tag: &str) -> Option<String> {
    let open_pos = raw.find(&format!("<{tag}"))?;
    let open_end = open_pos + raw[open_pos..].find('>')? + 1;
    let open_tag = &raw[open_pos..open_end];
    if open_tag.ends_with("/>") {
        return Some(open_tag.to_string());
    }
    let close_tag = format!("</{tag}>");
    let close_end = open_end + raw[open_end..].find(&close_tag)? + close_tag.len();
    Some(raw[open_pos..close_end].to_string())
}

fn apply_docx_image_tokens(raw: &str, images: &[PreparedDocxImage]) -> String {
    let mut rendered = raw.to_string();
    for image in images {
        rendered = rendered.replace(&image.token, &image.rel_id);
    }
    rendered
}

fn docx_list_items(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(crate::content_ir::stringify_value)
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![crate::content_ir::stringify_value(value)])
}

fn docx_list_xml(items: &[String], context: &DocxParagraphContext) -> String {
    items
        .iter()
        .map(|item| docx_paragraph_with_context(&format!("• {item}"), false, context))
        .collect::<String>()
}

fn docx_table_xml(rows: &[Vec<String>], context: &DocxParagraphContext) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let width = 9000 / column_count as i32;
    let grid = (0..column_count)
        .map(|_| format!(r#"<w:gridCol w:w="{width}"/>"#))
        .collect::<String>();
    let rows = rows
        .iter()
        .map(|row| {
            let cells = (0..column_count)
                .map(|index| {
                    let value = row.get(index).map(String::as_str).unwrap_or("");
                    format!(
                        r#"<w:tc><w:tcPr><w:tcW w:w="{width}" w:type="dxa"/></w:tcPr>{}</w:tc>"#,
                        docx_paragraph_with_context(value, false, context)
                    )
                })
                .collect::<String>();
            format!("<w:tr>{cells}</w:tr>")
        })
        .collect::<String>();
    format!(
        r#"<w:tbl><w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/><w:tblBorders><w:top w:val="single" w:sz="4" w:space="0" w:color="D7DEE8"/><w:left w:val="single" w:sz="4" w:space="0" w:color="D7DEE8"/><w:bottom w:val="single" w:sz="4" w:space="0" w:color="D7DEE8"/><w:right w:val="single" w:sz="4" w:space="0" w:color="D7DEE8"/><w:insideH w:val="single" w:sz="4" w:space="0" w:color="D7DEE8"/><w:insideV w:val="single" w:sz="4" w:space="0" w:color="D7DEE8"/></w:tblBorders></w:tblPr><w:tblGrid>{grid}</w:tblGrid>{rows}</w:tbl>"#
    )
}

fn docx_table_xml_from_prototype(
    rows: &[Vec<String>],
    table_xml: &str,
    placeholder: &str,
) -> String {
    if rows.is_empty() {
        return String::new();
    }
    let prototype = parse_docx_table_prototype(table_xml, placeholder);
    if prototype.rows.is_empty() {
        return docx_table_xml(rows, &DocxParagraphContext::default());
    }
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let tbl_pr = prototype.tbl_pr.as_deref().unwrap_or(
        r#"<w:tblPr><w:tblStyle w:val="TableGrid"/><w:tblW w:w="0" w:type="auto"/></w:tblPr>"#,
    );
    let tbl_grid = docx_table_grid_from_prototype(prototype.tbl_grid.as_deref(), column_count);
    let rendered_rows = rows
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            let row_prototype = docx_row_prototype_for_index(&prototype, row_index);
            docx_table_row_from_prototype(row, column_count, row_prototype)
        })
        .collect::<String>();
    format!(r#"<w:tbl>{tbl_pr}{tbl_grid}{rendered_rows}</w:tbl>"#)
}

fn parse_docx_table_prototype(table_xml: &str, placeholder: &str) -> DocxTablePrototype {
    let rows = extract_xml_elements(table_xml, "w:tr")
        .iter()
        .map(|row| parse_docx_row_prototype(row, placeholder))
        .collect::<Vec<_>>();
    DocxTablePrototype {
        tbl_pr: extract_xml_element(table_xml, "w:tblPr"),
        tbl_grid: extract_xml_element(table_xml, "w:tblGrid"),
        rows,
    }
}

fn parse_docx_row_prototype(row_xml: &str, placeholder: &str) -> DocxRowPrototype {
    let cells = extract_xml_elements(row_xml, "w:tc")
        .iter()
        .map(|cell| parse_docx_cell_prototype(cell, placeholder))
        .collect::<Vec<_>>();
    DocxRowPrototype {
        tr_pr: extract_xml_element(row_xml, "w:trPr"),
        cells,
    }
}

fn parse_docx_cell_prototype(cell_xml: &str, placeholder: &str) -> DocxCellPrototype {
    DocxCellPrototype {
        tc_pr: extract_xml_element(cell_xml, "w:tcPr"),
        paragraph: docx_first_paragraph_context(cell_xml, Some(placeholder)),
    }
}

fn extract_xml_elements(raw: &str, tag: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut cursor = 0;
    while let Some(start) = find_next_xml_element_start(raw, cursor, tag) {
        let Some(end) = find_xml_element_end(raw, start, tag) else {
            break;
        };
        items.push(raw[start..end].to_string());
        cursor = end;
    }
    items
}

fn docx_first_paragraph_context(raw: &str, placeholder: Option<&str>) -> DocxParagraphContext {
    let Some(start) = find_next_xml_element_start(raw, 0, "w:p") else {
        return DocxParagraphContext::default();
    };
    let Some(end) = find_xml_element_end(raw, start, "w:p") else {
        return DocxParagraphContext::default();
    };
    docx_paragraph_context_optional(&raw[start..end], placeholder)
}

fn docx_table_grid_from_prototype(tbl_grid: Option<&str>, column_count: usize) -> String {
    let columns = tbl_grid
        .map(|grid| extract_xml_elements(grid, "w:gridCol"))
        .unwrap_or_default();
    if columns.is_empty() {
        let width = 9000 / column_count as i32;
        let grid = (0..column_count)
            .map(|_| format!(r#"<w:gridCol w:w="{width}"/>"#))
            .collect::<String>();
        return format!("<w:tblGrid>{grid}</w:tblGrid>");
    }
    let mut normalized = columns.into_iter().take(column_count).collect::<Vec<_>>();
    while normalized.len() < column_count {
        let fallback = normalized.last().cloned().unwrap_or_else(|| {
            let width = 9000 / column_count as i32;
            format!(r#"<w:gridCol w:w="{width}"/>"#)
        });
        normalized.push(fallback);
    }
    format!("<w:tblGrid>{}</w:tblGrid>", normalized.join(""))
}

fn docx_row_prototype_for_index(
    prototype: &DocxTablePrototype,
    row_index: usize,
) -> &DocxRowPrototype {
    if row_index == 0 {
        return &prototype.rows[0];
    }
    prototype.rows.get(1).unwrap_or(&prototype.rows[0])
}

fn docx_table_row_from_prototype(
    row: &[String],
    column_count: usize,
    prototype: &DocxRowPrototype,
) -> String {
    let tr_pr = prototype.tr_pr.as_deref().unwrap_or("");
    let cells = (0..column_count)
        .map(|index| {
            let value = row.get(index).map(String::as_str).unwrap_or("");
            let fallback = DocxCellPrototype::default();
            let cell = prototype
                .cells
                .get(index)
                .or_else(|| prototype.cells.last())
                .unwrap_or(&fallback);
            docx_table_cell_from_prototype(value, column_count, cell)
        })
        .collect::<String>();
    format!("<w:tr>{tr_pr}{cells}</w:tr>")
}

fn docx_table_cell_from_prototype(
    value: &str,
    column_count: usize,
    prototype: &DocxCellPrototype,
) -> String {
    let default_width = 9000 / column_count as i32;
    let fallback_tc_pr = format!(r#"<w:tcPr><w:tcW w:w="{default_width}" w:type="dxa"/></w:tcPr>"#);
    let tc_pr = prototype.tc_pr.as_deref().unwrap_or(&fallback_tc_pr);
    format!(
        "<w:tc>{tc_pr}{}</w:tc>",
        docx_paragraph_with_context(value, false, &prototype.paragraph)
    )
}

fn docx_image_xml(
    token: &str,
    index: usize,
    width_emu: i64,
    height_emu: i64,
    context: &DocxParagraphContext,
) -> String {
    let p_pr = context.p_pr.as_deref().unwrap_or("");
    let r_pr = context.r_pr.as_deref().unwrap_or("");
    format!(
        r#"<w:p>{p_pr}<w:r>{r_pr}<w:drawing><wp:inline xmlns:wp="http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing" distT="0" distB="0" distL="0" distR="0"><wp:extent cx="{width_emu}" cy="{height_emu}"/><wp:docPr id="{doc_id}" name="Picture {index}"/><a:graphic xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main"><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:pic xmlns:pic="http://schemas.openxmlformats.org/drawingml/2006/picture"><pic:nvPicPr><pic:cNvPr id="{index}" name="rDeckForge image {index}"/><pic:cNvPicPr/></pic:nvPicPr><pic:blipFill><a:blip xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" r:embed="{token}"/><a:stretch><a:fillRect/></a:stretch></pic:blipFill><pic:spPr><a:xfrm><a:ext cx="{width_emu}" cy="{height_emu}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline></w:drawing></w:r></w:p>"#,
        doc_id = 5000 + index
    )
}

fn prepare_docx_images(
    out: &Path,
    requests: &[DocxImageRequest],
) -> Result<Vec<PreparedDocxImage>, String> {
    requests
        .iter()
        .enumerate()
        .map(|(index, request)| {
            let prepared_path = prepare_docx_image_path(out, &request.source_path, index + 1)?;
            let extension = prepared_path
                .extension()
                .and_then(|value| value.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let content_type = match extension.as_str() {
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                other => {
                    return Err(format!(
                        "unsupported DOCX image format '{}': {}",
                        other,
                        prepared_path.display()
                    ));
                }
            };
            let bytes = fs::read(&prepared_path).map_err(|err| {
                format!("failed to read image '{}': {err}", prepared_path.display())
            })?;
            Ok(PreparedDocxImage {
                token: request.token.clone(),
                rel_id: format!("rDeckForgeImage{}", index + 1),
                media_name: format!(
                    "rdeckforge-image-{}.{}",
                    index + 1,
                    if extension == "jpeg" {
                        "jpg"
                    } else {
                        extension.as_str()
                    }
                ),
                content_type,
                bytes,
            })
        })
        .collect()
}

fn prepare_docx_image_path(out: &Path, source: &Path, index: usize) -> Result<PathBuf, String> {
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if extension != "svg" {
        return Ok(source.to_path_buf());
    }
    let media_dir = out
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".rdeckforge-media");
    fs::create_dir_all(&media_dir).map_err(|err| {
        format!(
            "failed to create SVG rasterization directory '{}': {err}",
            media_dir.display()
        )
    })?;
    let scratch = media_dir.join(format!("docx-svg-{index}-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&scratch).map_err(|err| {
        format!(
            "failed to create SVG rasterization scratch directory '{}': {err}",
            scratch.display()
        )
    })?;
    let status = Command::new("/usr/bin/qlmanage")
        .args(["-t", "-s", "1600", "-o"])
        .arg(&scratch)
        .arg(source)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| {
            format!(
                "failed to run qlmanage for SVG '{}': {err}",
                source.display()
            )
        })?;
    if !status.success() {
        let _ = fs::remove_dir_all(&scratch);
        return Err(format!(
            "qlmanage failed to rasterize SVG '{}'",
            source.display()
        ));
    }
    let png = fs::read_dir(&scratch)
        .map_err(|err| format!("failed to read SVG rasterization output: {err}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .and_then(|value| value.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("png"))
                .unwrap_or(false)
        })
        .ok_or_else(|| {
            format!(
                "qlmanage did not produce PNG output for '{}'",
                source.display()
            )
        })?;
    let output = media_dir.join(format!("docx-image-{index}.png"));
    fs::rename(&png, &output).map_err(|err| {
        format!(
            "failed to move rasterized image '{}' to '{}': {err}",
            png.display(),
            output.display()
        )
    })?;
    let _ = fs::remove_dir_all(&scratch);
    Ok(output)
}

fn add_docx_image_relationships(raw: &str, images: &[PreparedDocxImage]) -> String {
    if images.is_empty() {
        return raw.to_string();
    }
    let additions = images
        .iter()
        .map(|image| {
            format!(
                r#"<Relationship Id="{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="media/{}"/>"#,
                image.rel_id, image.media_name
            )
        })
        .collect::<String>();
    raw.replace("</Relationships>", &format!("{additions}</Relationships>"))
}

fn add_docx_image_content_types(raw: &str, images: &[PreparedDocxImage]) -> String {
    let mut rendered = raw.to_string();
    let mut needed = BTreeMap::new();
    for image in images {
        let extension = image
            .media_name
            .rsplit_once('.')
            .map(|(_, extension)| extension)
            .unwrap_or("");
        needed.insert(extension.to_string(), image.content_type);
    }
    for (extension, content_type) in needed {
        if !rendered.contains(&format!(r#"Extension="{extension}""#)) {
            rendered = rendered.replace(
                "</Types>",
                &format!(
                    r#"<Default Extension="{extension}" ContentType="{content_type}"/></Types>"#
                ),
            );
        }
    }
    rendered
}

fn image_source(value: &Value) -> Option<String> {
    if let Some(value) = value.as_str() {
        return Some(value.to_string());
    }
    value
        .as_object()
        .and_then(|object| object.get("src"))
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn resolve_local_asset(source: &str, content_root: Option<&Path>) -> Result<PathBuf, String> {
    if source.starts_with("http://")
        || source.starts_with("https://")
        || source.starts_with("data:")
    {
        return Err(format!(
            "remote or data URI assets are not supported for Office rendering: {source}"
        ));
    }
    let normalized = source.replace('\\', "/");
    let path = Path::new(&normalized);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        content_root
            .ok_or_else(|| format!("relative asset has no content root: {source}"))?
            .join(path)
    };
    if !absolute.is_file() {
        return Err(format!("asset file not found: {}", absolute.display()));
    }
    Ok(absolute)
}

fn numeric_field(value: &Value, key: &str) -> Option<f64> {
    value.as_object()?.get(key)?.as_f64()
}

fn inches_to_emu(value: f64) -> i64 {
    (value.max(0.1) * 914_400.0).round() as i64
}

fn table_rows(value: &Value) -> Vec<Vec<String>> {
    if let Some(array) = value.as_array() {
        if array.iter().all(Value::is_array) {
            return array
                .iter()
                .map(|row| {
                    row.as_array().map_or_else(Vec::new, |items| {
                        items
                            .iter()
                            .map(crate::content_ir::stringify_value)
                            .collect()
                    })
                })
                .collect();
        }
        if array.iter().all(Value::is_object) {
            let columns = collect_table_columns(array);
            let mut rows = vec![columns.clone()];
            rows.extend(array.iter().map(|row| {
                let object = row.as_object();
                columns
                    .iter()
                    .map(|column| {
                        object
                            .and_then(|object| object.get(column))
                            .map(crate::content_ir::stringify_value)
                            .unwrap_or_default()
                    })
                    .collect()
            }));
            return rows;
        }
    }
    if let Some(object) = value.as_object() {
        if let (Some(columns), Some(rows)) = (object.get("columns"), object.get("rows")) {
            let columns = columns
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .map(crate::content_ir::stringify_value)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let mut output = if columns.is_empty() {
                Vec::new()
            } else {
                vec![columns.clone()]
            };
            if let Some(rows) = rows.as_array() {
                output.extend(rows.iter().map(|row| table_row_values(row, &columns)));
            }
            return output;
        }
        if object.contains_key("header") || object.contains_key("body") {
            let mut output = Vec::new();
            if let Some(header) = object.get("header") {
                output.extend(table_header_rows(header));
            }
            if let Some(body) = object.get("body") {
                output.extend(table_rows(body));
            }
            return output;
        }
    }
    Vec::new()
}

fn table_header_rows(value: &Value) -> Vec<Vec<String>> {
    if let Some(items) = value.as_array() {
        if items
            .iter()
            .all(|item| !item.is_array() && !item.is_object())
        {
            return vec![
                items
                    .iter()
                    .map(crate::content_ir::stringify_value)
                    .collect(),
            ];
        }
    }
    table_rows(value)
}

fn table_row_values(value: &Value, columns: &[String]) -> Vec<String> {
    if let Some(items) = value.as_array() {
        return items
            .iter()
            .map(crate::content_ir::stringify_value)
            .collect();
    }
    if let Some(object) = value.as_object() {
        return columns
            .iter()
            .map(|column| {
                object
                    .get(column)
                    .map(crate::content_ir::stringify_value)
                    .unwrap_or_default()
            })
            .collect();
    }
    vec![crate::content_ir::stringify_value(value)]
}

fn collect_table_columns(rows: &[Value]) -> Vec<String> {
    let mut columns = BTreeSet::new();
    for row in rows {
        if let Some(object) = row.as_object() {
            for key in object.keys() {
                columns.insert(key.clone());
            }
        }
    }
    columns.into_iter().collect()
}

fn content_title(value: &Value) -> String {
    value
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Untitled content")
        .to_string()
}

pub fn render_xlsx(
    input: &Path,
    template_dir: Option<&Path>,
    recipe: Option<&str>,
    out: &Path,
) -> Result<OfficeExportResult, String> {
    if let Some(template_dir) = template_dir {
        return render_xlsx_template(input, template_dir, recipe, out);
    }
    render_xlsx_builtin(input, out)
}

pub fn render_xlsx_atomic(
    input: &Path,
    template_dir: Option<&Path>,
    recipe: Option<&str>,
    out: &Path,
) -> Result<OfficeExportResult, String> {
    let mut transaction = crate::office_package::AtomicOfficeOutput::new(out, "xlsx")?;
    let mut result = render_xlsx(input, template_dir, recipe, transaction.staging_path())?;
    let integrity = transaction.validate()?;
    transaction.commit()?;
    result.output_file = transaction.final_path().display().to_string();
    result.output_integrity = Some(integrity);
    Ok(result)
}

fn render_xlsx_template(
    input: &Path,
    template_dir: &Path,
    recipe: Option<&str>,
    out: &Path,
) -> Result<OfficeExportResult, String> {
    let template_validation = crate::template_manifest::validate_template_pack(template_dir)?;
    let manifest = crate::template_manifest::load_manifest(template_dir)?;
    if manifest.format != "xlsx" {
        return Err(format!(
            "template '{}' is format '{}', expected xlsx",
            manifest.template_id, manifest.format
        ));
    }
    let recipe_id = crate::template_manifest::resolve_workbook_recipe_id(&manifest, recipe)?
        .ok_or_else(|| "workbook recipe is required for XLSX template rendering".to_string())?;
    crate::template_manifest::validate_workbook_recipe(&manifest, &recipe_id)?;

    let input_spec = crate::template_manifest::normalized_input_spec(&manifest);
    let mut source = crate::content_source::load_content_source(input)?;
    let allowed_formats = crate::template_manifest::effective_input_formats(&manifest);
    if !allowed_formats.contains(&source.input_format) {
        return Err(format!(
            "input format '{}' is not declared by template '{}'; allowed formats: {}",
            source.input_format,
            manifest.template_id,
            allowed_formats.join(", ")
        ));
    }
    source = crate::content_source::apply_md_profile(source, input_spec.md_profile.as_deref())?;
    crate::schema_validation::enforce_input_schema(template_dir, &input_spec, &source.value)?;

    let template_file = Path::new(&template_validation.entry_path);
    let workbook = read_xlsx_workbook_info(template_file)?;
    let mut warnings = template_validation.warnings;
    let cell_writes = build_xlsx_cell_writes(
        &manifest,
        &source.value,
        &recipe_id,
        &workbook,
        &mut warnings,
    )?;
    ensure_parent(out)?;
    copy_xlsx_template_with_writes(template_file, out, &cell_writes)?;

    Ok(OfficeExportResult {
        renderer_status: "rendered",
        format: "xlsx",
        output_file: absolutize(out).display().to_string(),
        source_file: absolutize(input).display().to_string(),
        content_title: content_title(&source.value),
        warnings,
        planned_page_count: None,
        sheet_count: Some(cell_writes.len()),
        output_integrity: None,
    })
}

fn render_xlsx_builtin(input: &Path, out: &Path) -> Result<OfficeExportResult, String> {
    let source = crate::content_source::load_content_source(input)?;
    let input_spec = crate::profiles::input_spec_for_profile("feature_assessment_v1")?;
    crate::schema_validation::enforce_input_schema(Path::new("."), &input_spec, &source.value)?;
    if !is_assessment_xlsx_content(&source.value) {
        return Err(
            "built-in XLSX rendering currently expects a JSON object with detailed_rows and optional summary; provide a template pack for other XLSX layouts"
                .to_string(),
        );
    }
    ensure_parent(out)?;
    let mut zip = open_zip(out)?;
    write_assessment_xlsx_package(&mut zip, &source.value)?;
    finish_zip(zip, out)?;

    Ok(OfficeExportResult {
        renderer_status: "rendered",
        format: "xlsx",
        output_file: absolutize(out).display().to_string(),
        source_file: absolutize(input).display().to_string(),
        content_title: assessment_xlsx_title(&source.value),
        warnings: vec![
            "XLSX export used the built-in assessment workbook path because no external XLSX template pack was supplied."
                .to_string(),
        ],
        planned_page_count: None,
        sheet_count: Some(1),
        output_integrity: None,
    })
}

fn is_assessment_xlsx_content(value: &Value) -> bool {
    value
        .get("detailed_rows")
        .and_then(Value::as_array)
        .map(|rows| !rows.is_empty())
        .unwrap_or(false)
}

fn assessment_xlsx_title(value: &Value) -> String {
    value
        .get("title")
        .and_then(Value::as_str)
        .or_else(|| {
            value
                .get("metadata")
                .and_then(|metadata| metadata.get("title"))
                .and_then(Value::as_str)
        })
        .unwrap_or("功能点评估")
        .to_string()
}

fn build_xlsx_cell_writes(
    manifest: &crate::template_manifest::TemplateManifest,
    content: &Value,
    recipe_id: &str,
    workbook: &XlsxWorkbookInfo,
    warnings: &mut Vec<String>,
) -> Result<BTreeMap<String, BTreeMap<String, XlsxCellWrite>>, String> {
    let steps = manifest
        .workbook_recipes
        .get(recipe_id)
        .ok_or_else(|| format!("workbook recipe not found: {recipe_id}"))?;
    let mut writes = BTreeMap::new();

    for step in steps {
        let sheet_template = manifest
            .sheet_templates
            .get(&step.use_template)
            .ok_or_else(|| format!("missing sheet template: {}", step.use_template))?;
        let sheet_path = resolve_sheet_template_path(sheet_template, &workbook.sheets)?;
        if let Some(output_name) = &sheet_template.output_sheet_name {
            if sheet_template.source_sheet.as_deref() != Some(output_name.as_str()) {
                warnings.push(format!(
                    "sheetTemplate '{}' requested outputSheetName '{}'; sheet renaming is not implemented yet",
                    step.use_template, output_name
                ));
            }
        }
        let step_values = workbook_step_values(content, step)?;
        for step_value in step_values {
            for (binding_id, binding) in &sheet_template.bindings {
                let Some((binding_sheet_path, cell)) =
                    resolve_xlsx_binding_target(binding, &sheet_path, workbook)?
                else {
                    warnings.push(format!(
                        "XLSX binding '{binding_id}' has neither cell nor namedRange"
                    ));
                    continue;
                };
                let data_path = binding.data_path.as_deref().unwrap_or("$");
                let value = crate::content_ir::resolve_data_path(
                    &step_value,
                    data_path,
                    &step_value,
                )
                .map_err(|err| {
                    format!("XLSX binding '{binding_id}' has invalid dataPath '{data_path}': {err}")
                })?;
                let Some(value) = value else {
                    if binding.required {
                        warnings.push(format!(
                            "XLSX binding '{binding_id}' is required but resolved empty"
                        ));
                    }
                    merge_cell_write(&mut writes, &binding_sheet_path, &cell, "");
                    continue;
                };
                let text = xlsx_binding_text(&value, binding.binding_type.as_deref());
                if let Some(max_length) = binding.max_length {
                    let length = text.chars().count();
                    if length > max_length {
                        warnings.push(format!(
                            "XLSX binding '{binding_id}' length {length} exceeds maxLength {max_length}"
                        ));
                    }
                }
                if let Some(max_items) = binding.max_items {
                    if value.as_array().map(Vec::len).unwrap_or(0) > max_items {
                        warnings.push(format!(
                            "XLSX binding '{binding_id}' has more than {max_items} items"
                        ));
                    }
                }
                apply_xlsx_binding_write(&mut writes, &binding_sheet_path, &cell, &value, binding)?;
            }
        }
    }

    Ok(writes)
}

fn workbook_step_values(
    content: &Value,
    step: &crate::template_manifest::WorkbookStep,
) -> Result<Vec<Value>, String> {
    if let Some(repeat_path) = &step.repeat {
        let value = crate::content_ir::resolve_data_path(content, repeat_path, content)?
            .ok_or_else(|| format!("repeat path does not exist: {repeat_path}"))?;
        let items = value
            .as_array()
            .ok_or_else(|| format!("repeat path must resolve to an array: {repeat_path}"))?;
        return Ok(items.clone());
    }

    let data_path = step.data.as_deref().unwrap_or("$");
    let value = crate::content_ir::resolve_data_path(content, data_path, content)?
        .ok_or_else(|| format!("data path does not exist: {data_path}"))?;
    Ok(vec![value])
}

fn xlsx_binding_text(value: &Value, binding_type: Option<&str>) -> String {
    if binding_type == Some("list") {
        return value
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(crate::content_ir::stringify_value)
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_else(|| crate::content_ir::stringify_value(value));
    }
    crate::content_ir::stringify_value(value)
}

fn resolve_xlsx_binding_target(
    binding: &crate::template_manifest::SheetBinding,
    default_sheet_path: &str,
    workbook: &XlsxWorkbookInfo,
) -> Result<Option<(String, String)>, String> {
    if let Some(cell) = &binding.cell {
        validate_cell_ref(cell)?;
        return Ok(Some((
            default_sheet_path.to_string(),
            cell.to_ascii_uppercase(),
        )));
    }

    if let Some(named_range) = &binding.named_range {
        let target = workbook
            .named_ranges
            .get(named_range)
            .ok_or_else(|| format!("XLSX namedRange not found: {named_range}"))?;
        return Ok(Some((target.sheet_path.clone(), target.cell.clone())));
    }

    Ok(None)
}

fn apply_xlsx_binding_write(
    writes: &mut BTreeMap<String, BTreeMap<String, XlsxCellWrite>>,
    sheet_path: &str,
    cell: &str,
    value: &Value,
    binding: &crate::template_manifest::SheetBinding,
) -> Result<(), String> {
    match binding.binding_type.as_deref() {
        Some("list") => {
            let items = xlsx_list_values(value);
            if items.is_empty() {
                merge_cell_write(writes, sheet_path, cell, "");
                return Ok(());
            }
            let (column, row) = cell_ref_parts(cell)?;
            for (index, item) in items.iter().enumerate() {
                let target_row = row
                    .checked_add(index as u32)
                    .ok_or_else(|| format!("XLSX list binding overflows row range at {cell}"))?;
                merge_cell_write_with_style(
                    writes,
                    sheet_path,
                    &cell_ref(column, target_row),
                    item,
                    Some(cell.to_string()),
                    Some(row),
                );
            }
        }
        Some("table") => {
            let rows = xlsx_table_rows(value, &binding.table_columns);
            if rows.is_empty() {
                merge_cell_write(writes, sheet_path, cell, "");
                return Ok(());
            }
            let (start_column, start_row) = cell_ref_parts(cell)?;
            for (row_offset, row) in rows.iter().enumerate() {
                let target_row = start_row
                    .checked_add(row_offset as u32)
                    .ok_or_else(|| format!("XLSX table binding overflows row range at {cell}"))?;
                for (column_offset, item) in row.iter().enumerate() {
                    let style_row = if row_offset == 0 {
                        start_row
                    } else {
                        start_row + binding.table_body_row_offset.unwrap_or(0)
                    };
                    merge_cell_write_with_style(
                        writes,
                        sheet_path,
                        &cell_ref(start_column + column_offset, target_row),
                        item,
                        Some(cell_ref(start_column + column_offset, style_row)),
                        Some(style_row),
                    );
                }
            }
        }
        _ => merge_cell_write(
            writes,
            sheet_path,
            cell,
            &xlsx_binding_text(value, binding.binding_type.as_deref()),
        ),
    }
    Ok(())
}

fn xlsx_table_rows(
    value: &Value,
    columns: &[crate::template_manifest::TableColumn],
) -> Vec<Vec<String>> {
    if columns.is_empty() {
        return table_rows(value);
    }
    let Some(items) = value.as_array() else {
        return table_rows(value);
    };
    let mut rows = vec![
        columns
            .iter()
            .map(|column| column.header.as_deref().unwrap_or(&column.key).to_string())
            .collect(),
    ];
    rows.extend(items.iter().map(|item| {
        let object = item.as_object();
        columns
            .iter()
            .map(|column| {
                object
                    .and_then(|object| object.get(&column.key))
                    .map(crate::content_ir::stringify_value)
                    .unwrap_or_default()
            })
            .collect()
    }));
    rows
}

fn xlsx_list_values(value: &Value) -> Vec<String> {
    value
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(crate::content_ir::stringify_value)
                .collect()
        })
        .unwrap_or_else(|| vec![crate::content_ir::stringify_value(value)])
}

fn merge_cell_write(
    writes: &mut BTreeMap<String, BTreeMap<String, XlsxCellWrite>>,
    sheet_path: &str,
    cell: &str,
    value: &str,
) {
    merge_cell_write_with_style(writes, sheet_path, cell, value, None, None);
}

fn merge_cell_write_with_style(
    writes: &mut BTreeMap<String, BTreeMap<String, XlsxCellWrite>>,
    sheet_path: &str,
    cell: &str,
    value: &str,
    style_cell: Option<String>,
    row_style: Option<u32>,
) {
    writes
        .entry(sheet_path.to_string())
        .or_default()
        .entry(cell.to_ascii_uppercase())
        .and_modify(|current| {
            if !current.value.is_empty() && !value.is_empty() {
                current.value.push('\n');
            }
            current.value.push_str(value);
            if current.style_cell.is_none() {
                current.style_cell = style_cell.clone();
            }
            if current.row_style.is_none() {
                current.row_style = row_style;
            }
        })
        .or_insert_with(|| XlsxCellWrite {
            value: value.to_string(),
            style_cell,
            row_style,
        });
}

fn resolve_sheet_template_path(
    sheet_template: &crate::template_manifest::SheetTemplate,
    sheets: &[WorkbookSheet],
) -> Result<String, String> {
    if let Some(name) = &sheet_template.source_sheet {
        return sheets
            .iter()
            .find(|sheet| sheet.name == *name)
            .map(|sheet| sheet.path.clone())
            .ok_or_else(|| format!("XLSX sourceSheet not found: {name}"));
    }

    let index = sheet_template.source_sheet_index.unwrap_or(1);
    if index == 0 {
        return Err("sourceSheetIndex is 1-based and must be greater than 0".to_string());
    }
    sheets
        .get(index as usize - 1)
        .map(|sheet| sheet.path.clone())
        .ok_or_else(|| format!("XLSX sourceSheetIndex is out of range: {index}"))
}

fn read_xlsx_workbook_info(template_file: &Path) -> Result<XlsxWorkbookInfo, String> {
    let workbook_xml = read_zip_text(template_file, "xl/workbook.xml")?;
    let rels_xml = read_zip_text(template_file, "xl/_rels/workbook.xml.rels")?;
    let relationships = parse_xlsx_relationships(&rels_xml);
    let mut sheets = Vec::new();

    for sheet_tag in xml_tags(&workbook_xml, "sheet") {
        let Some(name) = xml_attr(sheet_tag, "name") else {
            continue;
        };
        let Some(rel_id) = xml_attr(sheet_tag, "r:id") else {
            continue;
        };
        let Some(target) = relationships.get(&rel_id) else {
            continue;
        };
        sheets.push(WorkbookSheet {
            name,
            path: normalize_xlsx_target(target),
        });
    }

    if sheets.is_empty() {
        return Err("XLSX template has no worksheets in xl/workbook.xml".to_string());
    }
    let named_ranges = parse_xlsx_defined_names(&workbook_xml, &sheets);
    Ok(XlsxWorkbookInfo {
        sheets,
        named_ranges,
    })
}

fn parse_xlsx_relationships(raw: &str) -> BTreeMap<String, String> {
    let mut relationships = BTreeMap::new();
    for tag in xml_tags(raw, "Relationship") {
        let Some(id) = xml_attr(tag, "Id") else {
            continue;
        };
        let Some(target) = xml_attr(tag, "Target") else {
            continue;
        };
        relationships.insert(id, target);
    }
    relationships
}

fn parse_xlsx_defined_names(
    workbook_xml: &str,
    sheets: &[WorkbookSheet],
) -> BTreeMap<String, XlsxNamedRange> {
    let mut named_ranges = BTreeMap::new();
    let mut cursor = 0;
    while let Some(offset) = workbook_xml[cursor..].find("<definedName ") {
        let start = cursor + offset;
        let Some(open_end_offset) = workbook_xml[start..].find('>') else {
            break;
        };
        let open_end = start + open_end_offset + 1;
        let Some(close_offset) = workbook_xml[open_end..].find("</definedName>") else {
            break;
        };
        let close = open_end + close_offset;
        let tag = &workbook_xml[start..open_end];
        let body = &workbook_xml[open_end..close];
        cursor = close + "</definedName>".len();

        let Some(name) = xml_attr(tag, "name") else {
            continue;
        };
        let local_sheet_id = xml_attr(tag, "localSheetId").and_then(|value| value.parse().ok());
        let Some((sheet_name, cell)) = parse_xlsx_defined_name_ref(body, local_sheet_id, sheets)
        else {
            continue;
        };
        let Some(sheet) = sheets.iter().find(|sheet| sheet.name == sheet_name) else {
            continue;
        };
        named_ranges.insert(
            name,
            XlsxNamedRange {
                sheet_path: sheet.path.clone(),
                cell,
            },
        );
    }
    named_ranges
}

fn parse_xlsx_defined_name_ref(
    value: &str,
    local_sheet_id: Option<usize>,
    sheets: &[WorkbookSheet],
) -> Option<(String, String)> {
    let first_ref = value.split(',').next()?.trim();
    if first_ref.contains("#REF!") {
        return None;
    }
    let (sheet_name, reference) = if let Some((sheet, reference)) = first_ref.rsplit_once('!') {
        (unquote_xlsx_sheet_name(sheet), reference)
    } else {
        let sheet = sheets.get(local_sheet_id?)?;
        (sheet.name.clone(), first_ref)
    };
    let cell = reference
        .split(':')
        .next()
        .map(normalize_xlsx_cell_ref)
        .filter(|cell| validate_cell_ref(cell).is_ok())?;
    Some((sheet_name, cell))
}

fn unquote_xlsx_sheet_name(value: &str) -> String {
    let value = value.trim();
    if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        return value[1..value.len() - 1].replace("''", "'");
    }
    value.to_string()
}

fn normalize_xlsx_cell_ref(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_uppercase()
}

fn read_zip_text(zip_path: &Path, entry_path: &str) -> Result<String, String> {
    let input = File::open(zip_path).map_err(|err| {
        format!(
            "failed to open XLSX template '{}': {err}",
            zip_path.display()
        )
    })?;
    let mut archive = ZipArchive::new(input).map_err(|err| {
        format!(
            "failed to read XLSX template '{}': {err}",
            zip_path.display()
        )
    })?;
    let mut file = archive
        .by_name(entry_path)
        .map_err(|err| format!("XLSX template is missing {entry_path}: {err}"))?;
    let mut raw = String::new();
    file.read_to_string(&mut raw)
        .map_err(|err| format!("failed to read {entry_path} as UTF-8: {err}"))?;
    Ok(raw)
}

fn copy_xlsx_template_with_writes(
    template_file: &Path,
    out: &Path,
    cell_writes: &BTreeMap<String, BTreeMap<String, XlsxCellWrite>>,
) -> Result<(), String> {
    let input = File::open(template_file).map_err(|err| {
        format!(
            "failed to open XLSX template '{}': {err}",
            template_file.display()
        )
    })?;
    let mut archive = ZipArchive::new(input).map_err(|err| {
        format!(
            "failed to read XLSX template '{}': {err}",
            template_file.display()
        )
    })?;
    let output = File::create(out)
        .map_err(|err| format!("failed to create XLSX output '{}': {err}", out.display()))?;
    let mut zip = ZipWriter::new(output);
    let mut found_sheets = BTreeSet::new();

    for index in 0..archive.len() {
        let mut file = archive
            .by_index(index)
            .map_err(|err| format!("failed to read XLSX template entry #{index}: {err}"))?;
        let name = file.name().to_string();
        let options = FileOptions::default().compression_method(file.compression());
        if file.is_dir() {
            zip.add_directory(&name, options)
                .map_err(|err| format!("failed to add XLSX directory '{name}': {err}"))?;
            continue;
        }

        zip.start_file(&name, options)
            .map_err(|err| format!("failed to add XLSX file '{name}': {err}"))?;
        if let Some(writes) = cell_writes.get(&name) {
            found_sheets.insert(name.clone());
            let mut raw = String::new();
            file.read_to_string(&mut raw)
                .map_err(|err| format!("failed to read worksheet '{name}' as UTF-8: {err}"))?;
            let rendered = apply_xlsx_cell_writes(&raw, writes)?;
            zip.write_all(rendered.as_bytes())
                .map_err(|err| format!("failed to write rendered worksheet '{name}': {err}"))?;
        } else {
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)
                .map_err(|err| format!("failed to read XLSX file '{name}': {err}"))?;
            zip.write_all(&bytes)
                .map_err(|err| format!("failed to write XLSX file '{name}': {err}"))?;
        }
    }

    finish_zip(zip, out)?;
    for sheet_path in cell_writes.keys() {
        if !found_sheets.contains(sheet_path) {
            return Err(format!(
                "XLSX template is missing worksheet part: {sheet_path}"
            ));
        }
    }
    Ok(())
}

fn apply_xlsx_cell_writes(
    raw: &str,
    writes: &BTreeMap<String, XlsxCellWrite>,
) -> Result<String, String> {
    let mut rendered = raw.to_string();
    let mut ordered_writes = writes.iter().collect::<Vec<_>>();
    ordered_writes.sort_by_key(|(cell, _)| {
        cell_ref_parts(cell)
            .map(|(column, row)| (row, column))
            .unwrap_or((u32::MAX, usize::MAX))
    });
    for (cell, write) in ordered_writes {
        rendered = set_xlsx_cell(&rendered, cell, write)?;
    }
    Ok(rendered)
}

fn set_xlsx_cell(raw: &str, cell: &str, write: &XlsxCellWrite) -> Result<String, String> {
    validate_cell_ref(cell)?;
    let cell = cell.to_ascii_uppercase();
    if let Some((start, end)) = find_xlsx_cell_range(raw, &cell) {
        let template_tag = find_xlsx_cell_open_tag(raw, &cell);
        let cell_xml = xlsx_cell_xml(raw, &cell, &write.value, template_tag.as_deref());
        let mut rendered = String::with_capacity(raw.len() + cell_xml.len());
        rendered.push_str(&raw[..start]);
        rendered.push_str(&cell_xml);
        rendered.push_str(&raw[end..]);
        return Ok(rendered);
    }

    let row_number = cell_row_number(&cell)?;
    let template_tag = write
        .style_cell
        .as_deref()
        .and_then(|style_cell| find_xlsx_cell_open_tag(raw, style_cell));
    let cell_xml = xlsx_cell_xml(raw, &cell, &write.value, template_tag.as_deref());
    if let Some(close) = find_xlsx_row_close(raw, row_number) {
        let mut rendered = String::with_capacity(raw.len() + cell_xml.len());
        rendered.push_str(&raw[..close]);
        rendered.push_str(&cell_xml);
        rendered.push_str(&raw[close..]);
        return Ok(rendered);
    }

    let insert_at = find_xlsx_row_insert_position(raw, row_number)?;
    let row_open = write
        .row_style
        .and_then(|row| find_xlsx_row_open_tag(raw, row))
        .map(|tag| xlsx_row_open_xml(raw, row_number, Some(&tag)))
        .unwrap_or_else(|| xlsx_row_open_xml(raw, row_number, None));
    let row_tag = qualified_xml_tag(raw, "row");
    let row_xml = format!("{row_open}{cell_xml}</{row_tag}>");
    let mut rendered = String::with_capacity(raw.len() + row_xml.len());
    rendered.push_str(&raw[..insert_at]);
    rendered.push_str(&row_xml);
    rendered.push_str(&raw[insert_at..]);
    Ok(rendered)
}

fn find_xlsx_cell_range(raw: &str, cell: &str) -> Option<(usize, usize)> {
    let cell_tag = qualified_xml_tag(raw, "c");
    let needle = format!(r#"r="{cell}""#);
    if let Some(attr_pos) = raw.find(&needle) {
        let start = raw[..attr_pos].rfind(&format!("<{cell_tag}"))?;
        let tag_end = raw[attr_pos..].find('>').map(|value| attr_pos + value)?;
        if raw[start..=tag_end].ends_with("/>") {
            return Some((start, tag_end + 1));
        }
        let close_tag = format!("</{cell_tag}>");
        let close = raw[tag_end + 1..]
            .find(&close_tag)
            .map(|value| tag_end + 1 + value + close_tag.len())?;
        return Some((start, close));
    }
    None
}

fn find_xlsx_cell_open_tag(raw: &str, cell: &str) -> Option<String> {
    let cell_tag = qualified_xml_tag(raw, "c");
    let needle = format!(r#"r="{}""#, cell.to_ascii_uppercase());
    let attr_pos = raw.find(&needle)?;
    let start = raw[..attr_pos].rfind(&format!("<{cell_tag}"))?;
    let tag_end = raw[attr_pos..].find('>').map(|value| attr_pos + value)?;
    Some(raw[start..=tag_end].to_string())
}

fn find_xlsx_row_close(raw: &str, row_number: u32) -> Option<usize> {
    let row_tag = qualified_xml_tag(raw, "row");
    let needle = format!(r#"r="{row_number}""#);
    if let Some(attr_pos) = raw.find(&needle) {
        let start = raw[..attr_pos].rfind(&format!("<{row_tag}"))?;
        let tag_end = raw[attr_pos..].find('>').map(|value| attr_pos + value)?;
        if raw[start..=tag_end].ends_with("/>") {
            return Some(tag_end);
        }
        let close = raw[tag_end + 1..].find(&format!("</{row_tag}>"))?;
        return Some(tag_end + 1 + close);
    }
    None
}

fn find_xlsx_row_open_tag(raw: &str, row_number: u32) -> Option<String> {
    let row_tag = qualified_xml_tag(raw, "row");
    let needle = format!(r#"r="{row_number}""#);
    let attr_pos = raw.find(&needle)?;
    let start = raw[..attr_pos].rfind(&format!("<{row_tag}"))?;
    let tag_end = raw[attr_pos..].find('>').map(|value| attr_pos + value)?;
    Some(raw[start..=tag_end].to_string())
}

fn find_xlsx_row_insert_position(raw: &str, row_number: u32) -> Result<usize, String> {
    let sheet_data_tag = qualified_xml_tag(raw, "sheetData");
    let row_tag = qualified_xml_tag(raw, "row");
    let sheet_data_close = raw
        .find(&format!("</{sheet_data_tag}>"))
        .ok_or_else(|| "worksheet is missing sheetData".to_string())?;
    let mut cursor = 0;
    while let Some(offset) = raw[cursor..sheet_data_close].find(&format!("<{row_tag}")) {
        let start = cursor + offset;
        let Some(tag_end_offset) = raw[start..sheet_data_close].find('>') else {
            break;
        };
        let tag_end = start + tag_end_offset + 1;
        let tag = &raw[start..tag_end];
        if xml_attr(tag, "r")
            .and_then(|value| value.parse::<u32>().ok())
            .map(|existing_row| existing_row > row_number)
            .unwrap_or(false)
        {
            return Ok(start);
        }
        cursor = tag_end;
    }
    Ok(sheet_data_close)
}

fn xlsx_row_open_xml(raw: &str, row_number: u32, template_tag: Option<&str>) -> String {
    let attrs = template_tag
        .map(|tag| preserved_xml_attrs(tag, "row", &["r"]))
        .unwrap_or_default();
    if attrs.is_empty() {
        format!(r#"<{} r="{row_number}">"#, qualified_xml_tag(raw, "row"))
    } else {
        format!(
            r#"<{} r="{row_number}" {attrs}>"#,
            qualified_xml_tag(raw, "row")
        )
    }
}

fn xlsx_cell_xml(raw: &str, cell: &str, value: &str, template_tag: Option<&str>) -> String {
    let attrs = template_tag
        .map(|tag| preserved_xml_attrs(tag, "c", &["r", "t"]))
        .unwrap_or_default();
    let attrs = if attrs.is_empty() {
        String::new()
    } else {
        format!(" {attrs}")
    };
    let cell_tag = qualified_xml_tag(raw, "c");
    let inline_tag = qualified_xml_tag(raw, "is");
    let text_tag = qualified_xml_tag(raw, "t");
    format!(
        r#"<{cell_tag} r="{cell}"{attrs} t="inlineStr"><{inline_tag}><{text_tag}>{}</{text_tag}></{inline_tag}></{cell_tag}>"#,
        escape_xml(value)
    )
}

fn preserved_xml_attrs(tag: &str, tag_name: &str, skip: &[&str]) -> String {
    let trimmed = tag.trim();
    let Some(open_name) = xml_open_tag_name(trimmed) else {
        return String::new();
    };
    if open_name.rsplit(':').next() != Some(tag_name) {
        return String::new();
    }
    let Some(raw_attrs) = trimmed
        .strip_prefix(&format!("<{open_name}"))
        .and_then(|value| value.strip_suffix('>'))
    else {
        return String::new();
    };
    raw_attrs
        .trim()
        .trim_end_matches('/')
        .split_whitespace()
        .filter(|attr| {
            let name = attr.split_once('=').map(|(name, _)| name).unwrap_or(attr);
            !skip.contains(&name)
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn validate_cell_ref(cell: &str) -> Result<(), String> {
    let mut seen_digit = false;
    let mut seen_letter = false;
    for ch in cell.chars() {
        if ch.is_ascii_alphabetic() && !seen_digit {
            seen_letter = true;
            continue;
        }
        if ch.is_ascii_digit() {
            seen_digit = true;
            continue;
        }
        return Err(format!("invalid XLSX cell reference: {cell}"));
    }
    if seen_letter && seen_digit {
        Ok(())
    } else {
        Err(format!("invalid XLSX cell reference: {cell}"))
    }
}

fn cell_row_number(cell: &str) -> Result<u32, String> {
    let digits = cell
        .chars()
        .skip_while(|ch| ch.is_ascii_alphabetic())
        .collect::<String>();
    let row = digits
        .parse::<u32>()
        .map_err(|_| format!("invalid XLSX cell reference: {cell}"))?;
    if row == 0 {
        return Err(format!("invalid XLSX cell reference: {cell}"));
    }
    Ok(row)
}

fn cell_ref_parts(cell: &str) -> Result<(usize, u32), String> {
    validate_cell_ref(cell)?;
    let letters = cell
        .chars()
        .take_while(|ch| ch.is_ascii_alphabetic())
        .collect::<String>();
    let digits = cell
        .chars()
        .skip_while(|ch| ch.is_ascii_alphabetic())
        .collect::<String>();
    let row = digits
        .parse::<u32>()
        .map_err(|_| format!("invalid XLSX cell reference: {cell}"))?;
    Ok((column_index(&letters)?, row))
}

fn cell_ref(column: usize, row: u32) -> String {
    format!("{}{}", column_name(column), row)
}

fn column_index(name: &str) -> Result<usize, String> {
    let mut index = 0usize;
    for ch in name.chars() {
        if !ch.is_ascii_alphabetic() {
            return Err(format!("invalid XLSX column reference: {name}"));
        }
        index = index * 26 + (ch.to_ascii_uppercase() as usize - 'A' as usize + 1);
    }
    if index == 0 {
        return Err(format!("invalid XLSX column reference: {name}"));
    }
    Ok(index)
}

fn normalize_xlsx_target(target: &str) -> String {
    let target = target.trim_start_matches('/');
    if target.starts_with("xl/") {
        target.to_string()
    } else {
        format!("xl/{target}")
    }
}

fn xml_tags<'a>(raw: &'a str, tag_name: &str) -> Vec<&'a str> {
    let mut tags = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = raw[cursor..].find('<') {
        let start = cursor + offset;
        let Some(end_offset) = raw[start..].find('>') else {
            break;
        };
        let end = start + end_offset + 1;
        let tag = &raw[start..end];
        if xml_open_tag_name(tag).and_then(|name| name.rsplit(':').next()) == Some(tag_name) {
            tags.push(tag);
        }
        cursor = end;
    }
    tags
}

fn qualified_xml_tag(raw: &str, tag_name: &str) -> String {
    if let Some(name) = xml_tags(raw, tag_name)
        .first()
        .and_then(|tag| xml_open_tag_name(tag))
    {
        return name.to_string();
    }
    let mut root_prefix = None;
    let mut cursor = 0;
    while let Some(offset) = raw[cursor..].find('<') {
        let start = cursor + offset;
        let Some(end_offset) = raw[start..].find('>') else {
            break;
        };
        let end = start + end_offset + 1;
        if let Some((prefix, _)) =
            xml_open_tag_name(&raw[start..end]).and_then(|name| name.split_once(':'))
        {
            root_prefix = Some(prefix.to_string());
            break;
        }
        cursor = end;
    }
    root_prefix
        .map(|prefix| format!("{prefix}:{tag_name}"))
        .unwrap_or_else(|| tag_name.to_string())
}

fn xml_open_tag_name(tag: &str) -> Option<&str> {
    let value = tag.trim().strip_prefix('<')?;
    if value.starts_with('/') || value.starts_with('!') || value.starts_with('?') {
        return None;
    }
    let end = value
        .find(|ch: char| ch.is_whitespace() || ch == '/' || ch == '>')
        .unwrap_or(value.len());
    (end > 0).then_some(&value[..end])
}

fn xml_attr(tag: &str, attr_name: &str) -> Option<String> {
    let needle = format!(r#"{attr_name}=""#);
    let start = tag.find(&needle)? + needle.len();
    let end = tag[start..].find('"')? + start;
    Some(tag[start..end].to_string())
}

pub fn export_xlsx_report(
    input: &Path,
    template: Option<&Path>,
    recipe: Option<&str>,
    out: &Path,
) -> Result<OfficeExportResult, String> {
    let source = crate::content_source::load_content_source(input)?;
    let validation = crate::content_ir::validate_content_workspace_file(input, template, recipe)?;
    ensure_parent(out)?;
    let mut zip = open_zip(out)?;
    write_xlsx_package(&mut zip, &source.value, &validation)?;
    finish_zip(zip, out)?;

    let warnings = validation
        .content
        .warnings
        .iter()
        .chain(validation.content.schema_errors.iter())
        .chain(
            validation
                .template
                .iter()
                .flat_map(|item| item.warnings.iter()),
        )
        .chain(validation.binding_warnings.iter())
        .cloned()
        .collect();

    Ok(OfficeExportResult {
        renderer_status: "rendered",
        format: "xlsx",
        output_file: absolutize(out).display().to_string(),
        source_file: absolutize(input).display().to_string(),
        content_title: validation.content.title,
        warnings,
        planned_page_count: Some(validation.planned_pages.len()),
        sheet_count: Some(2),
        output_integrity: None,
    })
}

fn write_docx_package(
    zip: &mut ZipWriter<File>,
    content: &crate::content_ir::ContentDocument,
) -> Result<(), String> {
    write_docx_package_with_document(zip, &content.title, &docx_document_xml(content))
}

fn write_design_docx_package(zip: &mut ZipWriter<File>, content: &Value) -> Result<(), String> {
    let title = content_title(content);
    write_docx_package_with_document(zip, &title, &design_docx_document_xml(content))
}

fn write_docx_package_with_document(
    zip: &mut ZipWriter<File>,
    title: &str,
    document_xml: &str,
) -> Result<(), String> {
    zip_file(
        zip,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
</Types>"#,
    )?;
    zip_file(
        zip,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#,
    )?;
    zip_file(
        zip,
        "docProps/app.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>rDeckForge</Application>
</Properties>"#,
    )?;
    zip_file(
        zip,
        "docProps/core.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>{}</dc:title>
  <dc:creator>rDeckForge</dc:creator>
</cp:coreProperties>"#,
            escape_xml(title)
        ),
    )?;
    zip_file(zip, "word/document.xml", document_xml)
}

fn docx_document_xml(content: &crate::content_ir::ContentDocument) -> String {
    let mut body = String::new();
    body.push_str(&docx_paragraph(&content.title, true));
    if let Some(speaker) = &content.speaker {
        body.push_str(&docx_paragraph(&format!("讲者：{speaker}"), false));
    }
    if let Some(date) = &content.date {
        body.push_str(&docx_paragraph(&format!("日期：{date}"), false));
    }

    body.push_str(&docx_heading("教学目标"));
    for (index, goal) in content.goals.iter().enumerate() {
        body.push_str(&docx_paragraph(&format!("{}. {goal}", index + 1), false));
    }

    for chapter in &content.chapters {
        body.push_str(&docx_heading(&chapter.title));
        if let Some(label) = &chapter.label {
            body.push_str(&docx_paragraph(label, false));
        }
        for item in &chapter.items {
            body.push_str(&docx_paragraph(
                &format!("{}：{}", item.heading, item.body),
                false,
            ));
        }
    }

    body.push_str(&docx_heading("课程小结"));
    for item in &content.summary {
        body.push_str(&docx_paragraph(item, false));
    }

    body.push_str(&docx_heading("教学评价"));
    for evaluation in &content.evaluations {
        body.push_str(&docx_paragraph(&evaluation.title, true));
        for item in &evaluation.items {
            body.push_str(&docx_paragraph(&format!("- {item}"), false));
        }
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    {body}
    <w:sectPr>
      <w:pgSz w:w="11906" w:h="16838"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
}

fn design_docx_document_xml(content: &Value) -> String {
    let title = content_title(content);
    let mut body = String::new();
    body.push_str(&docx_styled_paragraph(&title, 36, true, None));
    body.push_str("<w:p/>");

    for block in content
        .get("blocks")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let block_type = block
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or("paragraph");
        let text = block
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        match block_type {
            "heading" => {
                let level = block.get("level").and_then(Value::as_u64).unwrap_or(2);
                body.push_str(&design_docx_heading(text, level));
            }
            "listItem" => {
                let level = block.get("level").and_then(Value::as_u64).unwrap_or(0) as i32;
                body.push_str(&design_docx_list_item(text, level, false));
            }
            "orderedItem" => {
                let level = block.get("level").and_then(Value::as_u64).unwrap_or(0) as i32;
                body.push_str(&design_docx_list_item(text, level, true));
            }
            "code" => body.push_str(&design_docx_code_block(text)),
            "quote" => body.push_str(&design_docx_quote(text)),
            _ => body.push_str(&docx_styled_paragraph(text, 22, false, None)),
        }
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    {body}
    <w:sectPr>
      <w:pgSz w:w="11906" w:h="16838"/>
      <w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/>
    </w:sectPr>
  </w:body>
</w:document>"#
    )
}

fn design_docx_heading(text: &str, level: u64) -> String {
    let (size, before, after) = match level {
        1 => (34, 320, 160),
        2 => (30, 280, 120),
        3 => (26, 220, 100),
        4 => (24, 180, 80),
        _ => (22, 120, 60),
    };
    let style = level.min(4);
    let p_pr = format!(
        r#"<w:pPr><w:pStyle w:val="Heading{style}"/><w:spacing w:before="{before}" w:after="{after}"/></w:pPr>"#
    );
    docx_styled_paragraph(text, size, true, Some(&p_pr))
}

fn design_docx_list_item(text: &str, level: i32, ordered: bool) -> String {
    let indent = 420 + level.max(0) * 360;
    let hanging = 240;
    let marker = if ordered { "1." } else { "•" };
    let p_pr = format!(
        r#"<w:pPr><w:spacing w:before="0" w:after="80"/><w:ind w:left="{indent}" w:hanging="{hanging}"/></w:pPr>"#
    );
    docx_styled_paragraph(&format!("{marker} {text}"), 22, false, Some(&p_pr))
}

fn design_docx_code_block(text: &str) -> String {
    text.lines()
        .map(|line| {
            let p_pr = r#"<w:pPr><w:spacing w:before="0" w:after="40"/><w:ind w:left="360"/></w:pPr>"#;
            let r_pr = r#"<w:rPr><w:rFonts w:ascii="Courier New" w:hAnsi="Courier New"/><w:sz w:val="20"/><w:szCs w:val="20"/></w:rPr>"#;
            docx_raw_paragraph(line, Some(p_pr), Some(r_pr))
        })
        .collect::<String>()
}

fn design_docx_quote(text: &str) -> String {
    let p_pr = r#"<w:pPr><w:spacing w:before="80" w:after="80"/><w:ind w:left="360"/></w:pPr>"#;
    let r_pr =
        r#"<w:rPr><w:i/><w:color w:val="666666"/><w:sz w:val="22"/><w:szCs w:val="22"/></w:rPr>"#;
    docx_raw_paragraph(text, Some(p_pr), Some(r_pr))
}

fn docx_heading(text: &str) -> String {
    docx_paragraph(text, true)
}

fn docx_paragraph(text: &str, bold: bool) -> String {
    docx_paragraph_with_context(text, bold, &DocxParagraphContext::default())
}

fn docx_paragraph_with_context(text: &str, bold: bool, context: &DocxParagraphContext) -> String {
    let paragraph_props = context.p_pr.as_deref().unwrap_or("");
    let run_props = context
        .r_pr
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(|| {
            if bold {
                "<w:rPr><w:b/></w:rPr>".to_string()
            } else {
                String::new()
            }
        });
    docx_raw_paragraph(text, Some(paragraph_props), Some(&run_props))
}

fn docx_styled_paragraph(text: &str, size: i32, bold: bool, p_pr: Option<&str>) -> String {
    let bold_xml = if bold { "<w:b/>" } else { "" };
    let r_pr =
        format!(r#"<w:rPr><w:sz w:val="{size}"/><w:szCs w:val="{size}"/>{bold_xml}</w:rPr>"#);
    docx_raw_paragraph(text, p_pr, Some(&r_pr))
}

fn docx_raw_paragraph(text: &str, p_pr: Option<&str>, r_pr: Option<&str>) -> String {
    let paragraph_props = p_pr.unwrap_or("");
    let run_props = r_pr.unwrap_or("");
    format!(
        "<w:p>{paragraph_props}<w:r>{run_props}{}</w:r></w:p>",
        docx_text_element(text)
    )
}

fn docx_text_element(text: &str) -> String {
    let preserve_space = text
        .chars()
        .next()
        .map(char::is_whitespace)
        .unwrap_or(false)
        || text
            .chars()
            .last()
            .map(char::is_whitespace)
            .unwrap_or(false);
    if preserve_space {
        format!(r#"<w:t xml:space="preserve">{}</w:t>"#, escape_xml(text))
    } else {
        format!("<w:t>{}</w:t>", escape_xml(text))
    }
}

fn write_xlsx_package(
    zip: &mut ZipWriter<File>,
    content: &Value,
    validation: &crate::content_ir::ContentWorkspaceValidation,
) -> Result<(), String> {
    zip_file(
        zip,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/worksheets/sheet2.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
</Types>"#,
    )?;
    zip_file(
        zip,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#,
    )?;
    zip_file(
        zip,
        "xl/workbook.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Content" sheetId="1" r:id="rId1"/>
    <sheet name="Validation" sheetId="2" r:id="rId2"/>
  </sheets>
</workbook>"#,
    )?;
    zip_file(
        zip,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet2.xml"/>
</Relationships>"#,
    )?;
    zip_file(
        zip,
        "docProps/app.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>rDeckForge</Application>
</Properties>"#,
    )?;
    zip_file(
        zip,
        "docProps/core.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>{}</dc:title>
  <dc:creator>rDeckForge</dc:creator>
</cp:coreProperties>"#,
            escape_xml(&validation.content.title)
        ),
    )?;
    zip_file(zip, "xl/worksheets/sheet1.xml", &content_sheet_xml(content))?;
    zip_file(
        zip,
        "xl/worksheets/sheet2.xml",
        &validation_sheet_xml(validation),
    )
}

fn write_assessment_xlsx_package(zip: &mut ZipWriter<File>, content: &Value) -> Result<(), String> {
    let title = assessment_xlsx_title(content);
    zip_file(
        zip,
        "[Content_Types].xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
  <Override PartName="/xl/styles.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.styles+xml"/>
  <Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/>
  <Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/>
</Types>"#,
    )?;
    zip_file(
        zip,
        "_rels/.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/>
  <Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/>
</Relationships>"#,
    )?;
    zip_file(
        zip,
        "xl/workbook.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="前端评估" sheetId="1" r:id="rId1"/></sheets>
</workbook>"#,
    )?;
    zip_file(
        zip,
        "xl/_rels/workbook.xml.rels",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
  <Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles" Target="styles.xml"/>
</Relationships>"#,
    )?;
    zip_file(
        zip,
        "docProps/app.xml",
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties">
  <Application>rDeckForge</Application>
</Properties>"#,
    )?;
    zip_file(
        zip,
        "docProps/core.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/">
  <dc:title>{}</dc:title>
  <dc:creator>rDeckForge</dc:creator>
</cp:coreProperties>"#,
            escape_xml(&title)
        ),
    )?;
    zip_file(zip, "xl/styles.xml", assessment_xlsx_styles_xml())?;
    zip_file(
        zip,
        "xl/worksheets/sheet1.xml",
        &assessment_xlsx_sheet_xml(content),
    )
}

fn assessment_xlsx_styles_xml() -> &'static str {
    r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<styleSheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <fonts count="3">
    <font><name val="Calibri"/><family val="2"/><color theme="1"/><sz val="11"/></font>
    <font><b/><color rgb="00FFFFFF"/><sz val="11"/></font>
    <font><b/><color rgb="00000000"/><sz val="11"/></font>
  </fonts>
  <fills count="4">
    <fill><patternFill/></fill>
    <fill><patternFill patternType="gray125"/></fill>
    <fill><patternFill patternType="solid"><fgColor rgb="001F4E78"/></patternFill></fill>
    <fill><patternFill patternType="solid"><fgColor rgb="00D9EAF7"/></patternFill></fill>
  </fills>
  <borders count="2">
    <border><left/><right/><top/><bottom/><diagonal/></border>
    <border><left style="thin"><color rgb="00D9D9D9"/></left><right style="thin"><color rgb="00D9D9D9"/></right><top style="thin"><color rgb="00D9D9D9"/></top><bottom style="thin"><color rgb="00D9D9D9"/></bottom></border>
  </borders>
  <cellStyleXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0"/></cellStyleXfs>
  <cellXfs count="4">
    <xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/>
    <xf numFmtId="0" fontId="1" fillId="2" borderId="1" applyAlignment="1" xfId="0"><alignment horizontal="center" vertical="center" wrapText="1"/></xf>
    <xf numFmtId="0" fontId="0" fillId="0" borderId="1" applyAlignment="1" xfId="0"><alignment vertical="center" wrapText="1"/></xf>
    <xf numFmtId="0" fontId="2" fillId="3" borderId="1" applyAlignment="1" xfId="0"><alignment vertical="center" wrapText="1"/></xf>
  </cellXfs>
  <cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles>
  <tableStyles count="0" defaultTableStyle="TableStyleMedium9" defaultPivotStyle="PivotStyleLight16"/>
</styleSheet>"#
}

fn assessment_xlsx_sheet_xml(content: &Value) -> String {
    let headers = [
        "模块",
        "功能点",
        "改造类型",
        "工时(人天)",
        "是否后端依赖",
        "后端工作说明",
        "负责人",
        "开发进度",
        "说明",
    ];
    let rows = content
        .get("detailed_rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let summary = content.get("summary").and_then(Value::as_object);
    let detail_count = rows.len();
    let blank_row = detail_count + 2;
    let total_row = detail_count + 3;
    let summary_start = total_row + 3;
    let dimension_end = summary_start + 4;

    let mut sheet_rows = String::new();
    sheet_rows.push_str(&assessment_xlsx_header_row(&headers));
    for (index, row) in rows.iter().enumerate() {
        sheet_rows.push_str(&assessment_xlsx_detail_row(index + 2, row));
    }
    sheet_rows.push_str(&assessment_xlsx_blank_row(blank_row, 2));
    sheet_rows.push_str(&assessment_xlsx_total_row(total_row, detail_count));
    sheet_rows.push_str(&assessment_xlsx_blank_row(total_row + 1, 2));
    sheet_rows.push_str(&assessment_xlsx_blank_row(total_row + 2, 2));
    sheet_rows.push_str(&assessment_xlsx_summary_row(
        summary_start,
        "评估口径",
        summary
            .and_then(|item| item.get("evaluation_basis"))
            .and_then(Value::as_str)
            .unwrap_or_default(),
    ));
    sheet_rows.push_str(&assessment_xlsx_blank_row(summary_start + 1, 2));
    sheet_rows.push_str(&assessment_xlsx_summary_row(
        summary_start + 2,
        "关键假设",
        summary
            .and_then(|item| item.get("assumptions"))
            .and_then(Value::as_str)
            .unwrap_or_default(),
    ));
    sheet_rows.push_str(&assessment_xlsx_blank_row(summary_start + 3, 2));
    sheet_rows.push_str(&assessment_xlsx_summary_row(
        summary_start + 4,
        "范围外项",
        summary
            .and_then(|item| item.get("out_of_scope"))
            .and_then(Value::as_str)
            .unwrap_or_default(),
    ));

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetPr><outlinePr summaryBelow="1" summaryRight="1"/><pageSetUpPr/></sheetPr>
  <dimension ref="A1:I{dimension_end}"/>
  <sheetViews><sheetView workbookViewId="0"><selection activeCell="A1" sqref="A1"/></sheetView></sheetViews>
  <sheetFormatPr baseColWidth="8" defaultRowHeight="15"/>
  <cols><col width="16" customWidth="1" min="1" max="1"/><col width="32" customWidth="1" min="2" max="2"/><col width="12" customWidth="1" min="3" max="3"/><col width="10" customWidth="1" min="4" max="4"/><col width="12" customWidth="1" min="5" max="5"/><col width="40" customWidth="1" min="6" max="6"/><col width="10" customWidth="1" min="7" max="7"/><col width="12" customWidth="1" min="8" max="8"/><col width="42" customWidth="1" min="9" max="9"/></cols>
  <sheetData>{sheet_rows}</sheetData>
  <mergeCells count="3"><mergeCell ref="B{summary_start}:I{summary_start}"/><mergeCell ref="B{}:I{}"/><mergeCell ref="B{}:I{}"/></mergeCells>
  <pageMargins left="0.75" right="0.75" top="1" bottom="1" header="0.5" footer="0.5"/>
</worksheet>"#,
        summary_start + 2,
        summary_start + 2,
        summary_start + 4,
        summary_start + 4
    )
}

fn assessment_xlsx_header_row(headers: &[&str]) -> String {
    let cells = headers
        .iter()
        .enumerate()
        .map(|(index, value)| xlsx_inline_cell(index + 1, 1, Some(1), value))
        .collect::<String>();
    format!(r#"<row r="1">{cells}</row>"#)
}

fn assessment_xlsx_detail_row(row_number: usize, row: &Value) -> String {
    let fields = [
        "module",
        "feature",
        "change_type",
        "effort",
        "backend_dependency",
        "backend_notes",
        "owner",
        "progress",
        "notes",
    ];
    let cells = fields
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let column = index + 1;
            let value = row.get(field).unwrap_or(&Value::Null);
            if *field == "effort" {
                xlsx_number_cell(column, row_number, Some(2), value)
            } else {
                xlsx_inline_cell(
                    column,
                    row_number,
                    Some(2),
                    &value
                        .as_str()
                        .map(str::to_string)
                        .unwrap_or_else(|| scalar_value(value)),
                )
            }
        })
        .collect::<String>();
    format!(r#"<row r="{row_number}" ht="34" customHeight="1">{cells}</row>"#)
}

fn assessment_xlsx_blank_row(row_number: usize, style: usize) -> String {
    let cells = (1..=9)
        .map(|column| xlsx_blank_cell(column, row_number, Some(style)))
        .collect::<String>();
    format!(r#"<row r="{row_number}">{cells}</row>"#)
}

fn assessment_xlsx_total_row(row_number: usize, detail_count: usize) -> String {
    let mut cells = String::new();
    cells.push_str(&xlsx_inline_cell(1, row_number, Some(3), "合计"));
    cells.push_str(&xlsx_blank_cell(2, row_number, Some(3)));
    cells.push_str(&xlsx_blank_cell(3, row_number, Some(3)));
    let formula = if detail_count == 0 {
        "0".to_string()
    } else {
        format!("SUM(D2:D{})", detail_count + 1)
    };
    cells.push_str(&xlsx_formula_cell(4, row_number, Some(3), &formula));
    for column in 5..=9 {
        cells.push_str(&xlsx_blank_cell(column, row_number, Some(3)));
    }
    format!(r#"<row r="{row_number}" ht="34" customHeight="1">{cells}</row>"#)
}

fn assessment_xlsx_summary_row(row_number: usize, label: &str, value: &str) -> String {
    let mut cells = String::new();
    cells.push_str(&xlsx_inline_cell(1, row_number, Some(3), label));
    cells.push_str(&xlsx_inline_cell(2, row_number, Some(2), value));
    for column in 3..=9 {
        cells.push_str(&xlsx_blank_cell(column, row_number, Some(2)));
    }
    format!(r#"<row r="{row_number}">{cells}</row>"#)
}

fn xlsx_inline_cell(column: usize, row: usize, style: Option<usize>, value: &str) -> String {
    let cell_ref = format!("{}{}", column_name(column), row);
    let style = style
        .map(|style| format!(r#" s="{style}""#))
        .unwrap_or_default();
    if value.is_empty() {
        return format!(r#"<c r="{cell_ref}"{style} t="inlineStr"></c>"#);
    }
    format!(
        r#"<c r="{cell_ref}"{style} t="inlineStr"><is><t>{}</t></is></c>"#,
        escape_xml(value)
    )
}

fn xlsx_blank_cell(column: usize, row: usize, style: Option<usize>) -> String {
    let cell_ref = format!("{}{}", column_name(column), row);
    let style = style
        .map(|style| format!(r#" s="{style}""#))
        .unwrap_or_default();
    format!(r#"<c r="{cell_ref}"{style}></c>"#)
}

fn xlsx_number_cell(column: usize, row: usize, style: Option<usize>, value: &Value) -> String {
    let Some(number) = value.as_f64() else {
        return xlsx_blank_cell(column, row, style);
    };
    let cell_ref = format!("{}{}", column_name(column), row);
    let style = style
        .map(|style| format!(r#" s="{style}""#))
        .unwrap_or_default();
    format!(r#"<c r="{cell_ref}"{style} t="n"><v>{number}</v></c>"#)
}

fn xlsx_formula_cell(column: usize, row: usize, style: Option<usize>, formula: &str) -> String {
    let cell_ref = format!("{}{}", column_name(column), row);
    let style = style
        .map(|style| format!(r#" s="{style}""#))
        .unwrap_or_default();
    format!(
        r#"<c r="{cell_ref}"{style}><f>{}</f><v></v></c>"#,
        escape_xml(formula)
    )
}

fn content_sheet_xml(content: &Value) -> String {
    let mut rows = vec![vec![
        "Section".to_string(),
        "Key".to_string(),
        "Value".to_string(),
    ]];

    flatten_value_rows("Content", "$", content, &mut rows);

    worksheet_xml(&rows)
}

fn validation_sheet_xml(validation: &crate::content_ir::ContentWorkspaceValidation) -> String {
    let mut rows = vec![
        vec!["Type".to_string(), "Key".to_string(), "Detail".to_string()],
        vec![
            "Content".to_string(),
            "Title".to_string(),
            validation.content.title.clone(),
        ],
        vec![
            "Content".to_string(),
            "InputFormat".to_string(),
            validation.content.input_format.clone(),
        ],
        vec![
            "Content".to_string(),
            "InputSchema".to_string(),
            validation
                .content
                .input_schema
                .clone()
                .or_else(|| validation.content.input_schema_id.clone())
                .unwrap_or_default(),
        ],
        vec![
            "Content".to_string(),
            "Counts".to_string(),
            format!(
                "{} goals, {} chapters, {} summary items, {} evaluations",
                validation.content.goals,
                validation.content.chapters,
                validation.content.summary_items,
                validation.content.evaluations
            ),
        ],
        vec![
            "Template".to_string(),
            "Name".to_string(),
            validation
                .template
                .as_ref()
                .map(|item| item.name.clone())
                .unwrap_or_else(|| "No template selected".to_string()),
        ],
        vec![
            "Recipe".to_string(),
            "DeckRecipe".to_string(),
            validation.deck_recipe.clone().unwrap_or_default(),
        ],
    ];

    for warning in &validation.content.warnings {
        rows.push(vec![
            "Warning".to_string(),
            "Content".to_string(),
            warning.clone(),
        ]);
    }
    for error in &validation.content.schema_errors {
        rows.push(vec![
            "Error".to_string(),
            "InputSchema".to_string(),
            error.clone(),
        ]);
    }
    if let Some(template) = &validation.template {
        for warning in &template.warnings {
            rows.push(vec![
                "Warning".to_string(),
                "Template".to_string(),
                warning.clone(),
            ]);
        }
    }
    for warning in &validation.binding_warnings {
        rows.push(vec![
            "Warning".to_string(),
            "Binding".to_string(),
            warning.clone(),
        ]);
    }
    for page in &validation.planned_pages {
        rows.push(vec![
            "PlannedPage".to_string(),
            format!("{}", page.page_index),
            format!(
                "{} | source slide {} | {}",
                page.page_template, page.source_slide, page.data_path
            ),
        ]);
    }

    worksheet_xml(&rows)
}

fn flatten_value_rows(section: &str, path: &str, value: &Value, rows: &mut Vec<Vec<String>>) {
    match value {
        Value::Object(object) => {
            if object.is_empty() {
                rows.push(vec![
                    section.to_string(),
                    path.to_string(),
                    "{}".to_string(),
                ]);
            }
            for (key, child) in object {
                let child_path = if path == "$" {
                    format!("$.{key}")
                } else {
                    format!("{path}.{key}")
                };
                flatten_value_rows(section, &child_path, child, rows);
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                rows.push(vec![
                    section.to_string(),
                    path.to_string(),
                    "[]".to_string(),
                ]);
            }
            for (index, child) in items.iter().enumerate() {
                flatten_value_rows(section, &format!("{path}[{index}]"), child, rows);
            }
        }
        _ => rows.push(vec![
            section.to_string(),
            path.to_string(),
            scalar_value(value),
        ]),
    }
}

fn scalar_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Number(value) => value.to_string(),
        Value::Bool(value) => value.to_string(),
        _ => serde_json::to_string(value).unwrap_or_default(),
    }
}

fn worksheet_xml(rows: &[Vec<String>]) -> String {
    let rows = rows
        .iter()
        .enumerate()
        .map(|(row_index, row)| {
            let row_number = row_index + 1;
            let cells = row
                .iter()
                .enumerate()
                .map(|(column_index, value)| {
                    let cell_ref = format!("{}{}", column_name(column_index + 1), row_number);
                    format!(
                        r#"<c r="{cell_ref}" t="inlineStr"><is><t>{}</t></is></c>"#,
                        escape_xml(value)
                    )
                })
                .collect::<String>();
            format!(r#"<row r="{row_number}">{cells}</row>"#)
        })
        .collect::<String>();

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>{rows}</sheetData>
</worksheet>"#
    )
}

fn zip_file(zip: &mut ZipWriter<File>, path: &str, content: &str) -> Result<(), String> {
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(path, options)
        .map_err(|err| format!("failed to add '{path}' to Office package: {err}"))?;
    zip.write_all(content.as_bytes())
        .map_err(|err| format!("failed to write '{path}' to Office package: {err}"))
}

fn zip_bytes(zip: &mut ZipWriter<File>, path: &str, content: &[u8]) -> Result<(), String> {
    let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
    zip.start_file(path, options)
        .map_err(|err| format!("failed to add '{path}' to Office package: {err}"))?;
    zip.write_all(content)
        .map_err(|err| format!("failed to write '{path}' to Office package: {err}"))
}

fn open_zip(path: &Path) -> Result<ZipWriter<File>, String> {
    let file = File::create(path)
        .map_err(|err| format!("failed to create '{}': {err}", path.display()))?;
    Ok(ZipWriter::new(file))
}

fn finish_zip(mut zip: ZipWriter<File>, path: &Path) -> Result<(), String> {
    zip.finish().map(|_| ()).map_err(|err| {
        format!(
            "failed to finish Office package '{}': {err}",
            path.display()
        )
    })
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create output directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    Ok(())
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

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn column_name(mut index: usize) -> String {
    let mut name = String::new();
    while index > 0 {
        let rem = (index - 1) % 26;
        name.insert(0, (b'A' + rem as u8) as char);
        index = (index - 1) / 26;
    }
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_docx_from_design_markdown_without_template() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-design-doc-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).map_err(|err| format!("failed to create test dir: {err}"))?;
        let input = root.join("design.md");
        fs::write(
            &input,
            r#"# 示例项目 V1.0 前端概要设计

## 1. 文档信息
- 文档名称：示例项目 V1.0 前端概要设计
- 适用项目：`/workspace/example-project`

## 2. 总体方案
### 2.1 架构原则
- 复用现有组件
- 保持提交结构不变

#### 2.1.1 页面范围
普通段落内容。

```ts
const flag = true;
```
"#,
        )
        .map_err(|err| format!("failed to write test markdown: {err}"))?;
        let out = root.join("design.docx");

        render_docx(&input, None, None, &out)?;
        let document = read_zip_text(&out, "word/document.xml")?;

        assert!(document.contains("示例项目 V1.0 前端概要设计"));
        assert!(document.contains(r#"<w:pStyle w:val="Heading2"/>"#));
        assert!(document.contains(r#"<w:pStyle w:val="Heading3"/>"#));
        assert!(document.contains(r#"<w:pStyle w:val="Heading4"/>"#));
        assert!(document.contains("• 文档名称：示例项目 V1.0 前端概要设计"));
        assert!(document.contains("/workspace/example-project"));
        assert!(document.contains("普通段落内容。"));
        assert!(document.contains("Courier New"));
        assert!(document.contains("const flag = true;"));
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn render_docx_semantic_blocks_use_template_style_map() {
        let blocks = serde_json::json!([
            { "type": "heading", "level": 2, "text": "总体方案" },
            { "type": "paragraph", "text": "复用现有能力。" },
            { "type": "listItem", "level": 0, "text": "保持结构稳定" },
            { "type": "callout", "body": "关键提醒" },
            { "type": "divider" },
            { "type": "pageBreak" },
            { "type": "code", "language": "ts", "content": "const enabled = true;\nreturn enabled;" }
        ]);
        let styles = BTreeMap::from([
            ("heading2".to_string(), "DesignHeading2".to_string()),
            ("paragraph".to_string(), "DesignBody".to_string()),
            ("listItem".to_string(), "DesignBullet".to_string()),
            ("code".to_string(), "DesignCode".to_string()),
        ]);

        let xml = docx_document_blocks_xml(blocks.as_array().unwrap(), &styles);

        assert!(xml.contains(r#"<w:pStyle w:val="DesignHeading2"/>"#));
        assert!(xml.contains(r#"<w:pStyle w:val="DesignBody"/>"#));
        assert!(xml.contains(r#"<w:pStyle w:val="DesignBullet"/>"#));
        assert_eq!(xml.matches(r#"<w:pStyle w:val="DesignCode"/>"#).count(), 2);
        assert!(xml.contains(r#"<w:pStyle w:val="IntenseQuote"/>"#));
        assert!(xml.contains(r#"<w:bottom w:val="single""#));
        assert!(xml.contains(r#"<w:br w:type="page"/>"#));
        assert!(xml.contains("const enabled = true;"));
        assert!(xml.contains("return enabled;"));
    }

    #[test]
    fn xlsx_table_columns_keep_declared_order_and_headers() {
        let value = serde_json::json!([
            { "feature": "新增菜单", "module": "营销配置", "effort": 0.5 },
            { "feature": "新增海报", "module": "营销宣传", "effort": 1 }
        ]);
        let columns = vec![
            crate::template_manifest::TableColumn {
                key: "module".to_string(),
                header: Some("模块".to_string()),
            },
            crate::template_manifest::TableColumn {
                key: "feature".to_string(),
                header: Some("功能点".to_string()),
            },
            crate::template_manifest::TableColumn {
                key: "effort".to_string(),
                header: Some("工时".to_string()),
            },
        ];

        let rows = xlsx_table_rows(&value, &columns);

        assert_eq!(rows[0], vec!["模块", "功能点", "工时"]);
        assert_eq!(rows[1], vec!["营销配置", "新增菜单", "0.5"]);
        assert_eq!(rows[2], vec!["营销宣传", "新增海报", "1"]);
    }

    #[test]
    fn xlsx_writer_preserves_namespace_prefixes() -> Result<(), String> {
        let raw = r#"<x:worksheet xmlns:x="urn:test"><x:sheetData><x:row r="1"><x:c r="A1" s="2" /></x:row></x:sheetData></x:worksheet>"#;
        let existing = XlsxCellWrite {
            value: "标题".to_string(),
            style_cell: None,
            row_style: None,
        };
        let inserted = XlsxCellWrite {
            value: "正文".to_string(),
            style_cell: Some("A1".to_string()),
            row_style: Some(1),
        };

        let rendered = set_xlsx_cell(raw, "A1", &existing)?;
        let rendered = set_xlsx_cell(&rendered, "A2", &inserted)?;

        assert!(rendered.contains(r#"<x:c r="A1" s="2" t="inlineStr">"#));
        assert!(rendered.contains("<x:is><x:t>标题</x:t></x:is>"));
        assert!(rendered.contains(r#"<x:row r="2">"#));
        assert!(rendered.contains(r#"<x:c r="A2" s="2" t="inlineStr">"#));
        assert!(!rendered.contains("<c "));
        assert!(!rendered.contains("<row "));
        Ok(())
    }

    #[test]
    fn render_docx_blocks_preserve_placeholder_paragraph_context() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!("rdeckforge-docx-{}", uuid::Uuid::new_v4()));
        let template_dir = root.join("template");
        fs::create_dir_all(&template_dir)
            .map_err(|err| format!("failed to create test template dir: {err}"))?;

        let template_file = template_dir.join("template.docx");
        let mut zip = open_zip(&template_file)?;
        zip_file(
            &mut zip,
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
        )?;
        zip_file(
            &mut zip,
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
        )?;
        zip_file(
            &mut zip,
            "word/document.xml",
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>{{title}}</w:t></w:r></w:p>
    <w:p><w:pPr><w:pStyle w:val="ListParagraph"/><w:spacing w:before="120"/><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:b/><w:color w:val="2F5597"/></w:rPr><w:t>{{goals}}</w:t></w:r></w:p>
    <w:p><w:pPr><w:pStyle w:val="TableText"/></w:pPr><w:r><w:rPr><w:i/></w:rPr><w:t>{{assessmentTable}}</w:t></w:r></w:p>
  </w:body>
</w:document>"#,
        )?;
        finish_zip(zip, &template_file)?;

        fs::write(
            template_dir.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "test-docx-style-context",
  "name": "Test DOCX Style Context",
  "format": "docx",
  "entry": "template.docx",
  "input": { "formats": ["json"] },
  "blockTemplates": {
    "summary": {
      "bindings": {
        "title": { "placeholder": "{{title}}", "dataPath": "$.title", "type": "text" },
        "goals": { "placeholder": "{{goals}}", "dataPath": "$.goals", "type": "list" },
        "assessmentTable": { "placeholder": "{{assessmentTable}}", "dataPath": "$.assessmentTable", "type": "table" }
      }
    }
  },
  "documentRecipes": { "standard_document": [{ "use": "summary", "data": "$" }] }
}"#,
        )
        .map_err(|err| format!("failed to write test manifest: {err}"))?;

        let input = root.join("content.json");
        fs::write(
            &input,
            r#"{
  "title": "Styled DOCX output",
  "goals": ["Goal A", "Goal B"],
  "assessmentTable": {
    "columns": ["Item", "Method"],
    "rows": [["Check", "Observe"]]
  }
}"#,
        )
        .map_err(|err| format!("failed to write test content: {err}"))?;

        let out = root.join("out.docx");
        render_docx(&input, Some(&template_dir), Some("standard_document"), &out)?;
        let document = read_zip_text(&out, "word/document.xml")?;

        assert!(document.contains("<w:t>Styled DOCX output</w:t>"));
        assert!(document.contains(r#"<w:pStyle w:val="ListParagraph"/>"#));
        assert!(document.contains(r#"<w:jc w:val="center"/>"#));
        assert!(
            document
                .contains(r#"<w:rPr><w:b/><w:color w:val="2F5597"/></w:rPr><w:t>• Goal A</w:t>"#)
        );
        assert!(document.contains(r#"<w:pStyle w:val="TableText"/>"#));
        assert!(document.contains(r#"<w:rPr><w:i/></w:rPr><w:t>Check</w:t>"#));
        assert!(!document.contains("{{goals}}"));
        assert!(!document.contains("{{assessmentTable}}"));
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn render_docx_table_placeholder_uses_table_prototype_styles() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-docx-table-{}", uuid::Uuid::new_v4()));
        let template_dir = root.join("template");
        fs::create_dir_all(&template_dir)
            .map_err(|err| format!("failed to create test template dir: {err}"))?;

        let template_file = template_dir.join("template.docx");
        let mut zip = open_zip(&template_file)?;
        zip_file(
            &mut zip,
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#,
        )?;
        zip_file(
            &mut zip,
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#,
        )?;
        zip_file(
            &mut zip,
            "word/document.xml",
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:tbl>
      <w:tblPr><w:tblStyle w:val="FancyTable"/><w:tblW w:w="0" w:type="auto"/><w:tblLook w:firstRow="1" w:lastRow="0" w:firstColumn="1" w:lastColumn="0"/></w:tblPr>
      <w:tblGrid><w:gridCol w:w="2400"/><w:gridCol w:w="4200"/></w:tblGrid>
      <w:tr><w:trPr><w:tblHeader/></w:trPr>
        <w:tc><w:tcPr><w:tcW w:w="2400" w:type="dxa"/><w:shd w:fill="DDEBF7"/></w:tcPr><w:p><w:pPr><w:pStyle w:val="HeaderCell"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Prototype Header A</w:t></w:r></w:p></w:tc>
        <w:tc><w:tcPr><w:tcW w:w="4200" w:type="dxa"/><w:shd w:fill="DDEBF7"/></w:tcPr><w:p><w:pPr><w:pStyle w:val="HeaderCell"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Prototype Header B</w:t></w:r></w:p></w:tc>
      </w:tr>
      <w:tr><w:trPr><w:cantSplit/></w:trPr>
        <w:tc><w:tcPr><w:tcW w:w="2400" w:type="dxa"/><w:shd w:fill="FFFFFF"/></w:tcPr><w:p><w:pPr><w:pStyle w:val="BodyCell"/></w:pPr><w:r><w:rPr><w:color w:val="404040"/></w:rPr><w:t>{{assessmentTable}}</w:t></w:r></w:p></w:tc>
        <w:tc><w:tcPr><w:tcW w:w="4200" w:type="dxa"/><w:shd w:fill="FFFFFF"/></w:tcPr><w:p><w:pPr><w:pStyle w:val="BodyCell"/></w:pPr><w:r><w:rPr><w:color w:val="404040"/></w:rPr><w:t>Prototype Body B</w:t></w:r></w:p></w:tc>
      </w:tr>
    </w:tbl>
  </w:body>
</w:document>"#,
        )?;
        finish_zip(zip, &template_file)?;

        fs::write(
            template_dir.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "test-docx-table-prototype",
  "name": "Test DOCX Table Prototype",
  "format": "docx",
  "entry": "template.docx",
  "input": { "formats": ["json"] },
  "blockTemplates": {
    "summary": {
      "bindings": {
        "assessmentTable": {
          "placeholder": "{{assessmentTable}}",
          "dataPath": "$.assessmentTable",
          "type": "table"
        }
      }
    }
  },
  "documentRecipes": { "standard_document": [{ "use": "summary", "data": "$" }] }
}"#,
        )
        .map_err(|err| format!("failed to write test manifest: {err}"))?;

        let input = root.join("content.json");
        fs::write(
            &input,
            r#"{
  "title": "Prototype table output",
  "assessmentTable": {
    "columns": ["Item", "Method"],
    "rows": [["Check", "Observe"]]
  }
}"#,
        )
        .map_err(|err| format!("failed to write test content: {err}"))?;

        let out = root.join("out.docx");
        render_docx(&input, Some(&template_dir), Some("standard_document"), &out)?;
        let document = read_zip_text(&out, "word/document.xml")?;

        assert!(document.contains(r#"<w:tblStyle w:val="FancyTable"/>"#));
        assert!(document.contains(r#"<w:tblHeader/>"#));
        assert!(document.contains(r#"<w:cantSplit/>"#));
        assert!(document.contains(r#"<w:shd w:fill="DDEBF7"/>"#));
        assert!(document.contains(r#"<w:shd w:fill="FFFFFF"/>"#));
        assert!(document.contains(
            r#"<w:pStyle w:val="HeaderCell"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>Item</w:t>"#
        ));
        assert!(document.contains(
            r#"<w:pStyle w:val="BodyCell"/></w:pPr><w:r><w:rPr><w:color w:val="404040"/></w:rPr><w:t>Check</w:t>"#
        ));
        assert!(!document.contains("{{assessmentTable}}"));
        assert!(!document.contains("Prototype Header A"));
        assert!(!document.contains("Prototype Body B"));
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn render_xlsx_named_ranges_preserve_style_attrs() -> Result<(), String> {
        let root = std::env::temp_dir().join(format!("rdeckforge-xlsx-{}", uuid::Uuid::new_v4()));
        let template_dir = root.join("template");
        fs::create_dir_all(&template_dir)
            .map_err(|err| format!("failed to create test template dir: {err}"))?;

        let template_file = template_dir.join("template.xlsx");
        let mut zip = open_zip(&template_file)?;
        zip_file(
            &mut zip,
            "[Content_Types].xml",
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#,
        )?;
        zip_file(
            &mut zip,
            "_rels/.rels",
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
        )?;
        zip_file(
            &mut zip,
            "xl/workbook.xml",
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets><sheet name="Report" sheetId="1" r:id="rId1"/></sheets>
  <definedNames>
    <definedName name="TitleCell">'Report'!$B$2</definedName>
    <definedName name="GoalsStart">'Report'!$B$4</definedName>
    <definedName name="AssessmentStart">'Report'!$A$9</definedName>
  </definedNames>
</workbook>"#,
        )?;
        zip_file(
            &mut zip,
            "xl/_rels/workbook.xml.rels",
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
        )?;
        zip_file(
            &mut zip,
            "xl/worksheets/sheet1.xml",
            r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main"><sheetData>
  <row r="2" ht="28" customHeight="1"><c r="B2" s="2" t="inlineStr"><is><t>{{title}}</t></is></c></row>
  <row r="4" ht="21" customHeight="1"><c r="B4" s="3" t="inlineStr"><is><t>{{goal}}</t></is></c></row>
  <row r="9" ht="19" customHeight="1"><c r="A9" s="4" t="inlineStr"><is><t>{{a}}</t></is></c><c r="B9" s="4" t="inlineStr"><is><t>{{b}}</t></is></c><c r="C9" s="5" t="inlineStr"><is><t>{{c}}</t></is></c></row>
  <row r="20"><c r="A20" t="inlineStr"><is><t>Footer</t></is></c></row>
</sheetData></worksheet>"#,
        )?;
        finish_zip(zip, &template_file)?;

        fs::write(
            template_dir.join("template.manifest.json"),
            r#"{
  "schemaVersion": "1.0",
  "templateId": "test-xlsx-named-style",
  "name": "Test XLSX Named Style",
  "format": "xlsx",
  "entry": "template.xlsx",
  "input": { "formats": ["json"] },
  "sheetTemplates": {
    "report": {
      "sourceSheet": "Report",
      "bindings": {
        "title": { "namedRange": "TitleCell", "dataPath": "$.title", "type": "text" },
        "goals": { "namedRange": "GoalsStart", "dataPath": "$.goals", "type": "list" },
        "assessmentTable": { "namedRange": "AssessmentStart", "dataPath": "$.assessmentTable", "type": "table" }
      }
    }
  },
  "workbookRecipes": { "standard_workbook": [{ "use": "report", "data": "$" }] }
}"#,
        )
        .map_err(|err| format!("failed to write test manifest: {err}"))?;

        let input = root.join("content.json");
        fs::write(
            &input,
            r#"{
  "title": "Named range output",
  "goals": ["Goal A", "Goal B"],
  "assessmentTable": {
    "columns": ["Col A", "Col B", "Col C"],
    "rows": [["A1", "B1", "C1"], ["A2", "B2", "C2"]]
  }
}"#,
        )
        .map_err(|err| format!("failed to write test content: {err}"))?;

        let out = root.join("out.xlsx");
        render_xlsx(&input, Some(&template_dir), Some("standard_workbook"), &out)?;
        let sheet = read_zip_text(&out, "xl/worksheets/sheet1.xml")?;

        assert!(
            sheet.contains(
                r#"<c r="B2" s="2" t="inlineStr"><is><t>Named range output</t></is></c>"#
            )
        );
        assert!(sheet.contains(r#"<row r="5" ht="21" customHeight="1"><c r="B5" s="3" t="inlineStr"><is><t>Goal B</t></is></c></row>"#));
        assert!(sheet.contains(r#"<row r="11" ht="19" customHeight="1"><c r="A11" s="4" t="inlineStr"><is><t>A2</t></is></c><c r="B11" s="4" t="inlineStr"><is><t>B2</t></is></c><c r="C11" s="5" t="inlineStr"><is><t>C2</t></is></c></row>"#));
        assert!(sheet.contains("Footer"));
        fs::remove_dir_all(&root).ok();
        Ok(())
    }

    #[test]
    fn render_xlsx_from_assessment_json_without_template() -> Result<(), String> {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-assessment-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).map_err(|err| format!("failed to create test dir: {err}"))?;
        let input = root.join("assessment.json");
        fs::write(
            &input,
            r#"{
  "title": "功能点评估",
  "detailed_rows": [
    {
      "module": "模块A",
      "feature": "新增列表页",
      "change_type": "全新页面",
      "effort": 1.5,
      "backend_dependency": "是",
      "backend_notes": "需要列表接口",
      "owner": "张三",
      "progress": "未开始",
      "notes": "复用现有组件"
    },
    {
      "module": "模块B",
      "feature": "权限按钮",
      "change_type": "现有页面增量改造",
      "effort": 0.5,
      "backend_dependency": "否",
      "backend_notes": "",
      "owner": "",
      "progress": "未开始",
      "notes": "前端控制"
    }
  ],
  "summary": {
    "evaluation_basis": "基于需求和代码基线估算。",
    "assumptions": "后端接口按期提供。",
    "out_of_scope": "不含上线陪跑。"
  }
}"#,
        )
        .map_err(|err| format!("failed to write test json: {err}"))?;
        let out = root.join("assessment.xlsx");

        render_xlsx(&input, None, None, &out)?;
        let workbook = read_zip_text(&out, "xl/workbook.xml")?;
        let sheet = read_zip_text(&out, "xl/worksheets/sheet1.xml")?;
        let styles = read_zip_text(&out, "xl/styles.xml")?;

        assert!(workbook.contains(r#"<sheet name="前端评估" sheetId="1" r:id="rId1"/>"#));
        assert!(sheet.contains("模块A"));
        assert!(sheet.contains("新增列表页"));
        assert!(sheet.contains(r#"<c r="D2" s="2" t="n"><v>1.5</v></c>"#));
        assert!(sheet.contains("<f>SUM(D2:D3)</f>"));
        assert!(sheet.contains("评估口径"));
        assert!(sheet.contains("基于需求和代码基线估算。"));
        assert!(sheet.contains(r#"<mergeCell ref="B8:I8"/>"#));
        assert!(styles.contains(r#"<fgColor rgb="001F4E78"/>"#));
        fs::remove_dir_all(&root).ok();
        Ok(())
    }
}
