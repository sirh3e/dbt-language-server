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
