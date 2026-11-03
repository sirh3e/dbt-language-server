//! workspace/diagnostic wire types (01 §6.4): project-wide findings
//! with the global context-revision result-id scheme.

use serde::{Deserialize, Serialize};

use super::results::Diagnostic;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDiagnosticParams {
    #[serde(default)]
    pub previous_result_ids: Vec<PreviousResultId>,
}

#[derive(Debug, Deserialize)]
pub struct PreviousResultId {
    pub uri: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct WorkspaceDiagnosticReport {
    pub items: Vec<WorkspaceDocumentReport>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum WorkspaceDocumentReport {
    Full {
        uri: String,
        #[serde(rename = "resultId")]
        result_id: String,
        version: Option<i64>,
        items: Vec<Diagnostic>,
    },
    Unchanged {
        uri: String,
        #[serde(rename = "resultId")]
        result_id: String,
        version: Option<i64>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_items_tag_their_kind() {
        let report = WorkspaceDiagnosticReport {
            items: vec![
                WorkspaceDocumentReport::Full {
                    uri: "file:///a.sql".into(),
                    result_id: "3".into(),
                    version: None,
                    items: Vec::new(),
                },
                WorkspaceDocumentReport::Unchanged {
                    uri: "file:///b.sql".into(),
                    result_id: "3".into(),
                    version: None,
                },
            ],
        };
        let json = serde_json::to_value(&report).unwrap();
        assert_eq!(json["items"][0]["kind"], "full");
        assert_eq!(json["items"][1]["kind"], "unchanged");
        assert_eq!(json["items"][1]["resultId"], "3");

        let params: WorkspaceDiagnosticParams = serde_json::from_str(
            r#"{"previousResultIds": [{"uri": "file:///a.sql", "value": "2"}]}"#,
        )
        .unwrap();
        assert_eq!(params.previous_result_ids.len(), 1);
        let empty: WorkspaceDiagnosticParams = serde_json::from_str("{}").unwrap();
        assert!(empty.previous_result_ids.is_empty());
    }
}
