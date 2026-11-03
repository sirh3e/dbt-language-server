//! Equivalence harness (06 §3.4): scripted ranged edits through the
//! incremental path must tokenize EXACTLY like a fresh full parse of
//! the same final text.

use cogitator_dialects::Dialect;
use cogitator_lsp::{Position, Range};
use cogitator_parser::DocumentParser;
use cogitator_treesitter::TreeSitterParser;

/// A model big enough to clear the 32 KiB incremental gate.
fn big_model() -> String {
    let mut text = String::from("{{ config(materialized='table') }}\nwith base as (\n");
    for i in 0..900 {
        text.push_str(&format!(
            "    select {i} as id, '{i}' as label from {{{{ ref('stg_orders_{i}') }}}} union all\n"
        ));
    }
    text.push_str("    select 0, '0' from {{ ref('stg_orders_0') }}\n)\nselect * from base\n");
    assert!(text.len() > 32 * 1024, "fixture must clear the gate");
    text
}

fn apply(text: &str, range: Range, new_text: &str) -> String {
    let mut lines: Vec<String> = text.split('\n').map(str::to_string).collect();
    let (sl, sc) = (range.start.line as usize, range.start.character as usize);
    let (el, ec) = (range.end.line as usize, range.end.character as usize);
    let tail = lines[el][ec..].to_string();
    let head = lines[sl][..sc].to_string();
    let replacement = format!("{head}{new_text}{tail}");
    lines.splice(sl..=el, replacement.split('\n').map(str::to_string));
    lines.join("\n")
}

fn tokens(doc: &cogitator_parser::ParsedDocument) -> Vec<(String, u32, u32)> {
    doc.tokens
        .iter()
        .map(|t| (format!("{:?}", t.kind), t.start, t.end))
        .collect()
}

#[test]
fn incremental_equals_full_reparse_over_an_edit_script() {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let fresh = TreeSitterParser::new().expect("grammar loads");
    let dialect = Dialect::default();
    let uri = "file:///p/models/big.sql";

    let mut text = big_model();
    let seeded = parser
        .parse_incremental(uri, &text, &dialect, &[])
        .expect("seed parse");
    assert!(!seeded.tokens.is_empty());

    // an edit script: tweak a ref, break a region, repair it, append
    let edits: Vec<(Range, &str)> = vec![
        (
            Range {
                start: Position {
                    line: 5,
                    character: 50,
                },
                end: Position {
                    line: 5,
                    character: 55,
                },
            },
            "stg_X",
        ),
        (
            Range {
                start: Position {
                    line: 40,
                    character: 0,
                },
                end: Position {
                    line: 40,
                    character: 0,
                },
            },
            "    select 99 as id, 'x' as label from {{ ref('fresh_model') }} union all\n",
        ),
        (
            Range {
                start: Position {
                    line: 2,
                    character: 4,
                },
                end: Position {
                    line: 2,
                    character: 10,
                },
            },
            "select",
        ),
    ];
    for (range, new_text) in edits {
        let edit = cogitator_analysis_shim::edit_for_change(&text, range, new_text)
            .expect("clean edit record");
        text = apply(&text, range, new_text);
        let incremental = parser
            .parse_incremental(uri, &text, &dialect, &[edit])
            .expect("incremental parse");
        let full = fresh.parse(&text, &dialect).expect("full parse");
        assert_eq!(
            tokens(&incremental),
            tokens(&full),
            "incremental and full parse diverged"
        );
    }
}

/// `edit_for_change` lives in cogitator-analysis, which depends on this
/// crate's SIBLING (parser) — re-derive the record here to avoid a dev
/// dependency cycle.
mod cogitator_analysis_shim {
    use super::*;

    pub fn edit_for_change(
        before: &str,
        range: Range,
        new_text: &str,
    ) -> Option<cogitator_parser::AppliedEdit> {
        let offset = |line: u32, character: u32| -> Option<usize> {
            let mut acc = 0usize;
            for (i, l) in before.split('\n').enumerate() {
                if i as u32 == line {
                    return (character as usize <= l.len()).then(|| acc + character as usize);
                }
                acc += l.len() + 1;
            }
            None
        };
        let start_byte = offset(range.start.line, range.start.character)?;
        let old_end_byte = offset(range.end.line, range.end.character)?;
        let newlines = new_text.matches('\n').count() as u32;
        let last_len = new_text.rsplit('\n').next().unwrap_or("").len() as u32;
        let new_end_point = if newlines == 0 {
            (range.start.line, range.start.character + last_len)
        } else {
            (range.start.line + newlines, last_len)
        };
        Some(cogitator_parser::AppliedEdit {
            start_byte,
            old_end_byte,
            new_end_byte: start_byte + new_text.len(),
            start_point: (range.start.line, range.start.character),
            old_end_point: (range.end.line, range.end.character),
            new_end_point,
        })
    }
}
