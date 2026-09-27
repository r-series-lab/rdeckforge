use serde::Serialize;
use serde_json::Value;
use std::{fs, path::Path};

const MAX_SCHEMA_ERRORS: usize = 20;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputSchemaDocument {
    pub label: String,
    pub builtin: bool,
    pub schema: Value,
}

pub fn validate_schema_file(template_dir: &Path, schema: &str) -> Result<(), String> {
    let (_, schema_value) = load_schema_value(template_dir, schema)?;
    compile_schema(schema, &schema_value).map(|_| ())
}

pub fn validate_effective_input_schema(
    template_dir: &Path,
    input_spec: &crate::template_manifest::TemplateInputSpec,
) -> Result<bool, String> {
    let Some((schema_label, schema_value, builtin_schema)) =
        effective_schema_value(template_dir, input_spec)?
    else {
        return Ok(false);
    };
    compile_schema(&schema_label, &schema_value).map(|_| builtin_schema)
}

pub fn validate_input_schema(
    template_dir: &Path,
    input_spec: &crate::template_manifest::TemplateInputSpec,
    value: &Value,
) -> Result<Vec<String>, String> {
    let Some((schema_label, schema_value, _builtin_schema)) =
        effective_schema_value(template_dir, input_spec)?
    else {
        return Ok(Vec::new());
    };

    let validator = compile_schema(&schema_label, &schema_value)?;
    let mut errors = validator
        .iter_errors(value)
        .take(MAX_SCHEMA_ERRORS + 1)
        .map(format_schema_error)
        .collect::<Vec<_>>();

    if errors.len() > MAX_SCHEMA_ERRORS {
        let hidden = errors.len() - MAX_SCHEMA_ERRORS;
        errors.truncate(MAX_SCHEMA_ERRORS);
        errors.push(format!(
            "schema produced more errors; showing first {MAX_SCHEMA_ERRORS}, {hidden} hidden"
        ));
    }

    Ok(errors)
}

pub fn effective_input_schema_document(
    template_dir: &Path,
    input_spec: &crate::template_manifest::TemplateInputSpec,
) -> Result<Option<InputSchemaDocument>, String> {
    let Some((label, schema, builtin)) = effective_schema_value(template_dir, input_spec)? else {
        return Ok(None);
    };
    Ok(Some(InputSchemaDocument {
        label,
        builtin,
        schema,
    }))
}

pub fn enforce_input_schema(
    template_dir: &Path,
    input_spec: &crate::template_manifest::TemplateInputSpec,
    value: &Value,
) -> Result<(), String> {
    let errors = validate_input_schema(template_dir, input_spec, value)?;
    if errors.is_empty() {
        return Ok(());
    }

    let schema_label = input_spec
        .schema
        .as_deref()
        .or(input_spec.schema_id.as_deref())
        .unwrap_or("input schema");
    Err(format!(
        "content schema validation failed for {schema_label}: {}",
        errors.join("; ")
    ))
}

fn load_schema_value(
    template_dir: &Path,
    schema: &str,
) -> Result<(std::path::PathBuf, Value), String> {
    let schema_path = template_dir.join(schema);
    crate::validator::require_file(&schema_path, "input schema")?;
    let raw = fs::read_to_string(&schema_path).map_err(|err| {
        format!(
            "failed to read input schema '{}': {err}",
            schema_path.display()
        )
    })?;
    let value = serde_json::from_str::<Value>(&raw).map_err(|err| {
        format!(
            "invalid input schema JSON '{}': {err}",
            schema_path.display()
        )
    })?;
    Ok((schema_path, value))
}

fn effective_schema_value(
    template_dir: &Path,
    input_spec: &crate::template_manifest::TemplateInputSpec,
) -> Result<Option<(String, Value, bool)>, String> {
    if let Some(schema) = input_spec.schema.as_deref() {
        let (schema_path, schema_value) = load_schema_value(template_dir, schema)?;
        return Ok(Some((
            schema_path.display().to_string(),
            schema_value,
            false,
        )));
    }

    let Some(profile_id) = input_spec.profile.as_deref() else {
        return Ok(None);
    };
    let Some(schema_value) = crate::profiles::profile_schema_value(profile_id)? else {
        return Ok(None);
    };
    let schema_label = input_spec
        .schema_id
        .as_deref()
        .unwrap_or(profile_id)
        .to_string();
    Ok(Some((schema_label, schema_value, true)))
}

fn compile_schema(schema_label: &str, schema: &Value) -> Result<jsonschema::Validator, String> {
    jsonschema::validator_for(schema)
        .map_err(|err| format!("invalid input schema '{schema_label}': {err}"))
}

fn format_schema_error(error: jsonschema::ValidationError<'_>) -> String {
    let location = error.instance_path().to_string();
    let location = if location.is_empty() {
        "$".to_string()
    } else {
        format!("${location}")
    };
    format!("{location}: {error}")
}
