//! The text of a staging model, in the shape of dbt's style guide: a
//! `source` CTE that selects the relation, a `renamed` CTE that renames
//! and casts, and `select * from renamed`; and the properties entry
//! that goes with it. Pure.
//!
//! Renaming, deterministic and conservative:
//! 1. snake_case: an upper-case letter after a lower-case letter or a
//!    digit starts a new word, every run of anything but letters and
//!    digits is one `_`, all lower case, no `_` at either end, and a
//!    leading digit gets a `col_` prefix;
//! 2. a column called `id` becomes `<object>_id`, the object being the
//!    table's (or model's) name without a `raw_` prefix, made singular
//!    by `ies` → `y`, dropping the `es` of `ches`, `shes`, `sses`, `xes`,
//!    or one trailing `s` (not after `s` or `u`); irregular plurals
//!    (`loaves`) come out regular (`loave`), which a reader renames;
//! 3. a name another column already took keeps the column's own name,
//!    and where that is taken too, `_2`, `_3`, … after it.
//!
//! A column is cast only where its type is known. Tests: the key alone
//! (`<object>_id`) gets `unique` and `not_null`; nothing else gets a
//! test, as a test is a claim the scaffold cannot back.

use super::yaml_text::plain_name;

/// A column the relation has, with what is known of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InputColumn {
    pub name: String,
    pub data_type: Option<String>,
    pub description: String,
}

/// A column of the staging model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Staged {
    /// The column as read from the relation, quoted where it must be.
    pub expr: String,
    pub name: String,
    pub data_type: Option<String>,
    pub description: String,
    pub key: bool,
}

pub(crate) fn snake_case(name: &str) -> String {
    let mut out = String::new();
    let mut prev: Option<char> = None;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            let boundary = c.is_ascii_uppercase()
                && prev.is_some_and(|p| p.is_ascii_lowercase() || p.is_ascii_digit());
            if boundary && !out.ends_with('_') {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
        prev = Some(c);
    }
    let out = out.trim_end_matches('_').to_string();
    if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("col_{out}")
    } else {
        out
    }
}

/// The object a table or model holds one row of: `raw_orders` → `order`.
pub(crate) fn object_of(relation: &str) -> String {
    let base = snake_case(relation);
    let base = base.strip_prefix("raw_").unwrap_or(&base).to_string();
    if let Some(stem) = base.strip_suffix("ies") {
        return format!("{stem}y");
    }
    for ending in ["ches", "shes", "sses", "xes"] {
        if base.ends_with(ending) {
            return base[..base.len() - 2].to_string();
        }
    }
    match base.strip_suffix('s') {
        Some(stem) if !stem.ends_with('s') && !stem.ends_with('u') && !stem.is_empty() => {
            stem.to_string()
        }
        _ => base,
    }
}

/// The staging columns of `columns`, renamed and typed.
pub(crate) fn staged(columns: &[InputColumn], relation: &str, dialect: &str) -> Vec<Staged> {
    let key = format!("{}_id", object_of(relation));
    let mut taken: Vec<String> = Vec::new();
    let mut out = Vec::new();
    for column in columns {
        let snake = snake_case(&column.name);
        let wanted = if snake == "id" { key.clone() } else { snake };
        let mut name = if wanted.is_empty() || taken.contains(&wanted) {
            column.name.clone()
        } else {
            wanted
        };
        let base = name.clone();
        let mut n = 2;
        while taken.contains(&name) {
            name = format!("{base}_{n}");
            n += 1;
        }
        taken.push(name.clone());
        out.push(Staged {
            expr: quoted(&column.name, dialect),
            key: name == key,
            name,
            data_type: column.data_type.clone(),
            description: column.description.clone(),
        });
    }
    out
}

/// A name as SQL reads it: bare where it is a plain lower-case word.
fn quoted(name: &str, dialect: &str) -> String {
    let plain = plain_name(name).is_some_and(|n| n == n.to_ascii_lowercase());
    if plain {
        return name.to_string();
    }
    if dialect == "bigquery" {
        format!("`{}`", name.replace('`', "\\`"))
    } else {
        format!("\"{}\"", name.replace('"', "\"\""))
    }
}

/// The model's SQL over `relation` (a `{{ source(..) }}` or
/// `{{ ref(..) }}` call). No columns: `select *`, said in a comment.
pub(crate) fn sql(relation: &str, columns: &[Staged], dialect: &str) -> String {
    let mut out = String::new();
    if columns.is_empty() {
        out.push_str(
            "-- columns unknown (no YAML columns, catalog or inferred types): select *\n\n",
        );
    }
    out.push_str("with\n\nsource as (\n\n");
    out.push_str(&format!("    select * from {relation}\n\n),\n\n"));
    out.push_str("renamed as (\n\n    select\n");
    if columns.is_empty() {
        out.push_str("        *\n");
    }
    for (i, column) in columns.iter().enumerate() {
        let read = match &column.data_type {
            Some(ty) => format!("cast({} as {ty})", column.expr),
            None => column.expr.clone(),
        };
        let item = if read == column.name {
            read
        } else {
            format!("{read} as {}", quoted(&column.name, dialect))
        };
        let comma = if i + 1 < columns.len() { "," } else { "" };
        out.push_str(&format!("        {item}{comma}\n"));
    }
    out.push_str("\n    from source\n\n)\n\nselect * from renamed\n");
    out
}

/// The properties file of the model, its entry and its columns.
pub(crate) fn yaml(model: &str, description: &str, columns: &[Staged]) -> String {
    let q = |s: &str| serde_json::to_string(s).expect("a string serializes");
    let mut out = format!(
        "version: 2\n\nmodels:\n  - name: {model}\n    description: {}\n",
        q(description)
    );
    if !columns.is_empty() {
        out.push_str("    columns:\n");
    }
    for column in columns {
        let name = if plain_name(&column.name).is_some() {
            column.name.clone()
        } else {
            q(&column.name)
        };
        out.push_str(&format!(
            "      - name: {name}\n        description: {}\n",
            q(&column.description)
        ));
        if column.key {
            out.push_str("        data_tests:\n          - unique\n          - not_null\n");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn names_become_snake_case() {
        let cases = [
            ("orderId", "order_id"),
            ("Order Date", "order_date"),
            ("HTTPCode", "httpcode"),
            ("amount_USD", "amount_usd"),
            ("2nd_line", "col_2nd_line"),
            ("__x__", "x"),
            ("already_snake", "already_snake"),
        ];
        for (input, want) in cases {
            assert_eq!(snake_case(input), want, "{input}");
        }
    }

    #[test]
    fn the_object_is_singular_and_without_raw() {
        let cases = [
            ("orders", "order"),
            ("raw_customers", "customer"),
            ("categories", "category"),
            ("address", "address"),
            ("status", "status"),
            ("payment", "payment"),
            ("batches", "batch"),
            ("boxes", "box"),
        ];
        for (input, want) in cases {
            assert_eq!(object_of(input), want, "{input}");
        }
    }

    fn col(name: &str, ty: Option<&str>) -> InputColumn {
        InputColumn {
            name: name.into(),
            data_type: ty.map(str::to_string),
            description: String::new(),
        }
    }

    #[test]
    fn id_becomes_the_key_and_a_clash_keeps_its_name() {
        let cols = [col("id", Some("integer")), col("order_id", None)];
        let staged = staged(&cols, "orders", "duckdb");
        assert_eq!(staged[0].name, "order_id");
        assert!(staged[0].key);
        // the second one clashes, and so does its own name
        assert_eq!(staged[1].name, "order_id_2");
        assert_eq!(staged[1].expr, "order_id");
        assert!(!staged[1].key);
    }

    #[test]
    fn upper_case_names_are_quoted() {
        assert_eq!(quoted("Amount", "duckdb"), "\"Amount\"");
        assert_eq!(quoted("Amount", "bigquery"), "`Amount`");
        assert_eq!(quoted("amount", "duckdb"), "amount");
    }
}
