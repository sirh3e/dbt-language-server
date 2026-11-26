//! The settings of the cost report and the column profiler
//! (`cost.query_history`, `warehouse.profile`, `warehouse.profile_sample_rows`):
//! file-only keys, one snapshot readers clone. A removed key reverts to
//! the default (these keys have no CLI counterpart).

use super::FileConfig;

pub const DEFAULT_PROFILE_SAMPLE_ROWS: u64 = 10_000;

#[derive(Debug, Clone, PartialEq)]
pub struct AnalyticsSettings {
    /// The query-history export, relative to the project root or absolute.
    pub query_history: Option<String>,
    /// dbt.profileColumn may query the warehouse.
    pub profile: bool,
    pub profile_sample_rows: u64,
}

impl Default for AnalyticsSettings {
    fn default() -> Self {
        AnalyticsSettings {
            query_history: None,
            profile: false,
            profile_sample_rows: DEFAULT_PROFILE_SAMPLE_ROWS,
        }
    }
}

impl FileConfig {
    pub fn analytics(&self) -> AnalyticsSettings {
        AnalyticsSettings {
            query_history: self
                .cost
                .query_history
                .clone()
                .filter(|p| !p.trim().is_empty()),
            profile: self.warehouse.profile.unwrap_or(false),
            profile_sample_rows: self
                .warehouse
                .profile_sample_rows
                .filter(|rows| *rows > 0)
                .unwrap_or(DEFAULT_PROFILE_SAMPLE_ROWS),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::parse;
    use super::*;
    use std::path::Path;

    #[test]
    fn defaults_and_file_values() {
        let empty = parse("", Path::new("c.yml")).unwrap();
        assert_eq!(empty.analytics(), AnalyticsSettings::default());
        let file = parse(
            "cost:\n  query_history: target/history.json\nwarehouse:\n  profile: true\n  profile_sample_rows: 500\n",
            Path::new("c.yml"),
        )
        .unwrap();
        let settings = file.analytics();
        assert_eq!(
            settings.query_history.as_deref(),
            Some("target/history.json")
        );
        assert!(settings.profile);
        assert_eq!(settings.profile_sample_rows, 500);
    }
}
