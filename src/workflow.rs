use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRepairHint {
    pub code: String,
    pub target: String,
    pub severity: String,
    pub blocking: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub message: String,
    pub suggested_action: String,
}

pub fn repair_hints(
    validation: &crate::content_ir::ContentWorkspaceValidation,
) -> Vec<WorkflowRepairHint> {
    let mut hints = Vec::new();

    for message in &validation.content.schema_errors {
        hints.push(repair_hint(
            "content_schema_error",
            "content",
            "error",
            true,
            None,
            Some("content.schema".to_string()),
            message,
            "fix_content_schema",
        ));
    }

    for message in &validation.content.warnings {
        hints.push(repair_hint(
            "content_warning",
            "content",
            "warning",
            false,
            None,
            Some("content".to_string()),
            message,
            "review_content",
        ));
    }

    if let Some(template) = validation.template.as_ref() {
        for message in &template.warnings {
            hints.push(repair_hint(
                "template_warning",
                "template",
                "warning",
                false,
                None,
                Some("template.manifest".to_string()),
                message,
                "repair_template_pack",
            ));
        }
    }

    for message in &validation.binding_warnings {
        let severity = if binding_warning_blocks_render(message) {
            "error"
        } else {
            "warning"
        };
        let blocking = severity == "error";
        let path = binding_warning_path(message);
        hints.push(repair_hint(
            binding_warning_code(message),
            binding_warning_target(message),
            severity,
            blocking,
            path.clone(),
            path,
            message,
            binding_warning_action(message),
        ));
    }

    if !validation.acceptance_summary.can_render && hints.is_empty() {
        hints.push(repair_hint(
            "acceptance_failed",
            "content",
            "error",
            true,
            None,
            Some("acceptanceSummary".to_string()),
            &validation.acceptance_summary.message,
            "fix_content_or_template",
        ));
    }

    hints
}

fn repair_hint(
    code: &str,
    target: &str,
    severity: &str,
    blocking: bool,
    path: Option<String>,
    source_path: Option<String>,
    message: &str,
    suggested_action: &str,
) -> WorkflowRepairHint {
    WorkflowRepairHint {
        code: code.to_string(),
        target: target.to_string(),
        severity: severity.to_string(),
        blocking,
        path,
        source_path,
        message: message.to_string(),
        suggested_action: suggested_action.to_string(),
    }
}

fn binding_warning_blocks_render(message: &str) -> bool {
    [
        "image file not found",
        "required but resolved empty",
        "unsupported remote/data",
        "relative image path",
        "invalid dataPath",
        "expects image data",
        "expects chart data",
        "expects table data",
        "expects list data",
    ]
    .iter()
    .any(|needle| message.contains(needle))
}

fn binding_warning_target(message: &str) -> &'static str {
    if message.contains("image")
        || message.contains("asset")
        || message.contains("remote/data")
        || message.contains("relative image path")
    {
        "asset"
    } else {
        "binding"
    }
}

fn binding_warning_action(message: &str) -> &'static str {
    if message.contains("invalid dataPath") {
        "fix_template_binding_path"
    } else if message.contains("expects image data")
        || message.contains("expects chart data")
        || message.contains("expects table data")
        || message.contains("expects list data")
    {
        "fix_content_field_type"
    } else if message.contains("required") && message.contains("resolved empty") {
        "fill_required_content_field"
    } else if message.contains("image file not found")
        || message.contains("relative image path")
        || message.contains("unsupported remote/data")
    {
        "fix_asset_reference"
    } else {
        "review_binding"
    }
}

fn binding_warning_code(message: &str) -> &'static str {
    if message.contains("invalid dataPath") {
        "binding_invalid_data_path"
    } else if message.contains("expects image data") {
        "binding_expected_image"
    } else if message.contains("expects chart data") {
        "binding_expected_chart"
    } else if message.contains("expects table data") {
        "binding_expected_table"
    } else if message.contains("expects list data") {
        "binding_expected_list"
    } else if message.contains("required") && message.contains("resolved empty") {
        "binding_required_empty"
    } else if message.contains("image file not found") {
        "asset_file_not_found"
    } else if message.contains("relative image path") {
        "asset_relative_path"
    } else if message.contains("unsupported remote/data") {
        "asset_unsupported_remote"
    } else {
        "binding_warning"
    }
}

fn binding_warning_path(message: &str) -> Option<String> {
    first_quoted_segment(message).map(|binding| format!("binding:{binding}"))
}

fn first_quoted_segment(message: &str) -> Option<String> {
    message
        .split('\'')
        .nth(1)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binding_warning_helpers_produce_agent_readable_hints() {
        let message = "binding 'hero_image' image file not found: missing.png";
        assert!(binding_warning_blocks_render(message));
        assert_eq!(binding_warning_target(message), "asset");
        assert_eq!(binding_warning_action(message), "fix_asset_reference");
        assert_eq!(binding_warning_code(message), "asset_file_not_found");
        assert_eq!(
            binding_warning_path(message),
            Some("binding:hero_image".to_string())
        );
    }

    #[test]
    fn repair_hint_contract_keeps_stable_agent_fields() {
        let hint = repair_hint(
            "asset_file_not_found",
            "asset",
            "error",
            true,
            Some("binding:hero_image".to_string()),
            Some("binding:hero_image".to_string()),
            "page 1 binding 'hero_image' image file not found: missing.png",
            "fix_asset_reference",
        );
        assert_eq!(hint.code, "asset_file_not_found");
        assert!(hint.blocking);
        assert_eq!(hint.source_path.as_deref(), Some("binding:hero_image"));
    }
}
