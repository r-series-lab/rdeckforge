use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentProfile {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub formats: Vec<&'static str>,
    pub schema_id: &'static str,
    pub md_profile: Option<&'static str>,
    pub render_targets: Vec<&'static str>,
    pub builtin_schema: bool,
}

pub fn list_profiles() -> Vec<ContentProfile> {
    vec![
        ContentProfile {
            id: "teaching_deck_v1",
            name: "Teaching Deck",
            description: "Teaching-oriented PPTX/DOCX content with title, goals, chapters, summary, assets, tables, and charts.",
            formats: vec!["json", "md"],
            schema_id: "teaching_deck_v1",
            md_profile: Some("teaching_outline_v1"),
            render_targets: vec!["pptx", "docx"],
            builtin_schema: true,
        },
        ContentProfile {
            id: "design_doc_v1",
            name: "Design Document",
            description: "Engineering or product design document content with markdown-derived heading, paragraph, list, quote, and code blocks.",
            formats: vec!["json", "md"],
            schema_id: "design_doc_v1",
            md_profile: Some("design_doc_v1"),
            render_targets: vec!["docx"],
            builtin_schema: true,
        },
        ContentProfile {
            id: "feature_assessment_v1",
            name: "Feature Assessment Workbook",
            description: "Feature effort assessment workbook content with detailed rows and summary notes.",
            formats: vec!["json"],
            schema_id: "feature_assessment_v1",
            md_profile: None,
            render_targets: vec!["xlsx"],
            builtin_schema: true,
        },
    ]
}

pub fn get_profile(id: &str) -> Option<ContentProfile> {
    let id = id.trim();
    list_profiles().into_iter().find(|profile| profile.id == id)
}

pub fn profile_ids() -> Vec<&'static str> {
    list_profiles()
        .into_iter()
        .map(|profile| profile.id)
        .collect()
}

pub fn input_spec_for_profile(
    profile_id: &str,
) -> Result<crate::template_manifest::TemplateInputSpec, String> {
    let profile = get_profile(profile_id).ok_or_else(|| {
        format!(
            "unsupported input profile '{profile_id}'; supported profiles: {}",
            profile_ids().join(", ")
        )
    })?;
    Ok(crate::template_manifest::TemplateInputSpec {
        profile: Some(profile.id.to_string()),
        formats: profile
            .formats
            .iter()
            .map(|value| (*value).to_string())
            .collect(),
        schema: None,
        schema_id: Some(profile.schema_id.to_string()),
        md_profile: profile.md_profile.map(str::to_string),
        authoring: None,
    })
}

pub fn apply_profile_defaults(input: &mut crate::template_manifest::TemplateInputSpec) {
    let Some(profile_id) = input.profile.as_deref() else {
        return;
    };
    let Some(profile) = get_profile(profile_id) else {
        return;
    };
    if input.formats.is_empty() {
        input.formats = profile
            .formats
            .iter()
            .map(|value| (*value).to_string())
            .collect();
    }
    if input.schema_id.is_none() {
        input.schema_id = Some(profile.schema_id.to_string());
    }
    if input.md_profile.is_none() {
        input.md_profile = profile.md_profile.map(str::to_string);
    }
}

pub fn profile_schema_value(profile_id: &str) -> Result<Option<Value>, String> {
    let Some(raw) = profile_schema_raw(profile_id) else {
        return Ok(None);
    };
    serde_json::from_str(raw)
        .map(Some)
        .map_err(|err| format!("invalid built-in profile schema '{profile_id}': {err}"))
}

fn profile_schema_raw(profile_id: &str) -> Option<&'static str> {
    match profile_id.trim() {
        "teaching_deck_v1" => Some(include_str!("../schemas/content.schema.json")),
        "design_doc_v1" => Some(include_str!("../schemas/design-doc.schema.json")),
        "feature_assessment_v1" => Some(include_str!("../schemas/feature-assessment.schema.json")),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_defaults_fill_input_spec() -> Result<(), String> {
        let spec = input_spec_for_profile("feature_assessment_v1")?;

        assert_eq!(spec.profile.as_deref(), Some("feature_assessment_v1"));
        assert_eq!(spec.formats, vec!["json"]);
        assert_eq!(spec.schema_id.as_deref(), Some("feature_assessment_v1"));
        assert!(spec.md_profile.is_none());
        assert!(profile_schema_value("feature_assessment_v1")?.is_some());
        Ok(())
    }
}
