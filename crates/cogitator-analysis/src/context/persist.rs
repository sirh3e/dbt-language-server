//! Scan-cache persistence (03 §10): the FileCache snapshots that make
//! a cold start skip unchanged files entirely. Scope is the SCAN cache
//! only — derived caches stay memory-only by design. The shell owns
//! the file format and I/O; this module is pure data.

use std::path::PathBuf;
use std::time::SystemTime;

use cogitator_parser::jinja_scan::{RawDocsBlock, RawMacro};
use cogitator_schemas::properties::PropertiesFile;
use serde::{Deserialize, Serialize};

use super::exposures::CitizenFile;
use super::ContextBuilder;
use crate::graph::EdgeSeed;

type Entries<T> = Vec<(PathBuf, (SystemTime, u64), T)>;

/// Every scan cache, stamps included, as one serializable value.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct PersistedScan {
    pub props: Entries<PropertiesFile>,
    pub citizens: Entries<CitizenFile>,
    pub macros: Entries<Vec<RawMacro>>,
    pub docs: Entries<Vec<RawDocsBlock>>,
    pub edges: Entries<Vec<EdgeSeed>>,
    pub sql_tags: Entries<Vec<String>>,
}

impl ContextBuilder {
    /// Nothing scanned yet (fresh process): the only moment seeding
    /// makes sense.
    pub fn scan_is_cold(&self) -> bool {
        self.props.is_empty() && self.macros.is_empty() && self.edges.is_empty()
    }

    pub fn export_scan(&self) -> PersistedScan {
        PersistedScan {
            props: self.props.export(),
            citizens: self.citizens.export(),
            macros: self.macros.export(),
            docs: self.docs.export(),
            edges: self.edges.export(),
            sql_tags: self.sql_tags.export(),
        }
    }

    /// Entries with stale stamps self-heal through the ordinary
    /// (mtime, size) check on the next scan.
    pub fn seed_scan(&mut self, scan: PersistedScan) {
        self.props.seed(scan.props);
        self.citizens.seed(scan.citizens);
        self.macros.seed(scan.macros);
        self.docs.seed(scan.docs);
        self.edges.seed(scan.edges);
        self.sql_tags.seed(scan.sql_tags);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_seed_round_trips_through_json() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/jaffle_shop_duckdb");
        let mut builder = ContextBuilder::default();
        assert!(builder.scan_is_cold());
        let _ctx = builder.build(root.to_str().unwrap());
        assert!(!builder.scan_is_cold());

        let json = serde_json::to_string(&builder.export_scan()).unwrap();
        let back: PersistedScan = serde_json::from_str(&json).unwrap();
        assert!(!back.props.is_empty());
        assert!(!back.edges.is_empty());

        let mut seeded = ContextBuilder::default();
        seeded.seed_scan(back);
        assert!(!seeded.scan_is_cold());
        // the seeded builder serves the same context WITHOUT re-reading
        // unchanged files: every lookup is a stamp hit
        let ctx = seeded.build(root.to_str().unwrap());
        assert!(!ctx.model_detail_map.is_empty());
        let (hits, misses) = seeded.cache_stats();
        assert!(hits > 0, "seed produced no hits ({hits}/{misses})");
    }
}
