//! Case tables for the pure parts of the breaking-change preview.

use super::diff::similarity;
use super::downstream::{severity, Severity};
use super::*;
use crate::lineage::{ColumnLineage, ModelLineage};

fn now(cols: &[(&str, Option<&str>)]) -> Vec<CurrentColumn> {
    cols.iter()
        .map(|(n, t)| CurrentColumn {
            name: n.to_string(),
            data_type: t.map(str::to_string),
        })
        .collect()
}

fn built(cols: &[(&str, Option<&str>)]) -> Vec<BaselineColumn> {
    cols.iter()
        .map(|(n, t)| BaselineColumn {
            name: n.to_string(),
            data_type: t.map(str::to_string),
        })
        .collect()
}

#[test]
fn diff_cases() {
    // (current, built, removed (name, renamed_to), added, retyped)
    type Case<'a> = (
        &'a [(&'a str, Option<&'a str>)],
        &'a [(&'a str, Option<&'a str>)],
        &'a [(&'a str, Option<&'a str>)],
        &'a [&'a str],
        &'a [&'a str],
    );
    let cases: &[Case] = &[
        (&[("id", None)], &[("ID", None)], &[], &[], &[]),
        (
            &[("id", None)],
            &[("id", None), ("name", None)],
            &[("name", None)],
            &[],
            &[],
        ),
        (
            &[("id", None), ("extra", None)],
            &[("id", None)],
            &[],
            &["extra"],
            &[],
        ),
        (
            &[("customer_key", None)],
            &[("customer_id", None)],
            &[("customer_id", Some("customer_key"))],
            &["customer_key"],
            &[],
        ),
        (
            &[("given_name", None)],
            &[("first_name", None)],
            &[("first_name", Some("given_name"))],
            &["given_name"],
            &[],
        ),
        (
            &[("zzz", None)],
            &[("amount", None)],
            &[("amount", None)],
            &["zzz"],
            &[],
        ),
        (
            &[("id", Some("BIGINT"))],
            &[("id", Some("INTEGER"))],
            &[],
            &[],
            &["id"],
        ),
        (
            &[("id", Some("int"))],
            &[("id", Some("INTEGER"))],
            &[],
            &[],
            &[],
        ),
        // types known on one side only: names alone
        (&[("id", None)], &[("id", Some("INTEGER"))], &[], &[], &[]),
        // two removals, one candidate: the closer one gets it
        (
            &[("order_date_utc", None)],
            &[("order_date", None), ("order_id", None)],
            &[("order_date", Some("order_date_utc")), ("order_id", None)],
            &["order_date_utc"],
            &[],
        ),
    ];
    for (current, base, removed, added, retyped) in cases {
        let got = diff(&now(current), &built(base));
        let got_removed: Vec<(&str, Option<&str>)> = got
            .removed
            .iter()
            .map(|r| (r.name.as_str(), r.possibly_renamed_to.as_deref()))
            .collect();
        assert_eq!(&got_removed, removed, "{current:?} vs {base:?}");
        let got_added: Vec<&str> = got.added.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(&got_added, added, "{current:?} vs {base:?}");
        let got_retyped: Vec<&str> = got.retyped.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(&got_retyped, retyped, "{current:?} vs {base:?}");
    }
}

#[test]
fn similarity_cases() {
    let cases = [
        ("customer_id", "customer_key", true),
        ("first_name", "given_name", true),
        ("amount", "zzz", false),
        ("id", "xy", false),
        ("", "", false),
    ];
    for (a, b, similar) in cases {
        assert_eq!(similarity(a, b) >= 0.5, similar, "{a} ~ {b}");
    }
}

fn use_of(contracted: bool, tested: bool) -> DownstreamUse {
    DownstreamUse {
        model: "m".into(),
        column: "c".into(),
        tests: if tested {
            vec!["not_null".into()]
        } else {
            vec![]
        },
        contracted,
        public: false,
    }
}

#[test]
fn severity_cases() {
    use ChangeKind::{Removed, Retyped};
    let tested = vec!["unique".to_string()];
    let cases = [
        (Removed, vec![], vec![], false, Severity::Information),
        (Removed, vec![], vec![], true, Severity::Warning),
        (
            Removed,
            vec![],
            vec![use_of(false, false)],
            false,
            Severity::Warning,
        ),
        (
            Removed,
            vec![],
            vec![use_of(true, false)],
            false,
            Severity::Error,
        ),
        (
            Removed,
            vec![],
            vec![use_of(false, true)],
            false,
            Severity::Error,
        ),
        (Removed, tested.clone(), vec![], false, Severity::Error),
        (Retyped, tested.clone(), vec![], false, Severity::Warning),
        (
            Retyped,
            vec![],
            vec![use_of(true, false)],
            false,
            Severity::Warning,
        ),
        (Retyped, vec![], vec![], false, Severity::Information),
    ];
    for (change, tests, downstream, outside, expected) in cases {
        assert_eq!(
            severity(change, &tests, &downstream, outside),
            expected,
            "{change:?} tests={tests:?} downstream={downstream:?} outside={outside}"
        );
    }
}

#[test]
fn stars_hide_the_current_columns() {
    let lineage = |names: &[&str]| ModelLineage {
        columns: names
            .iter()
            .map(|n| ColumnLineage {
                name: n.to_string(),
                sources: vec![],
            })
            .collect(),
        origin: None,
    };
    assert_eq!(current_columns(&lineage(&["a", "*"]), &|_| None), None);
    let typed = current_columns(&lineage(&["a"]), &|c| Some(format!("t_{c}"))).unwrap();
    assert_eq!(
        typed[0].data_type.as_deref(),
        Some("t_a"),
        "the type hook is consulted"
    );
}

#[test]
fn surfaces_stay_silent_without_a_comparison() {
    for status in [Status::NoBaseline, Status::UnknownColumns, Status::Compared] {
        let report = BreakingReport::without("m", status, None);
        assert!(diagnostics(&report).is_empty(), "{status:?}");
        assert!(lens(&report, "file:///m.sql").is_none(), "{status:?}");
    }
}

#[test]
fn lens_counts_removed_and_retyped() {
    let mut report = BreakingReport::without("m", Status::Compared, None);
    report.diff = diff(
        &now(&[("id", Some("BIGINT"))]),
        &built(&[("id", Some("INT")), ("gone", None)]),
    );
    let lens = lens(&report, "file:///m.sql").expect("lens");
    assert_eq!(lens.command.title, "2 breaking changes");
    assert_eq!(lens.command.command, "dbt.breakingChanges");
}
