use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
    time::Instant,
};
use zip::ZipArchive;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateTestReport {
    pub template_id: String,
    pub template_name: String,
    pub template_dir: String,
    pub format: String,
    pub output_dir: String,
    pub selected_count: usize,
    pub passed_count: usize,
    pub failed_count: usize,
    pub passed: bool,
    pub readiness: crate::environment::DiagnosticStatus,
    pub can_render: bool,
    pub cases: Vec<TemplateTestCaseResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateTestCaseResult {
    pub id: String,
    pub input_file: String,
    pub output_file: String,
    pub recipe: Option<String>,
    pub duration_ms: u128,
    pub passed: bool,
    pub error: Option<String>,
    pub assertions: Vec<TemplateAssertionResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TemplateAssertionResult {
    pub id: String,
    pub passed: bool,
    pub message: String,
}

pub fn run_template_tests(
    template_dir: &Path,
    case_id: Option<&str>,
    output_dir: Option<&Path>,
) -> Result<TemplateTestReport, String> {
    let template_dir = normalize_template_dir(template_dir)?;
    let validation = crate::template_manifest::validate_template_pack(&template_dir)?;
    let manifest = crate::template_manifest::load_manifest(&template_dir)?;
    if manifest.test_cases.is_empty() {
        return Err(format!(
            "template '{}' does not declare any tests",
            manifest.template_id
        ));
    }
    let selected = manifest
        .test_cases
        .iter()
        .filter(|test_case| case_id.is_none_or(|id| test_case.id == id))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(format!(
            "template test case was not found: {}",
            case_id.unwrap_or_default()
        ));
    }
    let output_dir = match output_dir {
        Some(output_dir) => absolutize(output_dir),
        None => std::env::temp_dir().join(format!(
            "rdeckforge-template-tests-{}-{}",
            sanitize_file_name(&manifest.template_id),
            uuid::Uuid::new_v4()
        )),
    };
    fs::create_dir_all(&output_dir)
        .map_err(|err| format!("failed to create test output directory: {err}"))?;
    let readiness = crate::template_readiness::diagnose_template_readiness(&template_dir)?;
    let uses_script_renderer = validation.template_type == "script"
        || (validation.template_type == "hybrid" && manifest.renderer.is_some());

    let mut cases = Vec::new();
    for test_case in selected {
        let started = Instant::now();
        let input = template_dir.join(&test_case.input);
        let output_name = test_case.output_name.clone().unwrap_or_else(|| {
            format!("{}.{}", sanitize_file_name(&test_case.id), manifest.format)
        });
        let output = output_dir.join(output_name);
        if output.exists() {
            fs::remove_file(&output)
                .map_err(|err| format!("failed to replace '{}': {err}", output.display()))?;
        }
        let render_result = if readiness.can_render {
            render_test_case(
                &manifest,
                &template_dir,
                &input,
                &output,
                test_case.recipe.as_deref(),
                uses_script_renderer,
            )
        } else {
            Err(readiness.summary.clone())
        };
        let (assertions, error) = match render_result {
            Ok(()) => (
                evaluate_assertions(&manifest.format, &output, &test_case.assertions),
                None,
            ),
            Err(error) => (
                vec![TemplateAssertionResult {
                    id: "render".to_string(),
                    passed: false,
                    message: "模板测试渲染失败。".to_string(),
                }],
                Some(error),
            ),
        };
        let passed = error.is_none() && assertions.iter().all(|assertion| assertion.passed);
        cases.push(TemplateTestCaseResult {
            id: test_case.id.clone(),
            input_file: input.display().to_string(),
            output_file: output.display().to_string(),
            recipe: test_case.recipe.clone(),
            duration_ms: started.elapsed().as_millis(),
            passed,
            error,
            assertions,
        });
    }

    let passed_count = cases.iter().filter(|case| case.passed).count();
    let failed_count = cases.len() - passed_count;
    Ok(TemplateTestReport {
        template_id: validation.template_id,
        template_name: validation.name,
        template_dir: template_dir.display().to_string(),
        format: validation.format,
        output_dir: output_dir.display().to_string(),
        selected_count: cases.len(),
        passed_count,
        failed_count,
        passed: failed_count == 0,
        readiness: readiness.readiness,
        can_render: readiness.can_render,
        cases,
    })
}

fn render_test_case(
    manifest: &crate::template_manifest::TemplateManifest,
    template_dir: &Path,
    input: &Path,
    output: &Path,
    recipe: Option<&str>,
    uses_script_renderer: bool,
) -> Result<(), String> {
    crate::validator::require_file(input, "template test input")?;
    if uses_script_renderer {
        crate::script_render::render_script_template(input, template_dir, output)?;
        return Ok(());
    }
    match manifest.format.as_str() {
        "pptx" => {
            let job = crate::render_job::build_pptx_job(template_dir, input, None, output, recipe)?;
            crate::renderer::render_pptx_job_atomic(&job, output)?;
        }
        "docx" => {
            crate::office_export::render_docx_atomic(input, Some(template_dir), recipe, output)?;
        }
        "xlsx" => {
            crate::office_export::render_xlsx_atomic(input, Some(template_dir), recipe, output)?;
        }
        other => return Err(format!("unsupported template test format: {other}")),
    }
    crate::script_render::run_pipeline_steps(template_dir, &manifest.pipeline, input, output)?;
    Ok(())
}

fn evaluate_assertions(
    format: &str,
    output: &Path,
    expected: &crate::template_manifest::TemplateTestAssertions,
) -> Vec<TemplateAssertionResult> {
    let mut assertions = Vec::new();
    let metadata = fs::metadata(output);
    assertions.push(match metadata.as_ref() {
        Ok(metadata) if metadata.is_file() => TemplateAssertionResult {
            id: "output_exists".to_string(),
            passed: true,
            message: format!("输出文件存在，共 {} 字节。", metadata.len()),
        },
        _ => TemplateAssertionResult {
            id: "output_exists".to_string(),
            passed: false,
            message: "没有生成输出文件。".to_string(),
        },
    });
    let minimum_size = expected.min_size_bytes.unwrap_or(1);
    let actual_size = metadata.as_ref().map(|value| value.len()).unwrap_or(0);
    assertions.push(TemplateAssertionResult {
        id: "min_size_bytes".to_string(),
        passed: actual_size >= minimum_size,
        message: format!("输出大小 {actual_size} 字节，要求至少 {minimum_size} 字节。"),
    });

    if !expected.validate_package.unwrap_or(true) {
        if !expected.required_text.is_empty() || !expected.forbidden_text.is_empty() {
            assertions.push(TemplateAssertionResult {
                id: "semantic_text_requires_package".to_string(),
                passed: false,
                message: "文本语义断言需要启用 Office 包校验。".to_string(),
            });
        }
        return assertions;
    }
    match inspect_office_package(output, format) {
        Ok(package) => {
            assertions.push(TemplateAssertionResult {
                id: "office_package".to_string(),
                passed: true,
                message: "输出是可读取的 Office ZIP 包。".to_string(),
            });
            for entry in default_required_entries(format)
                .into_iter()
                .chain(expected.required_zip_entries.iter().map(String::as_str))
            {
                assertions.push(TemplateAssertionResult {
                    id: format!("zip_entry:{entry}"),
                    passed: package.entries.contains(entry),
                    message: if package.entries.contains(entry) {
                        format!("ZIP 条目存在：{entry}")
                    } else {
                        format!("ZIP 条目缺失：{entry}")
                    },
                });
            }
            push_range_assertions(
                &mut assertions,
                "slides",
                package.slide_count,
                expected.min_slides,
                expected.max_slides,
            );
            push_range_assertions(
                &mut assertions,
                "sheets",
                package.sheet_count,
                expected.min_sheets,
                expected.max_sheets,
            );
            push_text_assertions(
                &mut assertions,
                &package.visible_text,
                &expected.required_text,
                &expected.forbidden_text,
            );
        }
        Err(error) => assertions.push(TemplateAssertionResult {
            id: "office_package".to_string(),
            passed: false,
            message: error,
        }),
    }
    assertions
}

fn push_text_assertions(
    assertions: &mut Vec<TemplateAssertionResult>,
    visible_text: &str,
    required: &[String],
    forbidden: &[String],
) {
    let corpus = normalize_semantic_text(visible_text);
    for (index, expected) in required.iter().enumerate() {
        let needle = normalize_semantic_text(expected);
        let passed = !needle.is_empty() && corpus.contains(&needle);
        assertions.push(TemplateAssertionResult {
            id: format!("required_text:{}", index + 1),
            passed,
            message: if passed {
                format!("输出包含必需文本：{expected}")
            } else {
                format!("输出缺少必需文本：{expected}")
            },
        });
    }
    for (index, unexpected) in forbidden.iter().enumerate() {
        let needle = normalize_semantic_text(unexpected);
        let passed = needle.is_empty() || !corpus.contains(&needle);
        assertions.push(TemplateAssertionResult {
            id: format!("forbidden_text:{}", index + 1),
            passed,
            message: if passed {
                format!("输出未包含禁用文本：{unexpected}")
            } else {
                format!("输出包含禁用文本：{unexpected}")
            },
        });
    }
}

fn normalize_semantic_text(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn push_range_assertions(
    assertions: &mut Vec<TemplateAssertionResult>,
    label: &str,
    actual: usize,
    minimum: Option<usize>,
    maximum: Option<usize>,
) {
    if let Some(minimum) = minimum {
        assertions.push(TemplateAssertionResult {
            id: format!("min_{label}"),
            passed: actual >= minimum,
            message: format!("实际 {label} 数量为 {actual}，要求至少 {minimum}。"),
        });
    }
    if let Some(maximum) = maximum {
        assertions.push(TemplateAssertionResult {
            id: format!("max_{label}"),
            passed: actual <= maximum,
            message: format!("实际 {label} 数量为 {actual}，要求至多 {maximum}。"),
        });
    }
}

struct OfficePackageInspection {
    entries: BTreeSet<String>,
    slide_count: usize,
    sheet_count: usize,
    visible_text: String,
}

fn inspect_office_package(output: &Path, format: &str) -> Result<OfficePackageInspection, String> {
    let file = File::open(output)
        .map_err(|err| format!("无法打开 Office 输出 '{}': {err}", output.display()))?;
    let mut archive =
        ZipArchive::new(file).map_err(|err| format!("输出不是有效的 Office ZIP 包：{err}"))?;
    let mut entries = BTreeSet::new();
    let mut visible_text = String::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| format!("无法读取 Office ZIP 条目 {index}：{err}"))?;
        let name = entry.name().to_string();
        if office_text_entry(format, &name) {
            let mut xml = String::new();
            entry
                .read_to_string(&mut xml)
                .map_err(|err| format!("无法读取 Office 文本条目 '{name}'：{err}"))?;
            let document = roxmltree::Document::parse(&xml)
                .map_err(|err| format!("无法解析 Office 文本条目 '{name}'：{err}"))?;
            for text in document.descendants().filter_map(|node| node.text()) {
                visible_text.push_str(text);
                visible_text.push(' ');
            }
        }
        entries.insert(name);
    }
    let slide_count = entries
        .iter()
        .filter(|entry| numbered_xml_entry(entry, "ppt/slides/slide"))
        .count();
    let sheet_count = entries
        .iter()
        .filter(|entry| numbered_xml_entry(entry, "xl/worksheets/sheet"))
        .count();
    Ok(OfficePackageInspection {
        entries,
        slide_count,
        sheet_count,
        visible_text,
    })
}

fn office_text_entry(format: &str, entry: &str) -> bool {
    match format {
        "pptx" => {
            numbered_xml_entry(entry, "ppt/slides/slide")
                || numbered_xml_entry(entry, "ppt/charts/chart")
        }
        "docx" => {
            entry == "word/document.xml"
                || numbered_xml_entry(entry, "word/header")
                || numbered_xml_entry(entry, "word/footer")
                || matches!(entry, "word/footnotes.xml" | "word/endnotes.xml")
        }
        "xlsx" => {
            entry == "xl/sharedStrings.xml" || numbered_xml_entry(entry, "xl/worksheets/sheet")
        }
        _ => false,
    }
}

fn numbered_xml_entry(entry: &str, prefix: &str) -> bool {
    entry
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(".xml"))
        .is_some_and(|value| {
            !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
        })
}

fn default_required_entries(format: &str) -> Vec<&'static str> {
    match format {
        "pptx" => vec!["[Content_Types].xml", "ppt/presentation.xml"],
        "docx" => vec!["[Content_Types].xml", "word/document.xml"],
        "xlsx" => vec!["[Content_Types].xml", "xl/workbook.xml"],
        _ => Vec::new(),
    }
}

fn normalize_template_dir(template_dir: &Path) -> Result<PathBuf, String> {
    let template_dir = absolutize(template_dir);
    crate::validator::require_dir(&template_dir, "template pack")?;
    template_dir
        .canonicalize()
        .map_err(|err| format!("failed to normalize template pack path: {err}"))
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

fn sanitize_file_name(value: &str) -> String {
    let value = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    let value = value.trim_matches('-');
    if value.is_empty() {
        "template-test".to_string()
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::{CompressionMethod, ZipWriter, write::FileOptions};

    #[test]
    fn office_assertions_count_slides_and_required_entries() {
        let root =
            std::env::temp_dir().join(format!("rdeckforge-self-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&root).unwrap();
        let output = root.join("sample.pptx");
        let file = File::create(&output).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Stored);
        for entry in [
            "[Content_Types].xml",
            "ppt/presentation.xml",
            "ppt/slides/slide1.xml",
            "ppt/slides/slide2.xml",
        ] {
            writer.start_file(entry, options).unwrap();
            writer.write_all(b"<xml/>").unwrap();
        }
        writer.finish().unwrap();
        let expected = crate::template_manifest::TemplateTestAssertions {
            validate_package: Some(true),
            min_size_bytes: Some(10),
            min_slides: Some(2),
            max_slides: Some(2),
            min_sheets: None,
            max_sheets: None,
            required_zip_entries: Vec::new(),
            required_text: Vec::new(),
            forbidden_text: Vec::new(),
        };

        let assertions = evaluate_assertions("pptx", &output, &expected);

        assert!(assertions.iter().all(|assertion| assertion.passed));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn office_assertions_find_text_split_across_pptx_runs() {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-self-test-text-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let output = root.join("sample.pptx");
        let file = File::create(&output).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Stored);
        for (entry, body) in [
            ("[Content_Types].xml", "<Types/>"),
            ("ppt/presentation.xml", "<p:presentation xmlns:p=\"p\"/>"),
            (
                "ppt/slides/slide1.xml",
                "<p:sld xmlns:p=\"p\" xmlns:a=\"a\"><a:t>护理质量</a:t><a:t>改进方法</a:t></p:sld>",
            ),
        ] {
            writer.start_file(entry, options).unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        let expected = crate::template_manifest::TemplateTestAssertions {
            validate_package: Some(true),
            min_size_bytes: Some(10),
            min_slides: Some(1),
            max_slides: Some(1),
            min_sheets: None,
            max_sheets: None,
            required_zip_entries: Vec::new(),
            required_text: vec!["护理质量改进方法".to_string()],
            forbidden_text: vec!["未替换占位符".to_string()],
        };

        let assertions = evaluate_assertions("pptx", &output, &expected);

        assert!(assertions.iter().all(|assertion| assertion.passed));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn office_assertions_fail_when_required_text_is_missing() {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-self-test-missing-text-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        let output = root.join("sample.docx");
        let file = File::create(&output).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::default().compression_method(CompressionMethod::Stored);
        for (entry, body) in [
            ("[Content_Types].xml", "<Types/>"),
            (
                "word/document.xml",
                "<w:document xmlns:w=\"w\"><w:t>已有正文</w:t></w:document>",
            ),
        ] {
            writer.start_file(entry, options).unwrap();
            writer.write_all(body.as_bytes()).unwrap();
        }
        writer.finish().unwrap();
        let expected = crate::template_manifest::TemplateTestAssertions {
            validate_package: Some(true),
            min_size_bytes: Some(10),
            min_slides: None,
            max_slides: None,
            min_sheets: None,
            max_sheets: None,
            required_zip_entries: Vec::new(),
            required_text: vec!["缺失标题".to_string()],
            forbidden_text: Vec::new(),
        };

        let assertions = evaluate_assertions("docx", &output, &expected);

        assert!(
            assertions
                .iter()
                .any(|assertion| { assertion.id == "required_text:1" && !assertion.passed })
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn office_assertions_extract_docx_and_xlsx_visible_text() {
        for (format, entries, required) in [
            (
                "docx",
                vec![
                    ("[Content_Types].xml", "<Types/>"),
                    (
                        "word/document.xml",
                        "<w:document xmlns:w=\"w\"><w:t>技术设计</w:t><w:t>文档</w:t></w:document>",
                    ),
                ],
                "技术设计文档",
            ),
            (
                "xlsx",
                vec![
                    ("[Content_Types].xml", "<Types/>"),
                    ("xl/workbook.xml", "<workbook/>"),
                    (
                        "xl/sharedStrings.xml",
                        "<sst><si><t>功能点评估</t></si></sst>",
                    ),
                    (
                        "xl/worksheets/sheet1.xml",
                        "<worksheet><sheetData/></worksheet>",
                    ),
                ],
                "功能点评估",
            ),
        ] {
            let root = std::env::temp_dir().join(format!(
                "rdeckforge-self-test-{format}-text-{}",
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&root).unwrap();
            let output = root.join(format!("sample.{format}"));
            let file = File::create(&output).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::default().compression_method(CompressionMethod::Stored);
            for (entry, body) in entries {
                writer.start_file(entry, options).unwrap();
                writer.write_all(body.as_bytes()).unwrap();
            }
            writer.finish().unwrap();
            let expected = crate::template_manifest::TemplateTestAssertions {
                validate_package: Some(true),
                min_size_bytes: Some(10),
                min_slides: None,
                max_slides: None,
                min_sheets: None,
                max_sheets: None,
                required_zip_entries: Vec::new(),
                required_text: vec![required.to_string()],
                forbidden_text: Vec::new(),
            };

            let assertions = evaluate_assertions(format, &output, &expected);

            assert!(assertions.iter().all(|assertion| assertion.passed));
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn template_without_tests_returns_a_clear_error() {
        let root = std::env::temp_dir().join(format!(
            "rdeckforge-self-test-empty-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(
            root.join("template.manifest.json"),
            r#"{
              "schemaVersion": "1.0",
              "templateId": "empty-test-pack",
              "name": "Empty Test Pack",
              "format": "pptx",
              "templateType": "script",
              "input": { "formats": ["json"] },
              "renderer": {
                "type": "script.command",
                "runtime": "missing",
                "entry": "missing"
              }
            }"#,
        )
        .unwrap();

        let error = run_template_tests(&root, None, None).unwrap_err();

        assert!(error.contains("does not declare any tests"));
        fs::remove_dir_all(root).unwrap();
    }
}
