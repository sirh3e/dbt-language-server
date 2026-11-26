//! The BI manifest (docs/column-reach.md): a tool-neutral JSON file a
//! team exports from (or writes for) its BI tool, naming each dashboard
//! and the dbt models — and, where known, the columns — it reads. It is
//! the end of the lineage for `dbt.impact`'s column reach. cogitator.yml
//! `bi.manifest` points at it; nothing is guessed when it is absent.
//!
//! ```json
//! {"dashboards": [
//!   {"name": "Revenue", "url": "https://bi.example/d/1",
//!    "reads": [{"model": "orders", "column": "amount"}, {"model": "customers"}]}
//! ]}
//! ```

use std::path::Path;

use cogitator_error::guidance::Guidance;
use serde::Deserialize;

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BiManifest {
    pub dashboards: Vec<Dashboard>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Dashboard {
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub reads: Vec<DashboardRead>,
}

/// One model a dashboard reads; `column: None` reads the model as a
/// whole (a node-level dependency).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashboardRead {
    pub model: String,
    #[serde(default)]
    pub column: Option<String>,
}

/// Parse the manifest's text (`path` names it in errors).
pub fn parse(text: &str, path: &Path) -> Result<BiManifest, Guidance> {
    serde_json::from_str(text).map_err(|err| {
        Guidance::new("the BI manifest does not parse.")
            .at(format!("{}:{}", path.display(), err.line()))
            .because(err.to_string())
            .fix(
                "write {\"dashboards\": [{\"name\": …, \"url\": …, \"reads\": [{\"model\": …, \
                 \"column\": …}]}]} (docs/column-reach.md), or drop bi.manifest from cogitator.yml.",
            )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_refuses() {
        let ok = parse(
            r#"{"dashboards":[{"name":"Rev","url":"u","reads":[{"model":"orders","column":"amount"},{"model":"customers"}]}]}"#,
            Path::new("bi.json"),
        )
        .unwrap();
        assert_eq!(ok.dashboards[0].reads[1].column, None);
        assert_eq!(ok.dashboards[0].reads[0].column.as_deref(), Some("amount"));
        for bad in [
            "",
            "{}",
            r#"{"dashboards":[{"url":"x"}]}"#,
            r#"{"dashboards":[],"extra":1}"#,
        ] {
            let err = parse(bad, Path::new("bi.json")).unwrap_err().to_message();
            assert!(err.contains("BI manifest"), "{bad}: {err}");
        }
    }
}
