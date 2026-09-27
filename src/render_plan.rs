use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{fs, path::Path};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedPage {
    pub page_index: usize,
    pub page_template: String,
    pub source_slide: u32,
    pub data_path: String,
    pub repeat_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data_value: Option<Value>,
}

pub fn build_plan(
    manifest: &crate::template_manifest::TemplateManifest,
    content_file: &Path,
    recipe_id: &str,
) -> Result<Vec<PlannedPage>, String> {
    let raw = fs::read_to_string(content_file)
        .map_err(|err| format!("failed to read content for render plan: {err}"))?;
    let content: Value =
        serde_json::from_str(&raw).map_err(|err| format!("invalid content JSON: {err}"))?;
    build_plan_from_value(manifest, &content, recipe_id)
}

pub fn build_plan_from_value(
    manifest: &crate::template_manifest::TemplateManifest,
    content: &Value,
    recipe_id: &str,
) -> Result<Vec<PlannedPage>, String> {
    let steps = manifest
        .deck_recipes
        .get(recipe_id)
        .ok_or_else(|| format!("deck recipe not found: {recipe_id}"))?;

    let mut pages = Vec::new();
    expand_steps(manifest, content, "$", steps, &mut pages)?;

    Ok(pages)
}

pub fn has_explicit_pages(content: &Value) -> bool {
    content.get("pages").and_then(Value::as_array).is_some()
}

pub fn uses_declarative_explicit_pages(
    manifest: &crate::template_manifest::TemplateManifest,
    content: &Value,
) -> bool {
    manifest.renderer.is_none()
        && !manifest.page_templates.is_empty()
        && has_explicit_pages(content)
}

pub fn build_plan_from_explicit_pages(
    manifest: &crate::template_manifest::TemplateManifest,
    content: &Value,
) -> Result<Vec<PlannedPage>, String> {
    let pages = content
        .get("pages")
        .and_then(Value::as_array)
        .ok_or_else(|| "explicit pages content must contain pages[]".to_string())?;
    let mut planned_pages = Vec::new();

    for (index, page) in pages.iter().enumerate() {
        let page_object = page
            .as_object()
            .ok_or_else(|| format!("pages[{index}] must be an object"))?;
        let page_template = page_object
            .get("use")
            .or_else(|| page_object.get("pageTemplate"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| format!("pages[{index}] must declare use or pageTemplate"))?;
        let template = manifest.page_templates.get(page_template).ok_or_else(|| {
            format!("pages[{index}] references missing pageTemplate '{page_template}'")
        })?;
        let data_value = page_object
            .get("data")
            .cloned()
            .unwrap_or_else(|| page.clone());

        planned_pages.push(PlannedPage {
            page_index: planned_pages.len() + 1,
            page_template: page_template.to_string(),
            source_slide: template.source_slide,
            data_path: format!("$.pages[{index}].data"),
            repeat_index: None,
            data_value: Some(data_value),
        });
    }

    Ok(planned_pages)
}

fn expand_steps(
    manifest: &crate::template_manifest::TemplateManifest,
    scope_data: &Value,
    scope_path: &str,
    steps: &[crate::template_manifest::DeckStep],
    pages: &mut Vec<PlannedPage>,
) -> Result<(), String> {
    for step in steps {
        if !step.steps.is_empty() {
            if !step.use_template.trim().is_empty() {
                return Err(format!(
                    "group step '{}' declares both use and steps",
                    step.use_template
                ));
            }
            expand_group_step(manifest, scope_data, scope_path, step, pages)?;
            continue;
        }
        expand_page_step(manifest, scope_data, scope_path, step, pages)?;
    }

    Ok(())
}

fn expand_group_step(
    manifest: &crate::template_manifest::TemplateManifest,
    scope_data: &Value,
    scope_path: &str,
    step: &crate::template_manifest::DeckStep,
    pages: &mut Vec<PlannedPage>,
) -> Result<(), String> {
    if let Some(repeat_path) = &step.repeat {
        let items = select_path(scope_data, repeat_path)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("repeat path must resolve to an array: {repeat_path}"))?;
        let full_repeat_path = join_selector_path(scope_path, repeat_path);
        for (index, item) in items.iter().enumerate() {
            if !step_condition_matches(&step.when, item)? {
                continue;
            }
            expand_steps(
                manifest,
                item,
                &format!("{full_repeat_path}[{index}]"),
                &step.steps,
                pages,
            )?;
        }
        return Ok(());
    }

    let data_path = step.data.as_deref().unwrap_or("$");
    let group_data = select_path(scope_data, data_path)
        .ok_or_else(|| format!("data path does not exist: {data_path}"))?;
    if !step_condition_matches(&step.when, group_data)? {
        return Ok(());
    }
    let full_data_path = join_selector_path(scope_path, data_path);
    expand_steps(manifest, group_data, &full_data_path, &step.steps, pages)
}

fn expand_page_step(
    manifest: &crate::template_manifest::TemplateManifest,
    scope_data: &Value,
    scope_path: &str,
    step: &crate::template_manifest::DeckStep,
    pages: &mut Vec<PlannedPage>,
) -> Result<(), String> {
    if step.use_template.trim().is_empty() {
        return Err("deck step has no page template or child steps".to_string());
    }

    if let Some(repeat_path) = &step.repeat {
        let items = select_path(scope_data, repeat_path)
            .and_then(Value::as_array)
            .ok_or_else(|| format!("repeat path must resolve to an array: {repeat_path}"))?;
        let full_repeat_path = join_selector_path(scope_path, repeat_path);
        if let Some(chunk) = &step.chunk {
            if chunk.size == 0 {
                return Err(format!(
                    "chunk.size must be greater than zero for page template '{}'",
                    step.use_template
                ));
            }
            let page_count = items.len().div_ceil(chunk.size);
            for (chunk_index, start_index) in (0..items.len()).step_by(chunk.size).enumerate() {
                let end_exclusive = (start_index + chunk.size).min(items.len());
                let data_value = build_chunk_data_value(
                    scope_data,
                    repeat_path,
                    items[start_index..end_exclusive].to_vec(),
                    chunk.items_as.as_deref(),
                    chunk_index,
                    page_count,
                    start_index,
                    end_exclusive,
                );
                if !step_condition_matches(&step.when, &data_value)? {
                    continue;
                }
                push_planned_pages_for_data(
                    manifest,
                    step,
                    &format!("{full_repeat_path}[{start_index}..{end_exclusive}]"),
                    Some(chunk_index),
                    &data_value,
                    Some(data_value.clone()),
                    pages,
                )?;
            }
            return Ok(());
        }
        for (index, item) in items.iter().enumerate() {
            if !step_condition_matches(&step.when, item)? {
                continue;
            }
            push_planned_pages_for_data(
                manifest,
                step,
                &format!("{full_repeat_path}[{index}]"),
                Some(index),
                item,
                None,
                pages,
            )?;
        }
    } else {
        let data_path = step.data.as_deref().unwrap_or("$");
        let page_data = select_path(scope_data, data_path)
            .ok_or_else(|| format!("data path does not exist: {data_path}"))?;
        if !step_condition_matches(&step.when, page_data)? {
            return Ok(());
        }
        let full_data_path = join_selector_path(scope_path, data_path);
        push_planned_pages_for_data(
            manifest,
            step,
            &full_data_path,
            None,
            page_data,
            None,
            pages,
        )?;
    }

    Ok(())
}

fn join_selector_path(scope_path: &str, selector: &str) -> String {
    let selector = selector.trim();
    if selector == "$" {
        return scope_path.to_string();
    }
    if scope_path == "$" {
        return selector.to_string();
    }
    if let Some(rest) = selector.strip_prefix("$.") {
        return format!("{scope_path}.{rest}");
    }
    if let Some(rest) = selector.strip_prefix("$[") {
        return format!("{scope_path}[{rest}");
    }
    selector.to_string()
}

fn push_planned_pages_for_data(
    manifest: &crate::template_manifest::TemplateManifest,
    step: &crate::template_manifest::DeckStep,
    data_path: &str,
    repeat_index: Option<usize>,
    page_data: &Value,
    data_value: Option<Value>,
    pages: &mut Vec<PlannedPage>,
) -> Result<(), String> {
    if let Some(overflow) = &step.overflow {
        if let Some(overflow_values) = build_overflow_data_values(page_data, overflow)? {
            for (overflow_index, overflow_value) in overflow_values.into_iter().enumerate() {
                push_single_planned_page(
                    manifest,
                    step,
                    &format!("{data_path}#overflow[{overflow_index}]"),
                    repeat_index,
                    &overflow_value,
                    Some(overflow_value.clone()),
                    pages,
                )?;
            }
            return Ok(());
        }
    }

    push_single_planned_page(
        manifest,
        step,
        data_path,
        repeat_index,
        page_data,
        data_value,
        pages,
    )
}

fn push_single_planned_page(
    manifest: &crate::template_manifest::TemplateManifest,
    step: &crate::template_manifest::DeckStep,
    data_path: &str,
    repeat_index: Option<usize>,
    page_data: &Value,
    data_value: Option<Value>,
    pages: &mut Vec<PlannedPage>,
) -> Result<(), String> {
    let (page_template, template) = {
        let variant_data = data_value.as_ref().unwrap_or(page_data);
        resolve_step_template(manifest, step, variant_data)?
    };
    pages.push(PlannedPage {
        page_index: pages.len() + 1,
        page_template,
        source_slide: template.source_slide,
        data_path: data_path.to_string(),
        repeat_index,
        data_value,
    });
    Ok(())
}

fn resolve_step_template<'a>(
    manifest: &'a crate::template_manifest::TemplateManifest,
    step: &'a crate::template_manifest::DeckStep,
    page_data: &Value,
) -> Result<(String, &'a crate::template_manifest::PageTemplate), String> {
    let template_id = select_variant_template(step, page_data)?.unwrap_or(&step.use_template);
    let template = manifest
        .page_templates
        .get(template_id)
        .ok_or_else(|| format!("missing page template: {template_id}"))?;
    Ok((template_id.to_string(), template))
}

fn select_variant_template<'a>(
    step: &'a crate::template_manifest::DeckStep,
    page_data: &Value,
) -> Result<Option<&'a str>, String> {
    for variant in &step.variants {
        if variant_matches(variant, page_data)? {
            return Ok(Some(&variant.use_template));
        }
    }
    Ok(None)
}

fn variant_matches(
    variant: &crate::template_manifest::DeckStepVariant,
    page_data: &Value,
) -> Result<bool, String> {
    let Some(condition) = &variant.when else {
        return Ok(true);
    };
    let condition_path = condition.path.as_deref().unwrap_or("$");
    let value = select_path(page_data, condition_path).ok_or_else(|| {
        format!(
            "variant '{}' condition path does not exist: {condition_path}",
            variant.use_template
        )
    })?;
    let has_count_rule = condition.count_min.is_some()
        || condition.count_max.is_some()
        || condition.count_eq.is_some();
    if has_count_rule {
        let count = value_count(value).ok_or_else(|| {
            format!(
                "variant '{}' condition path '{condition_path}' does not resolve to countable data",
                variant.use_template
            )
        })?;
        if condition.count_eq.is_some_and(|expected| count != expected) {
            return Ok(false);
        }
        if condition.count_min.is_some_and(|min| count < min) {
            return Ok(false);
        }
        if condition.count_max.is_some_and(|max| count > max) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn value_count(value: &Value) -> Option<usize> {
    match value {
        Value::Array(items) => Some(items.len()),
        Value::Object(object) => Some(object.len()),
        Value::String(text) => Some(text.chars().count()),
        _ => None,
    }
}

fn step_condition_matches(
    condition: &Option<crate::template_manifest::StepCondition>,
    data: &Value,
) -> Result<bool, String> {
    let Some(condition) = condition else {
        return Ok(true);
    };
    match condition {
        crate::template_manifest::StepCondition::Path(path) => {
            let Some(value) = select_path(data, path) else {
                return Ok(false);
            };
            Ok(value_is_truthy(value))
        }
        crate::template_manifest::StepCondition::Rule(rule) => {
            let condition_path = rule.path.as_deref().unwrap_or("$");
            let value = select_path(data, condition_path);
            if let Some(expected_exists) = rule.exists {
                if value.is_some() != expected_exists {
                    return Ok(false);
                }
                if !expected_exists {
                    return Ok(true);
                }
            }
            let Some(value) = value else {
                return Ok(false);
            };
            if rule
                .truthy
                .is_some_and(|expected| value_is_truthy(value) != expected)
            {
                return Ok(false);
            }
            let has_count_rule =
                rule.count_min.is_some() || rule.count_max.is_some() || rule.count_eq.is_some();
            if has_count_rule {
                let count = value_count(value).ok_or_else(|| {
                    format!(
                        "when condition path '{condition_path}' does not resolve to countable data"
                    )
                })?;
                if rule.count_eq.is_some_and(|expected| count != expected) {
                    return Ok(false);
                }
                if rule.count_min.is_some_and(|min| count < min) {
                    return Ok(false);
                }
                if rule.count_max.is_some_and(|max| count > max) {
                    return Ok(false);
                }
                return Ok(true);
            }
            if rule.exists.is_some() || rule.truthy.is_some() {
                return Ok(true);
            }
            Ok(value_is_truthy(value))
        }
    }
}

fn value_is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(object) => !object.is_empty(),
    }
}

fn build_overflow_data_values(
    page_data: &Value,
    overflow: &crate::template_manifest::DeckStepOverflow,
) -> Result<Option<Vec<Value>>, String> {
    let items = select_path(page_data, &overflow.path)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("overflow.path must resolve to an array: {}", overflow.path))?;
    if items.len() <= overflow.max_items {
        return Ok(None);
    }

    let overflow_count = items.len().div_ceil(overflow.max_items);
    let mut values = Vec::new();
    for (overflow_index, start_index) in (0..items.len()).step_by(overflow.max_items).enumerate() {
        let end_exclusive = (start_index + overflow.max_items).min(items.len());
        let chunk_value = Value::Array(items[start_index..end_exclusive].to_vec());
        let mut value = replace_path_value(page_data, &overflow.path, chunk_value.clone())?;
        if let Value::Object(object) = &mut value {
            if let Some(alias) = overflow
                .items_as
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                object.insert(alias.to_string(), chunk_value);
            }
            object.insert("overflowIndex".to_string(), json!(overflow_index + 1));
            object.insert("overflowCount".to_string(), json!(overflow_count));
            object.insert("overflowStartIndex".to_string(), json!(start_index));
            object.insert(
                "overflowEndIndex".to_string(),
                json!(end_exclusive.saturating_sub(1)),
            );
            object.insert("overflowStartNumber".to_string(), json!(start_index + 1));
            object.insert("overflowEndNumber".to_string(), json!(end_exclusive));
        }
        values.push(value);
    }
    Ok(Some(values))
}

fn replace_path_value(root: &Value, path: &str, replacement: Value) -> Result<Value, String> {
    if path == "$" {
        return Ok(replacement);
    }
    let Some(trimmed) = path.strip_prefix("$.") else {
        return Err(format!("unsupported overflow.path '{path}'"));
    };
    let segments = trimmed.split('.').collect::<Vec<_>>();
    if segments.is_empty() {
        return Err(format!("unsupported overflow.path '{path}'"));
    }
    let mut value = root.clone();
    replace_path_segments(&mut value, &segments, replacement, path)?;
    Ok(value)
}

fn replace_path_segments(
    current: &mut Value,
    segments: &[&str],
    replacement: Value,
    original_path: &str,
) -> Result<(), String> {
    let segment = segments
        .first()
        .ok_or_else(|| format!("unsupported overflow.path '{original_path}'"))?;
    let (name, index) = parse_segment(segment)
        .ok_or_else(|| format!("unsupported overflow.path '{original_path}'"))?;
    let object = current
        .as_object_mut()
        .ok_or_else(|| format!("overflow.path parent is not an object: {original_path}"))?;
    if segments.len() == 1 {
        if index.is_some() {
            return Err(format!(
                "overflow.path cannot replace an array item directly: {original_path}"
            ));
        }
        object.insert(name.to_string(), replacement);
        return Ok(());
    }
    let next = object
        .get_mut(name)
        .ok_or_else(|| format!("overflow.path does not exist: {original_path}"))?;
    let next = if let Some(index) = index {
        next.as_array_mut()
            .and_then(|items| items.get_mut(index))
            .ok_or_else(|| format!("overflow.path index does not exist: {original_path}"))?
    } else {
        next
    };
    replace_path_segments(next, &segments[1..], replacement, original_path)
}

fn build_chunk_data_value(
    root: &Value,
    repeat_path: &str,
    items: Vec<Value>,
    items_as: Option<&str>,
    chunk_index: usize,
    page_count: usize,
    start_index: usize,
    end_exclusive: usize,
) -> Value {
    let alias = items_as
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("items");
    let items_value = Value::Array(items);
    let mut object = Map::new();
    object.insert("items".to_string(), items_value.clone());
    if alias != "items" {
        object.insert(alias.to_string(), items_value);
    }
    object.insert("root".to_string(), root.clone());
    object.insert("sourcePath".to_string(), json!(repeat_path));
    object.insert("pageIndex".to_string(), json!(chunk_index + 1));
    object.insert("pageCount".to_string(), json!(page_count));
    object.insert("startIndex".to_string(), json!(start_index));
    object.insert(
        "endIndex".to_string(),
        json!(end_exclusive.saturating_sub(1)),
    );
    object.insert("startNumber".to_string(), json!(start_index + 1));
    object.insert("endNumber".to_string(), json!(end_exclusive));
    Value::Object(object)
}

fn select_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    if path == "$" {
        return Some(root);
    }
    let mut current = root;
    let trimmed = path.strip_prefix("$.")?;
    for segment in trimmed.split('.') {
        let (name, index) = parse_segment(segment)?;
        current = current.get(name)?;
        if let Some(index) = index {
            current = current.as_array()?.get(index)?;
        }
    }
    Some(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn script_manifest() -> crate::template_manifest::TemplateManifest {
        crate::template_manifest::TemplateManifest {
            schema_version: "1.0".to_string(),
            template_id: "script-pages-test".to_string(),
            name: "Script Pages Test".to_string(),
            family_id: None,
            role: None,
            format: "pptx".to_string(),
            template_type: Some("script".to_string()),
            entry: None,
            content_schema: None,
            input: None,
            preview: None,
            render_mode: None,
            renderer: Some(crate::template_manifest::TemplateRendererSpec {
                renderer_type: Some("script.python".to_string()),
                runtime: Some("python3".to_string()),
                entry: Some("renderer/build.py".to_string()),
                input: None,
                output: None,
                args: Vec::new(),
            }),
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

    #[test]
    fn script_template_owns_its_pages_array() {
        let manifest = script_manifest();
        let content = json!({
            "pages": [{ "type": "cover", "title": "护理教学" }]
        });

        assert!(has_explicit_pages(&content));
        assert!(!uses_declarative_explicit_pages(&manifest, &content));
    }

    #[test]
    fn repeat_chunk_expands_collection_into_paged_data_values() -> Result<(), String> {
        let manifest = crate::template_manifest::TemplateManifest {
            schema_version: "1.0".to_string(),
            template_id: "chunk-test".to_string(),
            name: "Chunk Test".to_string(),
            family_id: None,
            role: None,
            format: "pptx".to_string(),
            template_type: Some("declarative".to_string()),
            entry: Some("template.pptx".to_string()),
            content_schema: None,
            input: None,
            preview: None,
            render_mode: None,
            renderer: None,
            pipeline: Vec::new(),
            dependencies: crate::template_manifest::TemplateDependencySpec::default(),
            test_cases: Vec::new(),
            page_templates: BTreeMap::from([(
                "toc".to_string(),
                crate::template_manifest::PageTemplate {
                    source_slide: 2,
                    description: None,
                    bindings: BTreeMap::new(),
                    constraints: BTreeMap::new(),
                },
            )]),
            deck_recipes: BTreeMap::from([(
                "deck".to_string(),
                vec![crate::template_manifest::DeckStep {
                    use_template: "toc".to_string(),
                    data: None,
                    repeat: Some("$.chapters".to_string()),
                    steps: Vec::new(),
                    chunk: Some(crate::template_manifest::RepeatChunk {
                        size: 4,
                        items_as: Some("chapters".to_string()),
                    }),
                    variants: Vec::new(),
                    overflow: None,
                    when: None,
                }],
            )]),
            block_templates: BTreeMap::new(),
            document_recipes: BTreeMap::new(),
            sheet_templates: BTreeMap::new(),
            workbook_recipes: BTreeMap::new(),
            layouts: BTreeMap::new(),
        };
        let content = json!({
            "title": "护理教学",
            "chapters": [
                { "title": "一" },
                { "title": "二" },
                { "title": "三" },
                { "title": "四" },
                { "title": "五" }
            ]
        });

        let pages = build_plan_from_value(&manifest, &content, "deck")?;

        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].data_path, "$.chapters[0..4]");
        assert_eq!(pages[1].data_path, "$.chapters[4..5]");
        let first_page = pages[0].data_value.as_ref().expect("first data value");
        assert_eq!(first_page["chapters"].as_array().unwrap().len(), 4);
        assert_eq!(first_page["pageIndex"], json!(1));
        assert_eq!(first_page["pageCount"], json!(2));
        assert_eq!(first_page["root"]["title"], json!("护理教学"));
        let second_page = pages[1].data_value.as_ref().expect("second data value");
        assert_eq!(second_page["chapters"].as_array().unwrap().len(), 1);
        assert_eq!(second_page["startNumber"], json!(5));
        Ok(())
    }

    #[test]
    fn variants_select_page_template_from_chunk_count() -> Result<(), String> {
        let manifest = crate::template_manifest::TemplateManifest {
            schema_version: "1.0".to_string(),
            template_id: "variant-test".to_string(),
            name: "Variant Test".to_string(),
            family_id: None,
            role: None,
            format: "pptx".to_string(),
            template_type: Some("declarative".to_string()),
            entry: Some("template.pptx".to_string()),
            content_schema: None,
            input: None,
            preview: None,
            render_mode: None,
            renderer: None,
            pipeline: Vec::new(),
            dependencies: crate::template_manifest::TemplateDependencySpec::default(),
            test_cases: Vec::new(),
            page_templates: BTreeMap::from([
                (
                    "toc".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 2,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
                (
                    "toc_compact".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 3,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
            ]),
            deck_recipes: BTreeMap::from([(
                "deck".to_string(),
                vec![crate::template_manifest::DeckStep {
                    use_template: "toc".to_string(),
                    data: None,
                    repeat: Some("$.chapters".to_string()),
                    steps: Vec::new(),
                    chunk: Some(crate::template_manifest::RepeatChunk {
                        size: 4,
                        items_as: Some("chapters".to_string()),
                    }),
                    variants: vec![crate::template_manifest::DeckStepVariant {
                        use_template: "toc_compact".to_string(),
                        when: Some(crate::template_manifest::VariantCondition {
                            path: Some("$.chapters".to_string()),
                            count_min: None,
                            count_max: Some(2),
                            count_eq: None,
                        }),
                    }],
                    overflow: None,
                    when: None,
                }],
            )]),
            block_templates: BTreeMap::new(),
            document_recipes: BTreeMap::new(),
            sheet_templates: BTreeMap::new(),
            workbook_recipes: BTreeMap::new(),
            layouts: BTreeMap::new(),
        };
        let content = json!({
            "chapters": [
                { "title": "一" },
                { "title": "二" },
                { "title": "三" },
                { "title": "四" },
                { "title": "五" },
                { "title": "六" }
            ]
        });

        let pages = build_plan_from_value(&manifest, &content, "deck")?;

        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].page_template, "toc");
        assert_eq!(pages[0].source_slide, 2);
        assert_eq!(pages[1].page_template, "toc_compact");
        assert_eq!(pages[1].source_slide, 3);
        Ok(())
    }

    #[test]
    fn overflow_split_expands_long_nested_list_into_multiple_pages() -> Result<(), String> {
        let manifest = crate::template_manifest::TemplateManifest {
            schema_version: "1.0".to_string(),
            template_id: "overflow-test".to_string(),
            name: "Overflow Test".to_string(),
            family_id: None,
            role: None,
            format: "pptx".to_string(),
            template_type: Some("declarative".to_string()),
            entry: Some("template.pptx".to_string()),
            content_schema: None,
            input: None,
            preview: None,
            render_mode: None,
            renderer: None,
            pipeline: Vec::new(),
            dependencies: crate::template_manifest::TemplateDependencySpec::default(),
            test_cases: Vec::new(),
            page_templates: BTreeMap::from([(
                "chapter_content".to_string(),
                crate::template_manifest::PageTemplate {
                    source_slide: 4,
                    description: None,
                    bindings: BTreeMap::new(),
                    constraints: BTreeMap::new(),
                },
            )]),
            deck_recipes: BTreeMap::from([(
                "deck".to_string(),
                vec![crate::template_manifest::DeckStep {
                    use_template: "chapter_content".to_string(),
                    data: None,
                    repeat: Some("$.chapters".to_string()),
                    steps: Vec::new(),
                    chunk: None,
                    variants: Vec::new(),
                    overflow: Some(crate::template_manifest::DeckStepOverflow {
                        strategy: Some("split".to_string()),
                        path: "$.items".to_string(),
                        max_items: 4,
                        items_as: None,
                    }),
                    when: None,
                }],
            )]),
            block_templates: BTreeMap::new(),
            document_recipes: BTreeMap::new(),
            sheet_templates: BTreeMap::new(),
            workbook_recipes: BTreeMap::new(),
            layouts: BTreeMap::new(),
        };
        let content = json!({
            "chapters": [{
                "title": "护理执行",
                "items": (1..=9)
                    .map(|index| json!({ "heading": format!("要点 {index}"), "body": "正文" }))
                    .collect::<Vec<_>>()
            }]
        });

        let pages = build_plan_from_value(&manifest, &content, "deck")?;

        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].data_path, "$.chapters[0]#overflow[0]");
        assert_eq!(pages[1].data_path, "$.chapters[0]#overflow[1]");
        assert_eq!(pages[2].data_path, "$.chapters[0]#overflow[2]");
        assert_eq!(
            pages[0].data_value.as_ref().unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            pages[2].data_value.as_ref().unwrap()["items"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            pages[2].data_value.as_ref().unwrap()["overflowIndex"],
            json!(3)
        );
        assert_eq!(
            pages[2].data_value.as_ref().unwrap()["overflowCount"],
            json!(3)
        );
        Ok(())
    }

    #[test]
    fn variants_select_template_after_overflow_split() -> Result<(), String> {
        let manifest = crate::template_manifest::TemplateManifest {
            schema_version: "1.0".to_string(),
            template_id: "overflow-variant-test".to_string(),
            name: "Overflow Variant Test".to_string(),
            family_id: None,
            role: None,
            format: "pptx".to_string(),
            template_type: Some("declarative".to_string()),
            entry: Some("template.pptx".to_string()),
            content_schema: None,
            input: None,
            preview: None,
            render_mode: None,
            renderer: None,
            pipeline: Vec::new(),
            dependencies: crate::template_manifest::TemplateDependencySpec::default(),
            test_cases: Vec::new(),
            page_templates: BTreeMap::from([
                (
                    "chapter_content".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 4,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
                (
                    "chapter_content_compact".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 9,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
            ]),
            deck_recipes: BTreeMap::from([(
                "deck".to_string(),
                vec![crate::template_manifest::DeckStep {
                    use_template: "chapter_content".to_string(),
                    data: None,
                    repeat: Some("$.chapters".to_string()),
                    steps: Vec::new(),
                    chunk: None,
                    variants: vec![crate::template_manifest::DeckStepVariant {
                        use_template: "chapter_content_compact".to_string(),
                        when: Some(crate::template_manifest::VariantCondition {
                            path: Some("$.items".to_string()),
                            count_min: None,
                            count_max: Some(2),
                            count_eq: None,
                        }),
                    }],
                    overflow: Some(crate::template_manifest::DeckStepOverflow {
                        strategy: Some("split".to_string()),
                        path: "$.items".to_string(),
                        max_items: 4,
                        items_as: None,
                    }),
                    when: None,
                }],
            )]),
            block_templates: BTreeMap::new(),
            document_recipes: BTreeMap::new(),
            sheet_templates: BTreeMap::new(),
            workbook_recipes: BTreeMap::new(),
            layouts: BTreeMap::new(),
        };
        let content = json!({
            "chapters": [{
                "title": "护理执行",
                "items": (1..=9)
                    .map(|index| json!({ "heading": format!("要点 {index}"), "body": "正文" }))
                    .collect::<Vec<_>>()
            }]
        });

        let pages = build_plan_from_value(&manifest, &content, "deck")?;

        assert_eq!(pages.len(), 3);
        assert_eq!(pages[0].page_template, "chapter_content");
        assert_eq!(pages[1].page_template, "chapter_content");
        assert_eq!(pages[2].page_template, "chapter_content_compact");
        assert_eq!(pages[2].source_slide, 9);
        Ok(())
    }

    #[test]
    fn grouped_repeat_keeps_section_pages_with_chapter_content() -> Result<(), String> {
        let manifest = crate::template_manifest::TemplateManifest {
            schema_version: "1.0".to_string(),
            template_id: "group-test".to_string(),
            name: "Group Test".to_string(),
            family_id: None,
            role: None,
            format: "pptx".to_string(),
            template_type: Some("declarative".to_string()),
            entry: Some("template.pptx".to_string()),
            content_schema: None,
            input: None,
            preview: None,
            render_mode: None,
            renderer: None,
            pipeline: Vec::new(),
            dependencies: crate::template_manifest::TemplateDependencySpec::default(),
            test_cases: Vec::new(),
            page_templates: BTreeMap::from([
                (
                    "cover".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 1,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
                (
                    "section_divider".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 3,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
                (
                    "chapter_content".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 4,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
                (
                    "chapter_content_compact".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 9,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
                (
                    "closing".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 10,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
            ]),
            deck_recipes: BTreeMap::from([(
                "deck".to_string(),
                vec![
                    crate::template_manifest::DeckStep {
                        use_template: "cover".to_string(),
                        data: Some("$".to_string()),
                        repeat: None,
                        steps: Vec::new(),
                        chunk: None,
                        variants: Vec::new(),
                        overflow: None,
                        when: None,
                    },
                    crate::template_manifest::DeckStep {
                        use_template: String::new(),
                        data: None,
                        repeat: Some("$.chapters".to_string()),
                        steps: vec![
                            crate::template_manifest::DeckStep {
                                use_template: "section_divider".to_string(),
                                data: Some("$".to_string()),
                                repeat: None,
                                steps: Vec::new(),
                                chunk: None,
                                variants: Vec::new(),
                                overflow: None,
                                when: None,
                            },
                            crate::template_manifest::DeckStep {
                                use_template: "chapter_content".to_string(),
                                data: Some("$".to_string()),
                                repeat: None,
                                steps: Vec::new(),
                                chunk: None,
                                variants: vec![crate::template_manifest::DeckStepVariant {
                                    use_template: "chapter_content_compact".to_string(),
                                    when: Some(crate::template_manifest::VariantCondition {
                                        path: Some("$.items".to_string()),
                                        count_min: None,
                                        count_max: Some(2),
                                        count_eq: None,
                                    }),
                                }],
                                overflow: Some(crate::template_manifest::DeckStepOverflow {
                                    strategy: Some("split".to_string()),
                                    path: "$.items".to_string(),
                                    max_items: 4,
                                    items_as: None,
                                }),
                                when: None,
                            },
                        ],
                        chunk: None,
                        variants: Vec::new(),
                        overflow: None,
                        when: None,
                    },
                    crate::template_manifest::DeckStep {
                        use_template: "closing".to_string(),
                        data: Some("$".to_string()),
                        repeat: None,
                        steps: Vec::new(),
                        chunk: None,
                        variants: Vec::new(),
                        overflow: None,
                        when: None,
                    },
                ],
            )]),
            block_templates: BTreeMap::new(),
            document_recipes: BTreeMap::new(),
            sheet_templates: BTreeMap::new(),
            workbook_recipes: BTreeMap::new(),
            layouts: BTreeMap::new(),
        };
        let content = json!({
            "title": "护理教学",
            "chapters": [
                {
                    "title": "第一章",
                    "items": (1..=5)
                        .map(|index| json!({ "heading": format!("要点 {index}"), "body": "正文" }))
                        .collect::<Vec<_>>()
                },
                {
                    "title": "第二章",
                    "items": [{ "heading": "单点", "body": "正文" }]
                }
            ]
        });

        let pages = build_plan_from_value(&manifest, &content, "deck")?;

        assert_eq!(pages.len(), 7);
        assert_eq!(pages[0].page_template, "cover");
        assert_eq!(pages[1].page_template, "section_divider");
        assert_eq!(pages[1].data_path, "$.chapters[0]");
        assert_eq!(pages[2].page_template, "chapter_content");
        assert_eq!(pages[2].data_path, "$.chapters[0]#overflow[0]");
        assert_eq!(pages[3].page_template, "chapter_content_compact");
        assert_eq!(pages[3].data_path, "$.chapters[0]#overflow[1]");
        assert_eq!(pages[4].page_template, "section_divider");
        assert_eq!(pages[4].data_path, "$.chapters[1]");
        assert_eq!(pages[5].page_template, "chapter_content_compact");
        assert_eq!(pages[5].data_path, "$.chapters[1]");
        assert_eq!(pages[6].page_template, "closing");
        Ok(())
    }

    #[test]
    fn step_when_filters_pages_and_repeated_items() -> Result<(), String> {
        let manifest = crate::template_manifest::TemplateManifest {
            schema_version: "1.0".to_string(),
            template_id: "when-test".to_string(),
            name: "When Test".to_string(),
            family_id: None,
            role: None,
            format: "pptx".to_string(),
            template_type: Some("declarative".to_string()),
            entry: Some("template.pptx".to_string()),
            content_schema: None,
            input: None,
            preview: None,
            render_mode: None,
            renderer: None,
            pipeline: Vec::new(),
            dependencies: crate::template_manifest::TemplateDependencySpec::default(),
            test_cases: Vec::new(),
            page_templates: BTreeMap::from([
                (
                    "cover".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 1,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
                (
                    "assessment_table".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 5,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
                (
                    "no_assessment".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 6,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
                (
                    "summary".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 8,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
                (
                    "evaluation".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 9,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
                (
                    "closing".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 10,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
            ]),
            deck_recipes: BTreeMap::from([(
                "deck".to_string(),
                vec![
                    crate::template_manifest::DeckStep {
                        use_template: "cover".to_string(),
                        data: Some("$".to_string()),
                        repeat: None,
                        steps: Vec::new(),
                        chunk: None,
                        variants: Vec::new(),
                        overflow: None,
                        when: None,
                    },
                    crate::template_manifest::DeckStep {
                        use_template: "assessment_table".to_string(),
                        data: Some("$".to_string()),
                        repeat: None,
                        steps: Vec::new(),
                        chunk: None,
                        variants: Vec::new(),
                        overflow: None,
                        when: Some(crate::template_manifest::StepCondition::Path(
                            "$.assessmentTable".to_string(),
                        )),
                    },
                    crate::template_manifest::DeckStep {
                        use_template: "no_assessment".to_string(),
                        data: Some("$".to_string()),
                        repeat: None,
                        steps: Vec::new(),
                        chunk: None,
                        variants: Vec::new(),
                        overflow: None,
                        when: Some(crate::template_manifest::StepCondition::Rule(
                            crate::template_manifest::StepConditionRule {
                                path: Some("$.assessmentTable".to_string()),
                                exists: Some(false),
                                truthy: None,
                                count_min: None,
                                count_max: None,
                                count_eq: None,
                            },
                        )),
                    },
                    crate::template_manifest::DeckStep {
                        use_template: "summary".to_string(),
                        data: Some("$".to_string()),
                        repeat: None,
                        steps: Vec::new(),
                        chunk: None,
                        variants: Vec::new(),
                        overflow: None,
                        when: Some(crate::template_manifest::StepCondition::Rule(
                            crate::template_manifest::StepConditionRule {
                                path: Some("$.summary".to_string()),
                                exists: None,
                                truthy: None,
                                count_min: Some(1),
                                count_max: None,
                                count_eq: None,
                            },
                        )),
                    },
                    crate::template_manifest::DeckStep {
                        use_template: "evaluation".to_string(),
                        data: None,
                        repeat: Some("$.evaluations".to_string()),
                        steps: Vec::new(),
                        chunk: None,
                        variants: Vec::new(),
                        overflow: None,
                        when: Some(crate::template_manifest::StepCondition::Rule(
                            crate::template_manifest::StepConditionRule {
                                path: Some("$.items".to_string()),
                                exists: None,
                                truthy: None,
                                count_min: Some(1),
                                count_max: None,
                                count_eq: None,
                            },
                        )),
                    },
                    crate::template_manifest::DeckStep {
                        use_template: "closing".to_string(),
                        data: Some("$".to_string()),
                        repeat: None,
                        steps: Vec::new(),
                        chunk: None,
                        variants: Vec::new(),
                        overflow: None,
                        when: None,
                    },
                ],
            )]),
            block_templates: BTreeMap::new(),
            document_recipes: BTreeMap::new(),
            sheet_templates: BTreeMap::new(),
            workbook_recipes: BTreeMap::new(),
            layouts: BTreeMap::new(),
        };
        let content = json!({
            "title": "护理教学",
            "summary": ["复盘重点"],
            "evaluations": [
                { "title": "空问题", "items": [] },
                { "title": "课堂提问", "items": ["何时呼叫护士？"] }
            ]
        });

        let pages = build_plan_from_value(&manifest, &content, "deck")?;

        assert_eq!(pages.len(), 5);
        assert_eq!(pages[0].page_template, "cover");
        assert_eq!(pages[1].page_template, "no_assessment");
        assert_eq!(pages[2].page_template, "summary");
        assert_eq!(pages[3].page_template, "evaluation");
        assert_eq!(pages[3].data_path, "$.evaluations[1]");
        assert_eq!(pages[4].page_template, "closing");
        Ok(())
    }

    #[test]
    fn explicit_pages_build_direct_planned_pages() -> Result<(), String> {
        let manifest = crate::template_manifest::TemplateManifest {
            schema_version: "1.0".to_string(),
            template_id: "explicit-pages-test".to_string(),
            name: "Explicit Pages Test".to_string(),
            family_id: None,
            role: None,
            format: "pptx".to_string(),
            template_type: Some("declarative".to_string()),
            entry: Some("template.pptx".to_string()),
            content_schema: None,
            input: None,
            preview: None,
            render_mode: None,
            renderer: None,
            pipeline: Vec::new(),
            dependencies: crate::template_manifest::TemplateDependencySpec::default(),
            test_cases: Vec::new(),
            page_templates: BTreeMap::from([
                (
                    "cover".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 1,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
                (
                    "chapter_content".to_string(),
                    crate::template_manifest::PageTemplate {
                        source_slide: 4,
                        description: None,
                        bindings: BTreeMap::new(),
                        constraints: BTreeMap::new(),
                    },
                ),
            ]),
            deck_recipes: BTreeMap::new(),
            block_templates: BTreeMap::new(),
            document_recipes: BTreeMap::new(),
            sheet_templates: BTreeMap::new(),
            workbook_recipes: BTreeMap::new(),
            layouts: BTreeMap::new(),
        };
        let content = json!({
            "pages": [
                {
                    "use": "cover",
                    "data": { "title": "护理教学" }
                },
                {
                    "pageTemplate": "chapter_content",
                    "data": {
                        "contentTitle": "护理执行",
                        "items": [{ "heading": "主动巡视", "body": "正文" }]
                    }
                }
            ]
        });

        let pages = build_plan_from_explicit_pages(&manifest, &content)?;

        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].page_template, "cover");
        assert_eq!(pages[0].source_slide, 1);
        assert_eq!(pages[0].data_path, "$.pages[0].data");
        assert_eq!(
            pages[0].data_value.as_ref().unwrap()["title"],
            json!("护理教学")
        );
        assert_eq!(pages[1].page_template, "chapter_content");
        assert_eq!(pages[1].source_slide, 4);
        assert_eq!(
            pages[1].data_value.as_ref().unwrap()["contentTitle"],
            json!("护理执行")
        );
        Ok(())
    }
}

fn parse_segment(segment: &str) -> Option<(&str, Option<usize>)> {
    if let Some(left) = segment.find('[') {
        let right = segment.strip_suffix(']')?;
        let name = &segment[..left];
        let raw_index = &right[left + 1..];
        let index = raw_index.parse().ok()?;
        Some((name, Some(index)))
    } else {
        Some((segment, None))
    }
}
