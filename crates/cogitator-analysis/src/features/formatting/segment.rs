//! Document segmentation: full-line Jinja statements/comments are
//! emitted verbatim; everything between them formats as SQL. An
//! inline `{% %}` (mixed into a SQL line) makes the whole document
//! unformattable — None, by design.

pub enum Segment<'a> {
    /// A verbatim line (including its newline).
    Verbatim(&'a str),
    /// A run of SQL text.
    Sql(&'a str),
}

/// Split into segments, or None when any `{% %}` sits inside a mixed
/// line.
pub fn split(text: &str) -> Option<Vec<Segment<'_>>> {
    let mut segments = Vec::new();
    let mut sql_start = 0usize;
    let mut offset = 0usize;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim();
        let is_statement_line = !trimmed.is_empty() && statements_only(trimmed);
        let has_statement = line.contains("{%") || line.contains("{#");
        if is_statement_line {
            if offset > sql_start {
                segments.push(Segment::Sql(&text[sql_start..offset]));
            }
            segments.push(Segment::Verbatim(line));
            sql_start = offset + line.len();
        } else if has_statement {
            return None; // inline statement: formatting would reflow through it
        }
        offset += line.len();
    }
    if text.len() > sql_start {
        segments.push(Segment::Sql(&text[sql_start..]));
    }
    Some(segments)
}

/// Is this trimmed line nothing but `{% … %}` / `{# … #}` blocks?
fn statements_only(mut line: &str) -> bool {
    loop {
        line = line.trim_start();
        if line.is_empty() {
            return true;
        }
        let close = if line.starts_with("{%") {
            "%}"
        } else if line.starts_with("{#") {
            "#}"
        } else {
            return false;
        };
        match line.find(close) {
            Some(end) => line = &line[end + close.len()..],
            None => return false, // unterminated: not a clean statement line
        }
    }
}
