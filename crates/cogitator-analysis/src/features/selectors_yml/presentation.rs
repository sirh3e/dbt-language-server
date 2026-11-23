//! Hover and code lenses over selectors.yml definitions: how many nodes
//! each matches, and the first of them.

use cogitator_lsp::{CodeLens, Command, Position};

use crate::selectors::{self, Definition, Universe};

/// How many names a hover lists before "and N more".
const HOVER_NAMES: usize = 10;

/// The definition whose entry holds `line`.
fn definition_at(defs: &[Definition], line: u32) -> Option<&Definition> {
    let mut starts: Vec<&Definition> = defs.iter().collect();
    starts.sort_by_key(|d| d.range.start.line);
    starts
        .into_iter()
        .rev()
        .find(|d| d.range.start.line <= line)
}

fn count_words(n: usize) -> String {
    match n {
        0 => "no node matches".to_string(),
        1 => "1 node matches".to_string(),
        n => format!("{n} nodes match"),
    }
}

/// Markdown for the definition under the cursor.
pub fn selectors_hover(text: &str, pos: Position, u: &Universe) -> Option<String> {
    let file = selectors::yaml::parse(text);
    let def = definition_at(&file.definitions, pos.line)?;
    let eval = selectors::select_definition(u, &file.definitions, &def.name);
    let nodes = selectors::sorted(u, &eval);
    let mut out = format!("**selector `{}`**", def.name);
    if !def.description.is_empty() {
        out.push_str(&format!(" — {}", def.description));
    }
    if def.default {
        out.push_str(" (default)");
    }
    out.push_str(&format!("\n\n{}", count_words(nodes.len())));
    if !nodes.is_empty() {
        out.push(':');
    }
    out.push('\n');
    for n in nodes.iter().take(HOVER_NAMES) {
        out.push_str(&format!(
            "\n- {} `{}`",
            n.resource_type.as_str(),
            n.display_name()
        ));
    }
    if nodes.len() > HOVER_NAMES {
        out.push_str(&format!("\n- … and {} more", nodes.len() - HOVER_NAMES));
    }
    for issue in &eval.issues {
        out.push_str(&format!("\n\n⚠ {}", selectors::issue_message(issue)));
    }
    Some(out)
}

/// One lens per definition: its count, running `dbt.select` on it.
pub fn selectors_lenses(uri: &str, text: &str, u: &Universe) -> Vec<CodeLens> {
    let file = selectors::yaml::parse(text);
    file.definitions
        .iter()
        .map(|def| {
            let count = selectors::select_definition(u, &file.definitions, &def.name)
                .nodes
                .len();
            CodeLens {
                range: def.range,
                command: Command {
                    title: count_words(count),
                    command: "dbt.select".to_string(),
                    arguments: vec![serde_json::json!({
                        "uri": uri,
                        "selector": format!("selector:{}", def.name),
                    })],
                },
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selectors::{Node, ResourceType};

    const TEXT: &str = "\
selectors:
  - name: one
    description: the orders
    definition: orders
  - name: none
    definition: 'tag:nothing'
";

    fn universe() -> Universe {
        let mut u = Universe::default();
        for i in 0..12 {
            u.push(Node::new(ResourceType::Model, &format!("m{i:02}"), "p"));
        }
        u.push(Node::new(ResourceType::Model, "orders", "p"));
        u
    }

    #[test]
    fn lenses_count_each_definition() {
        let lenses = selectors_lenses("file:///p/selectors.yml", TEXT, &universe());
        let titles: Vec<_> = lenses.iter().map(|l| l.command.title.as_str()).collect();
        assert_eq!(titles, vec!["1 node matches", "no node matches"]);
        assert_eq!(lenses[0].command.arguments[0]["selector"], "selector:one");
        assert_eq!(lenses[0].range.start.line, 1);
    }

    #[test]
    fn hover_lists_the_first_names() {
        let u = universe();
        let hover = selectors_hover(
            TEXT,
            Position {
                line: 3,
                character: 8,
            },
            &u,
        )
        .unwrap();
        assert_eq!(
            hover,
            "**selector `one`** — the orders\n\n1 node matches:\n\n- model `orders`"
        );
        let many = "selectors:\n  - name: all\n    definition: 'resource_type:model'\n";
        let hover = selectors_hover(
            many,
            Position {
                line: 2,
                character: 4,
            },
            &u,
        )
        .unwrap();
        assert!(
            hover.starts_with("**selector `all`**\n\n13 nodes match:"),
            "{hover}"
        );
        assert!(hover.ends_with("- … and 3 more"), "{hover}");
        assert!(selectors_hover(
            TEXT,
            Position {
                line: 0,
                character: 0
            },
            &u
        )
        .is_none());
    }
}
