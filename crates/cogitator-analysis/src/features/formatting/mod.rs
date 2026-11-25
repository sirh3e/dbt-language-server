//! textDocument/formatting (05 §9): sqlformat over Jinja-segmented
//! SQL. Full-line `{% %}`/`{# #}` statements pass through verbatim,
//! `{{ }}` expressions ride as sentinel identifiers, and ANY doubt —
//! an inline statement, a sentinel miscount — declines with no edits.
//! Never corrupt; prefer no-op.

mod mask;
mod segment;

use cogitator_lsp::{Position, Range, TextEdit};

#[derive(Debug, Clone, Copy)]
pub struct FormatOptions {
    pub tab_size: u8,
    pub insert_spaces: bool,
}

/// The whole-document edit, `Some(vec![])` when already formatted,
/// `None` when the document is not safely formattable.
/// A file with CRLF line endings (a Windows checkout) is formatted as its
/// LF text and keeps CRLF: line endings alone never count as a change.
pub fn format_document(text: &str, options: &FormatOptions) -> Option<Vec<TextEdit>> {
    if !text.contains("\r\n") {
        return format_lf(text, options);
    }
    let lf = text.replace("\r\n", "\n");
    let mut edits = format_lf(&lf, options)?;
    for edit in &mut edits {
        edit.new_text = edit.new_text.replace('\n', "\r\n");
    }
    Some(edits)
}

fn format_lf(text: &str, options: &FormatOptions) -> Option<Vec<TextEdit>> {
    let segments = segment::split(text)?;
    let mut out = String::with_capacity(text.len());
    for seg in segments {
        match seg {
            segment::Segment::Verbatim(line) => {
                out.push_str(line);
            }
            segment::Segment::Sql(sql) => {
                let formatted = mask::format_masked(sql, options)?;
                out.push_str(&formatted);
                if !formatted.ends_with('\n') && !sql.trim_end_matches('\n').is_empty() {
                    out.push('\n');
                }
            }
        }
    }
    if out.trim_end() == text.trim_end() {
        return Some(Vec::new());
    }
    let end_line = text.lines().count() as u32 + 1;
    Some(vec![TextEdit {
        range: Range {
            start: Position::default(),
            end: Position {
                line: end_line,
                character: 0,
            },
        },
        new_text: out,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crlf_endings_are_no_change_and_are_kept() {
        let options = FormatOptions {
            tab_size: 4,
            insert_spaces: true,
        };
        let lf = "select a,b from {{ ref('x') }} where a>1\n";
        let formatted = format_document(lf, &options).unwrap()[0].new_text.clone();
        let crlf = formatted.replace('\n', "\r\n");
        assert_eq!(format_document(&crlf, &options), Some(Vec::new()));
        let messy = lf.replace('\n', "\r\n");
        let edit = &format_document(&messy, &options).unwrap()[0].new_text;
        assert_eq!(edit, &crlf);
        assert!(!edit.replace("\r\n", "").contains('\n'));
    }

    fn fmt(text: &str) -> Option<String> {
        let options = FormatOptions {
            tab_size: 4,
            insert_spaces: true,
        };
        format_document(text, &options).map(|edits| {
            edits
                .first()
                .map(|e| e.new_text.clone())
                .unwrap_or_else(|| text.to_string())
        })
    }

    #[test]
    fn formats_and_is_idempotent_with_jinja_preserved() {
        let input =
            "{{ config(materialized='table') }}\nselect a,b from {{ ref('stg_x') }} where a>1\n";
        let once = fmt(input).expect("formats");
        assert!(once.contains("{{ ref('stg_x') }}"), "{once}");
        assert!(
            once.contains("{{ config(materialized='table') }}"),
            "{once}"
        );
        assert_ne!(once, input);
        let twice = fmt(&once).expect("formats again");
        assert_eq!(once.trim_end(), twice.trim_end(), "idempotent");
    }

    #[test]
    fn full_line_statements_pass_verbatim() {
        let input =
            "{% set x = 1 %}\nselect {{ x }} as v\n{% if flag %}\nselect 2 as w\n{% endif %}\n";
        let formatted = fmt(input).expect("formats");
        assert!(formatted.contains("{% set x = 1 %}\n"), "{formatted}");
        assert!(formatted.contains("{% if flag %}\n"), "{formatted}");
        assert!(formatted.contains("{% endif %}"), "{formatted}");
    }

    #[test]
    fn inline_statements_decline() {
        assert!(fmt("select 1 {% if x %} ,2 {% endif %} from t\n").is_none());
    }

    #[test]
    fn already_formatted_yields_no_edits() {
        let input = "{{ config(materialized='table') }}\nselect a,b from {{ ref('stg_x') }}\n";
        let options = FormatOptions {
            tab_size: 4,
            insert_spaces: true,
        };
        let once = format_document(input, &options).unwrap();
        let text = &once[0].new_text;
        let again = format_document(text, &options).unwrap();
        assert!(again.is_empty(), "{again:?}");
    }
}
