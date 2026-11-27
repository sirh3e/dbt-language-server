//! "Models with a tag ›": the nodes `dbt.select {select: "tag:<t>"}` picks
//! as the next prompt's buttons, each opening its file (Neovim's
//! `:DbtTags`). Pure; the tag is read back from the answer's `selector`.

use serde_json::Value;

use super::menu::{Choice, Prompt, Step};

/// The prompt of the tagged nodes, or what to say when there are none.
/// Only the nodes that carry the tag themselves: the tests dbt's eager
/// indirect selection brings along are left out.
pub fn prompt(answer: &Value) -> Result<Prompt, String> {
    let selector = answer
        .get("selector")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let tag = selector.strip_prefix("tag:").unwrap_or(selector);
    let mut choices: Vec<Choice> = Vec::new();
    for node in answer
        .get("nodes")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let tagged = node
            .get("tags")
            .and_then(Value::as_array)
            .is_some_and(|tags| tags.iter().any(|t| t.as_str() == Some(tag)));
        if !tagged {
            continue;
        }
        let str_of = |key: &str| {
            node.get(key)
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        };
        let Some(label) = str_of("label").or_else(|| str_of("name")) else {
            continue;
        };
        let kind = str_of("resourceType")
            .or_else(|| str_of("kind"))
            .unwrap_or("node");
        let mut title = format!("{label} ({kind})");
        // a button's title is how its answer finds it: never two alike
        let mut n = 2;
        while choices.iter().any(|c| c.title == title) {
            title = format!("{label} ({kind}, {n})");
            n += 1;
        }
        let step = match str_of("uri") {
            Some(uri) => Step::Open {
                uri: uri.to_string(),
                selection: node
                    .get("range")
                    .and_then(|r| serde_json::from_value(r.clone()).ok()),
            },
            None => Step::Say(format!("{label} has no file on disk.")),
        };
        choices.push(Choice { title, step });
    }
    if choices.is_empty() {
        Err(format!("Nothing carries the tag {tag}."))
    } else {
        Ok(Prompt {
            message: format!("Open which node tagged {tag}?"),
            choices,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    fn range(line: u32) -> Value {
        json!({"start": {"line": line, "character": 0}, "end": {"line": line, "character": 3}})
    }

    #[test]
    fn the_tagged_nodes_open_their_files() {
        let answer = json!({
            "selector": "tag:nightly",
            "nodes": [
                {"label": "customers", "name": "customers", "resourceType": "model",
                 "uri": "file:///p/models/customers.sql", "tags": ["mart", "nightly"], "range": range(0)},
                {"label": "unique_customers_id", "name": "unique_customers_id", "resourceType": "test",
                 "uri": "file:///p/models/schema.yml", "tags": [], "range": range(9)},
                {"label": "raw.orders", "resourceType": "source", "uri": "file:///p/models/sources.yml",
                 "tags": ["nightly"], "range": range(4)},
                {"label": "ghost", "resourceType": "model", "uri": null, "tags": ["nightly"]}
            ]
        });
        let prompt = prompt(&answer).unwrap();
        assert_eq!(prompt.message, "Open which node tagged nightly?");
        let titles: Vec<&str> = prompt.choices.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["customers (model)", "raw.orders (source)", "ghost (model)"]
        );
        let Step::Open { uri, selection } = &prompt.choices[1].step else {
            panic!("an open: {:?}", prompt.choices[1].step);
        };
        assert_eq!(uri, "file:///p/models/sources.yml");
        assert_eq!(selection.unwrap().start.line, 4);
        assert_eq!(
            prompt.choices[2].step,
            Step::Say("ghost has no file on disk.".into())
        );
    }

    #[test]
    fn two_nodes_alike_get_two_buttons() {
        let node =
            json!({"label": "a", "resourceType": "model", "uri": "file:///a", "tags": ["t"]});
        let answer = json!({"selector": "tag:t", "nodes": [node.clone(), node]});
        let titles: Vec<String> = prompt(&answer)
            .unwrap()
            .choices
            .into_iter()
            .map(|c| c.title)
            .collect();
        assert_eq!(titles, vec!["a (model)", "a (model, 2)"]);
    }

    #[test]
    fn nothing_tagged_is_said() {
        assert_eq!(
            prompt(&json!({"selector": "tag:x", "nodes": []})),
            Err("Nothing carries the tag x.".into())
        );
    }

    /// The menu's choices this module and the cards add, as the menu offers
    /// them.
    mod in_the_menu {
        use super::super::super::cards::CardKind;
        use super::super::super::menu::{self, Chooser, Here, Present, Run};
        use super::{json, prompt, Prompt, Step};
        use pretty_assertions::assert_eq;

        fn step<'a>(prompt: &'a Prompt, title: &str) -> &'a Step {
            &prompt
                .choices
                .iter()
                .find(|c| c.title == title)
                .unwrap_or_else(|| panic!("no {title:?}"))
                .step
        }

        fn group(here: &Here, title: &str) -> Prompt {
            match step(&menu::root(here), title) {
                Step::Prompt(p) => p.clone(),
                other => panic!("{other:?}"),
            }
        }

        fn run(step: &Step) -> &Run {
            match step {
                Step::Run(r) => r,
                other => panic!("not a run: {other:?}"),
            }
        }

        fn here() -> Here {
            Here {
                uri: "file:///p/models/orders.sql".into(),
                model: Some("orders".into()),
                tags: vec!["daily".into(), "mart".into()],
                ..Here::default()
            }
        }

        #[test]
        fn a_tag_is_picked_then_its_nodes() {
            let project = group(&here(), "Project ›");
            let Step::Prompt(tags) = step(&project, "Models with a tag ›") else {
                panic!("a prompt");
            };
            assert_eq!(tags.message, "Which tag?");
            let mart = run(step(tags, "mart"));
            assert_eq!(mart.command, "dbt.select");
            assert_eq!(mart.argument["select"], "tag:mart");
            assert_eq!(mart.present, Present::Choose(Chooser::Tagged));
            let untagged = Here {
                tags: vec![],
                ..here()
            };
            assert_eq!(
                step(&group(&untagged, "Project ›"), "Models with a tag ›"),
                &Step::Say("No model of the project has a tag.".into())
            );
        }

        #[test]
        fn the_cards_and_the_query_are_model_choices() {
            let model = group(&here(), "Model ›");
            let cases = [
                ("Its card", "dbt.modelInfo", CardKind::Model),
                ("Its model graph", "dbt.modelGraph", CardKind::Model),
                ("Its node card", "dbt.nodeInfo", CardKind::Node),
                (
                    "What it uses and what uses it",
                    "dbt.modelDependencies",
                    CardKind::Dependencies,
                ),
                ("A query of its relation", "dbt.relation", CardKind::Query),
            ];
            for (title, command, kind) in cases {
                let r = run(step(&model, title));
                assert_eq!(
                    (r.command, &r.present),
                    (command, &Present::Card(kind)),
                    "{title}"
                );
            }
        }

        #[test]
        fn the_tagged_chooser_is_this_modules_prompt() {
            let answer = json!({"selector": "tag:t", "nodes": [
                {"label": "a", "resourceType": "model", "uri": "file:///a", "tags": ["t"]}
            ]});
            assert_eq!(
                menu::choose(Chooser::Tagged, "file:///p/m.sql", &answer),
                prompt(&answer)
            );
        }
    }
}
