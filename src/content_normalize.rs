use serde::Serialize;
use serde_json::{Map, Value, json};
use std::{fs, path::Path};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentNormalizeResult {
    pub input_file: String,
    pub output_file: String,
    pub changed: bool,
    pub fixes: Vec<ContentNormalizeFix>,
    pub validation: crate::content_ir::ContentWorkspaceValidation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentNormalizeFix {
    pub code: String,
    pub path: String,
    pub message: String,
}

pub fn normalize_content_file(
    input: &Path,
    template: Option<&Path>,
    recipe: Option<&str>,
    profile: Option<&str>,
    out: &Path,
) -> Result<ContentNormalizeResult, String> {
    let mut source = crate::content_source::load_content_source(input)?;
    let input_spec = if let Some(template) = template {
        let manifest = crate::template_manifest::load_manifest(template)?;
        Some(crate::template_manifest::normalized_input_spec(&manifest))
    } else if let Some(profile) = profile.filter(|value| !value.trim().is_empty()) {
        Some(crate::profiles::input_spec_for_profile(profile)?)
    } else {
        None
    };

    if let Some(input_spec) = input_spec.as_ref() {
        source = crate::content_source::apply_md_profile(source, input_spec.md_profile.as_deref())?;
    }

    let mut fixes = Vec::new();
    let changed = normalize_value(
        &mut source.value,
        input_spec
            .as_ref()
            .and_then(|input| input.schema_id.as_deref())
            .or(profile),
        &mut fixes,
    );

    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create normalize output directory: {err}"))?;
    }
    let raw = serde_json::to_string_pretty(&source.value)
        .map_err(|err| format!("failed to serialize normalized content: {err}"))?;
    fs::write(out, raw).map_err(|err| {
        format!(
            "failed to write normalized content '{}': {err}",
            out.display()
        )
    })?;

    let validation = crate::content_ir::validate_content_workspace_file_with_profile(
        out, template, recipe, profile,
    )?;

    Ok(ContentNormalizeResult {
        input_file: input.display().to_string(),
        output_file: out.display().to_string(),
        changed,
        fixes,
        validation,
    })
}

fn normalize_value(
    value: &mut Value,
    schema_id: Option<&str>,
    fixes: &mut Vec<ContentNormalizeFix>,
) -> bool {
    let mut changed = false;
    changed |= normalize_common_object(value, fixes);

    match schema_id.unwrap_or_default() {
        "teaching_deck_v1" => {
            changed |= normalize_teaching_deck(value, fixes);
        }
        "design_doc_v1" => {
            changed |= ensure_string_field(value, "$.schemaVersion", "schemaVersion", "1.0", fixes);
            changed |=
                ensure_string_field(value, "$.documentType", "documentType", "design_doc", fixes);
            changed |= ensure_title(value, fixes);
        }
        "feature_assessment_v1" => {
            changed |= ensure_string_field(value, "$.schemaVersion", "schemaVersion", "1.0", fixes);
            changed |= ensure_title(value, fixes);
        }
        _ => {}
    }

    changed
}

fn normalize_common_object(value: &mut Value, fixes: &mut Vec<ContentNormalizeFix>) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let mut changed = false;
    changed |= rename_field(object, "schema_version", "schemaVersion", "$", fixes);
    changed |= rename_field(object, "document_type", "documentType", "$", fixes);
    changed |= rename_field(object, "content_title", "contentTitle", "$", fixes);
    changed
}

fn normalize_teaching_deck(value: &mut Value, fixes: &mut Vec<ContentNormalizeFix>) -> bool {
    let mut changed = false;
    changed |= ensure_string_field(value, "$.schemaVersion", "schemaVersion", "1.0", fixes);
    let document_type = if value.get("pages").and_then(Value::as_array).is_some() {
        "pptx_pages"
    } else {
        "teaching_deck"
    };
    changed |= ensure_string_field(
        value,
        "$.documentType",
        "documentType",
        document_type,
        fixes,
    );
    changed |= ensure_title(value, fixes);
    changed |= string_to_array(value, "goals", "$.goals", fixes);
    changed |= string_to_array(value, "summary", "$.summary", fixes);
    changed |= ensure_array_field(value, "evaluations", "$.evaluations", fixes);
    changed |= normalize_chapters(value, fixes);
    changed |= normalize_evaluations(value, fixes);
    changed
}

fn ensure_title(value: &mut Value, fixes: &mut Vec<ContentNormalizeFix>) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    if object
        .get("title")
        .and_then(Value::as_str)
        .is_some_and(|title| !title.trim().is_empty())
    {
        return false;
    }
    let replacement = object
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| object.get("heading").and_then(Value::as_str))
        .unwrap_or("Untitled content")
        .trim()
        .to_string();
    object.insert("title".to_string(), json!(replacement));
    fixes.push(fix(
        "default_title",
        "$.title",
        "Filled missing title from name/heading or fallback.",
    ));
    true
}

fn ensure_string_field(
    value: &mut Value,
    path: &str,
    key: &str,
    default_value: &str,
    fixes: &mut Vec<ContentNormalizeFix>,
) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    if object
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty())
    {
        return false;
    }
    object.insert(key.to_string(), json!(default_value));
    fixes.push(fix(
        "default_field",
        path,
        &format!("Filled missing {key}."),
    ));
    true
}

fn normalize_chapters(value: &mut Value, fixes: &mut Vec<ContentNormalizeFix>) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let Some(chapters) = object.get_mut("chapters") else {
        return false;
    };
    let mut changed = false;
    if chapters.is_object() {
        let items = chapters
            .as_object()
            .map(|object| object.values().cloned().collect::<Vec<_>>())
            .unwrap_or_default();
        *chapters = Value::Array(items);
        fixes.push(fix(
            "object_to_list",
            "$.chapters",
            "Converted chapters object to array.",
        ));
        changed = true;
    }

    let Some(chapter_items) = chapters.as_array_mut() else {
        return changed;
    };

    for (index, chapter) in chapter_items.iter_mut().enumerate() {
        let path = format!("$.chapters[{index}]");
        if chapter.is_string() {
            let title = chapter.as_str().unwrap_or("未命名章节").to_string();
            *chapter = json!({
                "id": format!("chapter-{}", index + 1),
                "title": title,
                "items": []
            });
            fixes.push(fix(
                "string_to_chapter",
                &path,
                "Converted chapter string to chapter object.",
            ));
            changed = true;
        }
        let Some(chapter_object) = chapter.as_object_mut() else {
            continue;
        };
        changed |= rename_field(
            chapter_object,
            "content_title",
            "contentTitle",
            &path,
            fixes,
        );
        if !chapter_object.contains_key("id") {
            chapter_object.insert("id".to_string(), json!(format!("chapter-{}", index + 1)));
            fixes.push(fix(
                "default_chapter_id",
                &format!("{path}.id"),
                "Filled chapter id.",
            ));
            changed = true;
        }
        if !chapter_object
            .get("title")
            .and_then(Value::as_str)
            .is_some_and(|title| !title.trim().is_empty())
        {
            let title = chapter_object
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or("未命名章节")
                .to_string();
            chapter_object.insert("title".to_string(), json!(title));
            fixes.push(fix(
                "default_chapter_title",
                &format!("{path}.title"),
                "Filled chapter title.",
            ));
            changed = true;
        }
        if !chapter_object
            .get("contentTitle")
            .and_then(Value::as_str)
            .is_some_and(|title| !title.trim().is_empty())
        {
            let title = chapter_object
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or("未命名章节")
                .to_string();
            chapter_object.insert("contentTitle".to_string(), json!(title));
            fixes.push(fix(
                "default_chapter_content_title",
                &format!("{path}.contentTitle"),
                "Filled chapter contentTitle from title.",
            ));
            changed = true;
        }
        changed |= normalize_chapter_items(chapter_object, &path, fixes);
    }

    changed
}

fn normalize_chapter_items(
    chapter: &mut Map<String, Value>,
    chapter_path: &str,
    fixes: &mut Vec<ContentNormalizeFix>,
) -> bool {
    let mut changed = false;
    let items = chapter
        .entry("items")
        .or_insert_with(|| Value::Array(Vec::new()));
    if items.is_string() {
        let body = items.as_str().unwrap_or_default().to_string();
        *items = json!([{ "heading": "要点", "body": body }]);
        fixes.push(fix(
            "string_to_items",
            &format!("{chapter_path}.items"),
            "Converted items string to a single item.",
        ));
        changed = true;
    }
    let Some(items) = items.as_array_mut() else {
        return changed;
    };
    for (index, item) in items.iter_mut().enumerate() {
        let path = format!("{chapter_path}.items[{index}]");
        if item.is_string() {
            let body = item.as_str().unwrap_or_default().to_string();
            *item = json!({
                "heading": format!("要点 {}", index + 1),
                "body": body
            });
            fixes.push(fix(
                "string_to_item",
                &path,
                "Converted item string to heading/body object.",
            ));
            changed = true;
        }
    }
    changed
}

fn normalize_evaluations(value: &mut Value, fixes: &mut Vec<ContentNormalizeFix>) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let Some(evaluations) = object.get_mut("evaluations") else {
        return false;
    };
    let mut changed = false;
    if evaluations.is_string() {
        let title = evaluations.as_str().unwrap_or("评价").to_string();
        *evaluations = json!([{ "title": title, "items": [] }]);
        fixes.push(fix(
            "string_to_evaluations",
            "$.evaluations",
            "Converted evaluations string to array.",
        ));
        changed = true;
    }
    let Some(items) = evaluations.as_array_mut() else {
        return changed;
    };
    for (index, item) in items.iter_mut().enumerate() {
        if item.is_string() {
            let title = item.as_str().unwrap_or("评价").to_string();
            *item = json!({ "title": title, "items": [] });
            fixes.push(fix(
                "string_to_evaluation",
                &format!("$.evaluations[{index}]"),
                "Converted evaluation string to object.",
            ));
            changed = true;
        } else if let Some(object) = item.as_object_mut() {
            if object.get("items").is_some_and(Value::is_string) {
                let raw = object
                    .get("items")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                object.insert("items".to_string(), json!(split_inline_list(raw)));
                fixes.push(fix(
                    "string_to_list",
                    &format!("$.evaluations[{index}].items"),
                    "Converted evaluation items string to array.",
                ));
                changed = true;
            }
        }
    }
    changed
}

fn string_to_array(
    value: &mut Value,
    key: &str,
    path: &str,
    fixes: &mut Vec<ContentNormalizeFix>,
) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let Some(raw) = object.get(key).and_then(Value::as_str).map(str::to_string) else {
        return false;
    };
    object.insert(key.to_string(), json!(split_inline_list(&raw)));
    fixes.push(fix(
        "string_to_list",
        path,
        &format!("Converted {key} string to array."),
    ));
    true
}

fn ensure_array_field(
    value: &mut Value,
    key: &str,
    path: &str,
    fixes: &mut Vec<ContentNormalizeFix>,
) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    if object.get(key).is_some() {
        return false;
    }
    object.insert(key.to_string(), Value::Array(Vec::new()));
    fixes.push(fix(
        "default_array",
        path,
        &format!("Filled missing {key} with an empty array."),
    ));
    true
}

fn rename_field(
    object: &mut Map<String, Value>,
    from: &str,
    to: &str,
    base_path: &str,
    fixes: &mut Vec<ContentNormalizeFix>,
) -> bool {
    if object.contains_key(to) {
        return false;
    }
    let Some(value) = object.remove(from) else {
        return false;
    };
    object.insert(to.to_string(), value);
    fixes.push(fix(
        "rename_field",
        &format!("{base_path}.{to}"),
        &format!("Renamed {from} to {to}."),
    ));
    true
}

fn split_inline_list(value: &str) -> Vec<String> {
    value
        .split(['\n', ';', '；', ',', '，'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn fix(code: &str, path: &str, message: &str) -> ContentNormalizeFix {
    ContentNormalizeFix {
        code: code.to_string(),
        path: path.to_string(),
        message: message.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_common_teaching_deck_shapes() {
        let mut value = json!({
            "schema_version": "1.0",
            "name": "护理培训",
            "goals": "目标一；目标二",
            "chapters": ["概述", { "label": "操作", "items": ["洗手", "核对"] }],
            "summary": "回顾；答疑"
        });
        let mut fixes = Vec::new();

        assert!(normalize_value(
            &mut value,
            Some("teaching_deck_v1"),
            &mut fixes
        ));
        assert_eq!(value["schemaVersion"], "1.0");
        assert_eq!(value["documentType"], "teaching_deck");
        assert_eq!(value["title"], "护理培训");
        assert_eq!(value["goals"].as_array().unwrap().len(), 2);
        assert_eq!(value["evaluations"].as_array().unwrap().len(), 0);
        assert_eq!(value["chapters"][0]["id"], "chapter-1");
        assert_eq!(value["chapters"][1]["items"][0]["heading"], "要点 1");
        assert!(fixes.iter().any(|fix| fix.code == "string_to_list"));
    }
}
