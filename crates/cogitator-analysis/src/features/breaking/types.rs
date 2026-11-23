//! Comparing a current column type with a built one: case, spacing and
//! the common spellings of one type are not changes.

/// The canonical spelling of a type, for comparison only.
pub(super) fn normalize(data_type: &str) -> String {
    let lowered = data_type
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();
    let (head, params) = match lowered.find('(') {
        Some(at) => (lowered[..at].trim_end(), &lowered[at..]),
        None => (lowered.as_str(), ""),
    };
    let head = match head {
        "int" | "int4" | "integer" | "signed" => "integer",
        "int8" | "bigint" | "long" => "bigint",
        "int2" | "smallint" | "short" => "smallint",
        "float8" | "double" | "double precision" | "float64" => "double",
        "float4" | "real" | "float32" => "real",
        "bool" | "boolean" | "logical" => "boolean",
        "string" | "text" | "varchar" | "character varying" | "char varying" => "varchar",
        "numeric" | "decimal" => "decimal",
        "timestamp without time zone" | "datetime" => "timestamp",
        "timestamp with time zone" | "timestamptz" => "timestamptz",
        other => other,
    };
    format!("{head}{}", params.replace(' ', ""))
}

/// Both known and different after normalization.
pub(super) fn changed(current: Option<&str>, built: Option<&str>) -> bool {
    match (current, built) {
        (Some(current), Some(built)) => normalize(current) != normalize(built),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_changes() {
        let cases = [
            (Some("INTEGER"), Some("int"), false),
            (Some("BIGINT"), Some("integer"), true),
            (Some("varchar"), Some("TEXT"), false),
            (Some("decimal(10, 2)"), Some("NUMERIC(10,2)"), false),
            (Some("decimal(10,2)"), Some("decimal(12,2)"), true),
            (Some("double precision"), Some("DOUBLE"), false),
            (None, Some("int"), false),
            (Some("int"), None, false),
            (None, None, false),
            (Some("DATE"), Some("timestamp"), true),
        ];
        for (current, built, expected) in cases {
            assert_eq!(
                changed(current, built),
                expected,
                "{current:?} vs {built:?}"
            );
        }
    }
}
