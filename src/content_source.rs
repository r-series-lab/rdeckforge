use serde::Serialize;
use serde_json::{Value, json};
use std::{
    fs,
    path::{Path, PathBuf},
};

const CONTENT_PACK_ENTRIES: &[&str] = &["content.json", "content.md", "outline.md"];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentSource {
    pub input_format: String,
    pub value: Value,
    pub source_path: Option<PathBuf>,
    pub content_root: Option<PathBuf>,
}

pub fn load_content_source(path: &Path) -> Result<ContentSource, String> {
    let source_path = resolve_content_source_path(path)?;
    let raw = fs::read_to_string(&source_path).map_err(|err| {
        format!(
            "failed to read content source '{}': {err}",
            source_path.display()
        )
    })?;
    let mut source = parse_content_source_text(&raw, &infer_format(&source_path))?;
    source.content_root = source_path.parent().map(Path::to_path_buf);
    source.source_path = Some(source_path);
    Ok(source)
}

pub fn parse_content_source_text(raw: &str, input_format: &str) -> Result<ContentSource, String> {
    let normalized_format = normalize_format(input_format);
    let value = match normalized_format.as_str() {
        "json" => {
            serde_json::from_str(raw).map_err(|err| format!("invalid content JSON: {err}"))?
        }
        "md" | "markdown" => parse_markdown_value(raw),
        other => return Err(format!("unsupported content input format: {other}")),
    };
    Ok(ContentSource {
        input_format: normalized_format,
        value,
        source_path: None,
        content_root: None,
    })
}

pub fn apply_md_profile(
    source: ContentSource,
    md_profile: Option<&str>,
) -> Result<ContentSource, String> {
    if source.input_format != "md" {
        return Ok(source);
    }

    match md_profile {
        Some("teaching_outline_v1") => {
            let ContentSource {
                input_format,
                value,
                source_path,
                content_root,
            } = source;
            Ok(ContentSource {
                input_format,
                value: markdown_to_teaching_deck(value)?,
                source_path,
                content_root,
            })
        }
        Some("design_doc_v1") => {
            let ContentSource {
                input_format,
                value,
                source_path,
                content_root,
            } = source;
            Ok(ContentSource {
                input_format,
                value: markdown_to_design_doc(value)?,
                source_path,
                content_root,
            })
        }
        Some(other) => Err(format!("unsupported markdown profile: {other}")),
        None => Ok(source),
    }
}

pub fn resolve_content_source_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_file() {
        return Ok(path.to_path_buf());
    }
    if path.is_dir() {
        for entry in CONTENT_PACK_ENTRIES {
            let candidate = path.join(entry);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
        return Err(format!(
            "content pack '{}' is missing one of: {}",
            path.display(),
            CONTENT_PACK_ENTRIES.join(", ")
        ));
    }
    Err(format!("content source not found: {}", path.display()))
}

pub fn infer_format(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .map(normalize_format)
        .unwrap_or_else(|| "json".to_string())
}

pub fn normalize_format(value: &str) -> String {
    match value
        .trim()
        .trim_start_matches('.')
        .to_ascii_lowercase()
        .as_str()
    {
        "markdown" => "md".to_string(),
        "json" => "json".to_string(),
        "md" => "md".to_string(),
        other => other.to_string(),
    }
}

fn parse_markdown_value(raw: &str) -> Value {
    let (metadata, body) = split_front_matter(raw);
    let title = metadata
        .get("title")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| first_markdown_heading(&body))
        .unwrap_or_else(|| "Untitled Markdown".to_string());
    let sections = parse_markdown_sections(&body);

    json!({
        "title": title,
        "markdown": body,
        "metadata": metadata,
        "sections": sections
    })
}

fn markdown_to_teaching_deck(value: Value) -> Result<Value, String> {
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Untitled Markdown")
        .to_string();
    let metadata = value
        .get("metadata")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let sections = value
        .get("sections")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if sections.is_empty() {
        return Err("teaching_outline_v1 markdown needs at least one ## section".to_string());
    }

    let goals = metadata
        .get("goals")
        .and_then(Value::as_str)
        .map(split_inline_list)
        .unwrap_or_else(|| {
            sections
                .iter()
                .filter_map(|section| section.get("title").and_then(Value::as_str))
                .take(4)
                .map(|title| format!("掌握{title}"))
                .collect()
        });

    let chapters = sections
        .iter()
        .enumerate()
        .map(|(index, section)| {
            let section_title = section
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("未命名章节");
            let body = section
                .get("body")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let paragraphs = split_paragraphs(body);
            let items = if paragraphs.is_empty() {
                vec![json!({
                    "heading": section_title,
                    "body": ""
                })]
            } else {
                paragraphs
                    .into_iter()
                    .take(4)
                    .enumerate()
                    .map(|(item_index, paragraph)| {
                        json!({
                            "heading": format!("要点 {}", item_index + 1),
                            "body": paragraph
                        })
                    })
                    .collect()
            };
            json!({
                "id": format!("chapter-{}", index + 1),
                "label": section_title,
                "title": section_title,
                "contentTitle": section_title,
                "items": items
            })
        })
        .collect::<Vec<_>>();

    let summary = metadata
        .get("summary")
        .and_then(Value::as_str)
        .map(split_inline_list)
        .unwrap_or_else(|| {
            vec![
                format!("围绕《{title}》完成核心内容回顾"),
                "结合课堂提问或案例反馈检验学习效果".to_string(),
            ]
        });

    let mut deck = serde_json::Map::new();
    deck.insert("schemaVersion".to_string(), json!("1.0"));
    deck.insert("documentType".to_string(), json!("teaching_deck"));
    deck.insert("title".to_string(), json!(title));
    if let Some(speaker) = metadata.get("author").and_then(Value::as_str) {
        deck.insert("speaker".to_string(), json!(speaker));
    }
    if let Some(date) = metadata.get("date").and_then(Value::as_str) {
        deck.insert("date".to_string(), json!(date));
    }
    deck.insert("goals".to_string(), json!(goals));
    deck.insert("chapters".to_string(), json!(chapters));
    deck.insert("summary".to_string(), json!(summary));
    deck.insert("evaluations".to_string(), json!([]));

    Ok(Value::Object(deck))
}

fn markdown_to_design_doc(value: Value) -> Result<Value, String> {
    let title = value
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("Untitled Markdown")
        .to_string();
    let markdown = value
        .get("markdown")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let metadata = value
        .get("metadata")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let blocks = parse_design_doc_blocks(markdown, &title);
    if blocks.is_empty() {
        return Err(
            "design_doc_v1 markdown needs at least one heading, paragraph, or list item"
                .to_string(),
        );
    }

    Ok(json!({
        "schemaVersion": "1.0",
        "documentType": "design_doc",
        "title": title,
        "metadata": metadata,
        "blocks": blocks
    }))
}

fn parse_design_doc_blocks(raw: &str, title: &str) -> Vec<Value> {
    let mut blocks = Vec::new();
    let mut paragraph = Vec::new();
    let mut code_lines = Vec::new();
    let mut code_language = String::new();
    let mut in_code = false;

    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(language) = trimmed.strip_prefix("```") {
            flush_design_paragraph(&mut blocks, &mut paragraph);
            if in_code {
                blocks.push(json!({
                    "type": "code",
                    "language": code_language,
                    "text": code_lines.join("\n")
                }));
                code_lines.clear();
                code_language.clear();
                in_code = false;
            } else {
                code_language = language.trim().to_string();
                in_code = true;
            }
            continue;
        }
        if in_code {
            code_lines.push(line.to_string());
            continue;
        }
        if trimmed.is_empty() {
            flush_design_paragraph(&mut blocks, &mut paragraph);
            continue;
        }
        if let Some((level, text)) = markdown_heading(trimmed) {
            flush_design_paragraph(&mut blocks, &mut paragraph);
            if !(level == 1 && blocks.is_empty() && text == title) {
                blocks.push(json!({
                    "type": "heading",
                    "level": level,
                    "text": text
                }));
            }
            continue;
        }
        if let Some((level, ordered, text)) = markdown_list_item(line) {
            flush_design_paragraph(&mut blocks, &mut paragraph);
            blocks.push(json!({
                "type": if ordered { "orderedItem" } else { "listItem" },
                "level": level,
                "text": text
            }));
            continue;
        }
        if let Some(text) = trimmed.strip_prefix('>') {
            flush_design_paragraph(&mut blocks, &mut paragraph);
            blocks.push(json!({
                "type": "quote",
                "text": clean_markdown_inline(text.trim())
            }));
            continue;
        }
        paragraph.push(trimmed.to_string());
    }
    if in_code {
        blocks.push(json!({
            "type": "code",
            "language": code_language,
            "text": code_lines.join("\n")
        }));
    }
    flush_design_paragraph(&mut blocks, &mut paragraph);
    blocks
}

fn flush_design_paragraph(blocks: &mut Vec<Value>, paragraph: &mut Vec<String>) {
    if paragraph.is_empty() {
        return;
    }
    blocks.push(json!({
        "type": "paragraph",
        "text": clean_markdown_inline(&paragraph.join(" "))
    }));
    paragraph.clear();
}

fn markdown_heading(line: &str) -> Option<(usize, String)> {
    let level = line.chars().take_while(|ch| *ch == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = line.get(level..)?;
    if !rest.starts_with(' ') {
        return None;
    }
    let text = clean_markdown_inline(rest.trim());
    if text.is_empty() {
        return None;
    }
    Some((level, text))
}

fn markdown_list_item(line: &str) -> Option<(usize, bool, String)> {
    let leading_spaces = line.chars().take_while(|ch| *ch == ' ').count();
    let level = leading_spaces / 2;
    let trimmed = line.trim_start();
    for marker in ["- ", "* "] {
        if let Some(rest) = trimmed.strip_prefix(marker) {
            let text = clean_markdown_inline(rest.trim());
            return (!text.is_empty()).then_some((level, false, text));
        }
    }
    let dot = trimmed.find(". ")?;
    if dot == 0 || !trimmed[..dot].chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let text = clean_markdown_inline(trimmed[dot + 2..].trim());
    (!text.is_empty()).then_some((level, true, text))
}

fn clean_markdown_inline(raw: &str) -> String {
    raw.replace('`', "")
        .replace("**", "")
        .replace("__", "")
        .replace('*', "")
}

fn parse_markdown_sections(raw: &str) -> Vec<Value> {
    let mut sections = Vec::new();
    let mut current_title: Option<String> = None;
    let mut current_body = Vec::new();

    for line in raw.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix("## ") {
            push_markdown_section(&mut sections, &mut current_title, &mut current_body);
            let heading = heading.trim();
            if !heading.is_empty() {
                current_title = Some(heading.to_string());
            }
            continue;
        }
        if current_title.is_some() {
            current_body.push(line.to_string());
        }
    }
    push_markdown_section(&mut sections, &mut current_title, &mut current_body);
    sections
}

fn push_markdown_section(
    sections: &mut Vec<Value>,
    current_title: &mut Option<String>,
    current_body: &mut Vec<String>,
) {
    let Some(title) = current_title.take() else {
        return;
    };
    let body = current_body.join("\n").trim().to_string();
    sections.push(json!({
        "title": title,
        "body": body
    }));
    current_body.clear();
}

fn split_inline_list(raw: &str) -> Vec<String> {
    raw.split([';', '；', ',', '，', '|'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn split_paragraphs(raw: &str) -> Vec<String> {
    raw.split("\n\n")
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(|item| item.lines().map(str::trim).collect::<Vec<_>>().join("\n"))
        .collect()
}

fn split_front_matter(raw: &str) -> (serde_json::Map<String, Value>, String) {
    let mut metadata = serde_json::Map::new();
    let Some(rest) = raw.strip_prefix("---\n") else {
        return (metadata, raw.to_string());
    };
    let Some(end) = rest.find("\n---") else {
        return (metadata, raw.to_string());
    };
    let front_matter = &rest[..end];
    for line in front_matter.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        metadata.insert(
            key.to_string(),
            Value::String(value.trim().trim_matches('"').to_string()),
        );
    }
    let body = rest[end + "\n---".len()..].trim_start().to_string();
    (metadata, body)
}

fn first_markdown_heading(raw: &str) -> Option<String> {
    raw.lines()
        .find_map(|line| line.trim().strip_prefix("# "))
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
}
