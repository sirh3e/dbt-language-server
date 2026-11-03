//! Sentinel masking for `{{ }}` expressions inside one SQL segment:
//! each becomes a COGITATOR_JINJA_<n> identifier sqlformat preserves,
//! restored in order afterwards — any miscount declines the format.

use super::FormatOptions;

pub fn format_masked(sql: &str, options: &FormatOptions) -> Option<String> {
    let (masked, expressions) = mask(sql)?;
    let formatted = sqlformat::format(
        &masked,
        &sqlformat::QueryParams::None,
        &sqlformat::FormatOptions {
            indent: if options.insert_spaces {
                sqlformat::Indent::Spaces(options.tab_size)
            } else {
                sqlformat::Indent::Tabs
            },
            uppercase: Some(false),
            lines_between_queries: 1,
            ignore_case_convert: None,
        },
    );
    restore(&formatted, &expressions)
}

fn mask(sql: &str) -> Option<(String, Vec<&str>)> {
    let mut masked = String::with_capacity(sql.len());
    let mut expressions = Vec::new();
    let mut rest = sql;
    while let Some(open) = rest.find("{{") {
        let close = rest[open..].find("}}")? + open + 2;
        masked.push_str(&rest[..open]);
        masked.push_str(&format!("COGITATOR_JINJA_{}", expressions.len()));
        expressions.push(&rest[open..close]);
        rest = &rest[close..];
    }
    masked.push_str(rest);
    Some((masked, expressions))
}

fn restore(formatted: &str, expressions: &[&str]) -> Option<String> {
    let mut out = formatted.to_string();
    // reverse order so _1 never matches inside _10's sentinel
    for (i, expression) in expressions.iter().enumerate().rev() {
        let sentinel = format!("COGITATOR_JINJA_{i}");
        if out.matches(&sentinel).count() != 1 {
            return None; // duplicated or swallowed: decline, never corrupt
        }
        out = out.replacen(&sentinel, expression, 1);
    }
    (!out.contains("COGITATOR_JINJA_")).then_some(out)
}
