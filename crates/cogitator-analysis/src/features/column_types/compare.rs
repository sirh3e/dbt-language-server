//! Comparing and spelling types on the `SqlType` lattice.
//!
//! Normalisation itself is the front end's: `parse_sql_type` already reads
//! `int`/`integer`/`int4` as a 32-bit integer, `bigint`/`int8` as 64-bit,
//! `varchar`/`text`/`string` as text and `numeric(p,s)` as a decimal, per
//! dialect. What is decided here is what two such types *prove* about each
//! other, and how the dialect spells one back.
//!
//! The comparison is deliberately coarse: only a different *family* of
//! types (a number against text, a date against a timestamp) is a
//! disagreement. Widths, precisions and unstated time zones stay silent,
//! because inference rarely proves them and warehouses widen freely.

use cogitator_dialects::Dialect;
use cogitator_ir::types::{SqlType, Tz};
use serde::Serialize;

/// What a proven type says about a declared one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Verdict {
    /// Same family, nothing contradicts.
    Agree,
    /// Different families: no value of one is a value of the other.
    Disagree,
    /// One side is not modelled well enough to say.
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Family {
    Bool,
    Number,
    Text,
    Binary,
    Date,
    Time,
    Timestamp,
    Interval,
    Json,
}

fn family(ty: &SqlType) -> Option<Family> {
    Some(match ty {
        SqlType::Bool => Family::Bool,
        SqlType::Int { .. } | SqlType::Decimal { .. } | SqlType::Float { .. } => Family::Number,
        SqlType::Text => Family::Text,
        SqlType::Binary => Family::Binary,
        SqlType::Date => Family::Date,
        SqlType::Time => Family::Time,
        SqlType::Timestamp { .. } => Family::Timestamp,
        SqlType::Interval => Family::Interval,
        SqlType::Json => Family::Json,
        _ => return None,
    })
}

/// Compares an inferred type with a declared `data_type`.
pub fn compare(proven: &SqlType, declared: &SqlType) -> Verdict {
    let (Some(a), Some(b)) = (family(proven), family(declared)) else {
        return Verdict::Unknown;
    };
    if a != b {
        // JSON is stored as text by several warehouses: not a contradiction.
        return match (a, b) {
            (Family::Json, Family::Text) | (Family::Text, Family::Json) => Verdict::Unknown,
            _ => Verdict::Disagree,
        };
    }
    match (proven, declared) {
        (SqlType::Timestamp { tz: x }, SqlType::Timestamp { tz: y })
            if *x != Tz::Unknown && *y != Tz::Unknown && x != y =>
        {
            Verdict::Disagree
        }
        // a number of another kind or width: widening is the warehouse's
        (SqlType::Int { .. }, SqlType::Int { .. })
        | (SqlType::Decimal { .. }, SqlType::Decimal { .. })
        | (SqlType::Float { .. }, SqlType::Float { .. }) => Verdict::Agree,
        (SqlType::Int { .. } | SqlType::Decimal { .. } | SqlType::Float { .. }, _) => {
            Verdict::Unknown
        }
        _ => Verdict::Agree,
    }
}

fn is(dialect: &Dialect, names: &[&str]) -> bool {
    names.contains(&dialect.as_str())
}

/// The dialect's spelling of `ty`, only when `ty` pins one type exactly:
/// an integer of unproven width or a timestamp of unknown zone has none.
pub fn render(ty: &SqlType, dialect: &Dialect) -> Option<String> {
    let bq = is(dialect, &["bigquery"]);
    let sf = is(dialect, &["snowflake"]);
    let pg = is(dialect, &["postgres", "redshift"]);
    let s = |v: &str| Some(v.to_string());
    match ty {
        SqlType::Bool if bq => s("bool"),
        SqlType::Bool => s("boolean"),
        SqlType::Int { bits: Some(b) } => match (*b, bq, sf) {
            (64, true, _) => s("int64"),
            (_, true, _) | (_, _, true) => None,
            (8, ..) if !pg => s("tinyint"),
            (16, ..) => s("smallint"),
            (32, ..) => s("integer"),
            (64, ..) => s("bigint"),
            (128, ..) if !pg => s("hugeint"),
            _ => None,
        },
        SqlType::Decimal {
            p: Some(p),
            s: Some(sc),
        } => Some(if sf {
            format!("number({p},{sc})")
        } else {
            format!("numeric({p},{sc})")
        }),
        SqlType::Float { bits: Some(64) } => Some(
            match (bq, sf, pg) {
                (true, ..) => "float64",
                (_, true, _) => "float",
                (.., true) => "double precision",
                _ => "double",
            }
            .to_string(),
        ),
        SqlType::Float { bits: Some(32) } if !bq && !sf => s("real"),
        SqlType::Text if bq => s("string"),
        SqlType::Text if pg && !is(dialect, &["redshift"]) => s("text"),
        SqlType::Text => s("varchar"),
        SqlType::Binary => s(if bq {
            "bytes"
        } else if sf {
            "binary"
        } else if pg {
            "bytea"
        } else {
            "blob"
        }),
        SqlType::Date => s("date"),
        SqlType::Time => s("time"),
        SqlType::Timestamp { tz: Tz::With } => s(if bq {
            "timestamp"
        } else if sf {
            "timestamp_tz"
        } else {
            "timestamptz"
        }),
        SqlType::Timestamp { tz: Tz::Without } => s(if bq {
            "datetime"
        } else if sf {
            "timestamp_ntz"
        } else {
            "timestamp"
        }),
        SqlType::Interval if !sf => s("interval"),
        SqlType::Json if !sf => s("json"),
        _ => None,
    }
}

/// A short description for hovers: the exact spelling when there is one,
/// otherwise the family with what is not proven.
pub fn describe(ty: &SqlType, dialect: &Dialect) -> String {
    if let Some(exact) = render(ty, dialect) {
        return exact;
    }
    match ty {
        SqlType::Unknown => "unknown".into(),
        SqlType::Null => "null (no type)".into(),
        SqlType::Int { .. } => "integer (width not proven)".into(),
        SqlType::Decimal { .. } => "decimal (precision not proven)".into(),
        SqlType::Float { .. } => "floating point (width not proven)".into(),
        SqlType::Timestamp { .. } => "timestamp (time zone not proven)".into(),
        SqlType::Array(inner) => format!("array of {}", describe(inner, dialect)),
        SqlType::Struct(_) => "struct".into(),
        SqlType::Map(_) => "map".into(),
        SqlType::Other(raw) => raw.to_string(),
        other => format!("{other:?}").to_lowercase(),
    }
}
