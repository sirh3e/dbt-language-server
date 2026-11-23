//! Snapshots as consumers: the project graph does not list them as
//! dependents, so their files are read here. A snapshot's body is one
//! query between `{% snapshot name %}` and `{% endsnapshot %}`; those two
//! tags are blanked (spaces, newlines kept, so every span stays where it
//! was) and the rest is read like a model. Consumers `ref()` it by name.

use std::path::PathBuf;

use cogitator_parser::{DocumentParser, JinjaKind, JinjaSqlParser, ParsedDocument, TokenKind};

use super::super::references::files::Reader;
use super::super::DbtContext;

pub(super) struct Snapshot {
    pub path: PathBuf,
    pub name: String,
    pub refs: Vec<String>,
    pub masked: String,
}

pub(super) fn snapshot_files(ctx: &DbtContext, reader: &Reader<'_>) -> Vec<Snapshot> {
    let Some(root) = ctx.project_root.as_deref() else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for rel in &ctx.project.snapshot_paths {
        for path in crate::util::walk_filepath(&root.join(rel), ".sql") {
            let Some(text) = reader.read(&path) else {
                continue;
            };
            let Ok(parsed) = JinjaSqlParser.parse(&text, &ctx.dialect) else {
                continue;
            };
            if let Some((name, masked)) = mask(&parsed) {
                let refs = parsed
                    .tokens
                    .iter()
                    .filter(|t| t.kind == TokenKind::Ref)
                    .map(|t| parsed.literal(t).to_string())
                    .collect();
                out.push(Snapshot {
                    path,
                    name,
                    refs,
                    masked,
                });
            }
        }
    }
    out
}

/// The snapshot's name and its text with both tags blanked.
pub(super) fn mask(parsed: &ParsedDocument) -> Option<(String, String)> {
    let tokens = &parsed.tokens;
    let mut name = None;
    let mut blank: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i].kind != TokenKind::JinjaOpen(JinjaKind::Stmt) {
            i += 1;
            continue;
        }
        let close = (i..tokens.len())
            .find(|&j| tokens[j].kind == TokenKind::JinjaClose(JinjaKind::Stmt))?;
        let words: Vec<&str> = tokens[i + 1..close]
            .iter()
            .filter(|t| !matches!(t.kind, TokenKind::Symbol(_)))
            .map(|t| parsed.literal(t))
            .collect();
        match words.as_slice() {
            ["snapshot", n] => {
                name = Some(n.to_string());
                blank.push((tokens[i].start as usize, tokens[close].end as usize));
            }
            ["endsnapshot"] => blank.push((tokens[i].start as usize, tokens[close].end as usize)),
            _ => {}
        }
        i = close + 1;
    }
    let name = name?;
    let mut masked = parsed.text.to_string();
    for (start, end) in blank {
        let spaces: String = masked[start..end]
            .bytes()
            .map(|b| if b == b'\n' { '\n' } else { ' ' })
            .collect();
        masked.replace_range(start..end, &spaces);
    }
    Some((name, masked))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cogitator_dialects::Dialect;

    #[test]
    fn tags_are_blanked_in_place() {
        let text = "{% snapshot orders_snapshot %}\nselect * from {{ ref('stg_orders') }}\n{% endsnapshot %}";
        let parsed = JinjaSqlParser.parse(text, &Dialect::default()).unwrap();
        let (name, masked) = mask(&parsed).unwrap();
        assert_eq!(name, "orders_snapshot");
        assert_eq!(masked.len(), text.len());
        assert!(masked
            .trim()
            .starts_with("select * from {{ ref('stg_orders') }}"));
        assert!(!masked.contains("snapshot %}"));
    }
}
