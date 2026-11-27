//! The dbt menu as dialogs, for a client with no command surface of its own
//! (Zed): the groups of Neovim's `:Dbt` menu, each choice a
//! `window/showMessageRequest` button, walked until a `dbt.*` command runs.
//! What a command needs that a button can give (a CTE, a strategy, a
//! selection, a target, a column, a job) is asked on the way. Pure: the
//! dispatcher sends what this builds and hands back the answers.
//!
//! Every command quoted here is one Zed reaches; the parity test
//! (`crates/cogitator-lsp/tests/consumer_parity.rs`) reads this file for them.

use serde_json::{json, Map, Value};

/// The button that shows the next page of a long prompt.
pub const MORE: &str = "More…";
/// The button that ends a dialog (Zed's close button answers `null`, too).
pub const CANCEL: &str = "Cancel";
/// Choices a page shows before "More…": Zed stacks the buttons, so a page
/// stays shorter than a screen.
pub const PAGE: usize = 9;

/// One prompt: a message and its buttons.
#[derive(Debug, Clone, PartialEq)]
pub struct Prompt {
    pub message: String,
    pub choices: Vec<Choice>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub title: String,
    pub step: Step,
}

/// What choosing a button does.
#[derive(Debug, Clone, PartialEq)]
pub enum Step {
    /// Ask the next question.
    Prompt(Prompt),
    /// Run a `dbt.*` command and present its answer.
    Run(Run),
    /// Say something, and end.
    Say(String),
}

/// A command the dialog runs, and how its answer reaches the reader.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// What the reader chose, for the document's heading and messages.
    pub title: String,
    pub command: &'static str,
    pub argument: Map<String, Value>,
    pub present: Present,
}

/// How a command's answer is shown in a client that only opens documents
/// and shows messages.
#[derive(Debug, Clone, PartialEq)]
pub enum Present {
    /// The command opens or says what it made itself (`show: true`, a
    /// confirmation message, an applied edit): nothing more to do.
    Itself,
    /// A dbt job started (`{token}`): say so; its outcome follows as the
    /// job's own message.
    Started,
    /// The answer as a markdown document.
    Document,
    /// One text field of the answer as a file of its own (`sql`, `md`).
    Field {
        field: &'static str,
        extension: &'static str,
    },
    /// A `Location` to go to.
    Location,
    /// The lineage site written to disk: open its page outside the editor.
    Site,
    /// The answer holds the next question's choices.
    Choose(Chooser),
}

/// The questions whose choices come from a command's answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chooser {
    /// `dbt.profiles` → the targets, then `dbt.setTarget`.
    Targets,
    /// `dbt.columnLineage` → the model's columns, then `dbt.impact`.
    Columns,
}

/// What the menu knows of where it was opened.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Here {
    pub uri: String,
    /// The cursor, as LSP's `{line, character}`.
    pub position: Option<(u32, u32)>,
    /// The model of a model's `.sql` file.
    pub model: Option<String>,
    /// That model's CTEs, in the order they are defined.
    pub ctes: Vec<String>,
    /// The project's tags and `selectors.yml` entries, sorted.
    pub tags: Vec<String>,
    pub selectors: Vec<String>,
    /// The running dbt jobs: token and label.
    pub jobs: Vec<(u64, String)>,
}

/// The incremental strategies a model can be shown with.
pub const STRATEGIES: [&str; 5] = [
    "append",
    "merge",
    "delete+insert",
    "insert_overwrite",
    "microbatch",
];

fn prompt(message: impl Into<String>, choices: Vec<Choice>) -> Prompt {
    Prompt {
        message: message.into(),
        choices,
    }
}

fn ask(title: &str, next: Prompt) -> Choice {
    Choice {
        title: title.to_string(),
        step: Step::Prompt(next),
    }
}

impl Here {
    fn argument(&self, extra: Value) -> Map<String, Value> {
        let mut arg = Map::new();
        arg.insert("uri".into(), Value::String(self.uri.clone()));
        if let Value::Object(extra) = extra {
            arg.extend(extra);
        }
        arg
    }

    fn run(&self, title: &str, command: &'static str, extra: Value, present: Present) -> Choice {
        Choice {
            title: title.to_string(),
            step: Step::Run(Run {
                title: title.to_string(),
                command,
                argument: self.argument(extra),
                present,
            }),
        }
    }

    fn position(&self) -> Value {
        match self.position {
            Some((line, character)) => json!({ "line": line, "character": character }),
            None => json!({ "line": 0, "character": 0 }),
        }
    }
}

/// The first prompt: the groups that apply here.
pub fn root(here: &Here) -> Prompt {
    let mut choices = Vec::new();
    let message = match &here.model {
        Some(model) => {
            choices.push(ask("Model ›", model_group(here, model)));
            format!("dbt — {model}")
        }
        None => "dbt — open a model's .sql file for the Model commands".to_string(),
    };
    choices.push(ask("Project ›", project_group(here)));
    choices.push(ask("Session ›", session_group(here)));
    prompt(message, choices)
}

fn model_group(here: &Here, model: &str) -> Prompt {
    let show = json!({ "show": true });
    let mut ctes: Vec<String> = here.ctes.clone();
    let mut cte_sql: Vec<Choice> = ctes
        .iter()
        .map(|cte| {
            here.run(
                cte,
                "dbt.cteSql",
                json!({ "cte": cte, "show": true }),
                Present::Itself,
            )
        })
        .collect();
    cte_sql.push(here.run(
        "the final select",
        "dbt.cteSql",
        show.clone(),
        Present::Itself,
    ));
    let mut cte_rows: Vec<Choice> = ctes
        .drain(..)
        .map(|cte| {
            let title = cte.clone();
            here.run(
                &title,
                "dbt.previewCte",
                json!({ "cte": cte }),
                Present::Field {
                    field: "table",
                    extension: "md",
                },
            )
        })
        .collect();
    cte_rows.push(here.run(
        "the final select",
        "dbt.previewCte",
        json!({}),
        Present::Field {
            field: "table",
            extension: "md",
        },
    ));
    let mut strategies = vec![here.run(
        "as configured",
        "dbt.incrementalSql",
        show.clone(),
        Present::Itself,
    )];
    strategies.extend(STRATEGIES.iter().map(|strategy| {
        here.run(
            strategy,
            "dbt.incrementalSql",
            json!({ "strategy": strategy, "show": true }),
            Present::Itself,
        )
    }));
    let doc = || Present::Document;
    prompt(
        format!("dbt — {model}"),
        vec![
            here.run("Run it (dbt run)", "dbt.run", json!({}), Present::Started),
            here.run(
                "Test it (dbt test)",
                "dbt.test",
                json!({}),
                Present::Started,
            ),
            here.run(
                "Compile it, open the compiled SQL",
                "dbt.compile",
                json!({}),
                Present::Started,
            ),
            here.run(
                "Preview its rows (dbt show)",
                "dbt.preview",
                json!({}),
                Present::Started,
            ),
            here.run(
                "Its Jinja rendered",
                "dbt.expandJinja",
                json!({}),
                Present::Field {
                    field: "sql",
                    extension: "sql",
                },
            ),
            here.run(
                "Its compiled SQL (last compile)",
                "dbt.compiledSql",
                json!({}),
                Present::Field {
                    field: "content",
                    extension: "sql",
                },
            ),
            here.run(
                "The SQL each of its tests runs",
                "dbt.testSql",
                show.clone(),
                Present::Itself,
            ),
            ask(
                "A CTE's SQL, no warehouse ›",
                prompt(format!("The SQL of which CTE of {model}?"), cte_sql),
            ),
            ask(
                "A CTE's rows ›",
                prompt(format!("The rows of which CTE of {model}?"), cte_rows),
            ),
            ask(
                "What an incremental run executes ›",
                prompt("With which strategy?", strategies),
            ),
            here.run(
                "Incremental run against a full refresh",
                "dbt.incrementalDiff",
                show,
                Present::Itself,
            ),
            here.run("Its query plan", "dbt.queryPlan", json!({}), doc()),
            here.run("Its column lineage", "dbt.columnLineage", json!({}), doc()),
            here.run(
                "Downstream impact of a column ›",
                "dbt.columnLineage",
                json!({}),
                Present::Choose(Chooser::Columns),
            ),
            here.run(
                "Breaking changes against the last build",
                "dbt.breakingChanges",
                json!({}),
                doc(),
            ),
            here.run(
                "What it uses and what uses it",
                "dbt.modelDependencies",
                json!({}),
                doc(),
            ),
            here.run(
                "Its model graph",
                "dbt.modelGraph",
                json!({ "model": model }),
                doc(),
            ),
            here.run("Its card", "dbt.modelInfo", json!({}), doc()),
            here.run(
                "Its node card",
                "dbt.nodeInfo",
                json!({ "node": { "kind": "model", "name": model } }),
                doc(),
            ),
            here.run("Its tests", "dbt.modelTests", json!({}), doc()),
            here.run("Its relation", "dbt.relation", json!({}), doc()),
            here.run(
                "Its schema YAML entry",
                "dbt.goToSchema",
                json!({ "position": here.position() }),
                Present::Location,
            ),
            here.run(
                "Its lineage as a message",
                "dbt.showLineage",
                json!({}),
                Present::Itself,
            ),
            here.run(
                "Extract it into schema YAML",
                "dbt.extractModelYaml",
                json!({}),
                Present::Itself,
            ),
            here.run(
                "Sync its schema YAML: what would change",
                "dbt.syncYaml",
                json!({}),
                doc(),
            ),
            here.run(
                "Sync its schema YAML now",
                "dbt.syncYaml",
                json!({ "apply": true }),
                doc(),
            ),
            here.run(
                "Split it at its incrementality frontier: the plan",
                "dbt.splitPlan",
                json!({}),
                Present::Field {
                    field: "sexp",
                    extension: "lisp",
                },
            ),
            here.run("Its syntax tree", "dbt.syntaxTree", json!({}), doc()),
        ],
    )
}

/// The selections a run or a listing can take.
fn selections(here: &Here, make: impl Fn(&str, String) -> Choice) -> Vec<Choice> {
    let mut out = Vec::new();
    if let Some(model) = &here.model {
        out.push(make(&format!("{model} (this model)"), model.clone()));
        out.push(make(
            &format!("{model}+ (and downstream)"),
            format!("{model}+"),
        ));
        out.push(make(
            &format!("+{model} (and upstream)"),
            format!("+{model}"),
        ));
    }
    out.extend(
        here.selectors
            .iter()
            .map(|s| make(&format!("selector:{s}"), format!("selector:{s}"))),
    );
    out.extend(
        here.tags
            .iter()
            .map(|t| make(&format!("tag:{t}"), format!("tag:{t}"))),
    );
    out
}

fn project_group(here: &Here) -> Prompt {
    let doc = || Present::Document;
    let kinds = ["run", "test", "build", "compile"]
        .into_iter()
        .map(|kind| {
            let choices = selections(here, |title, selector| {
                here.run(
                    title,
                    "dbt.runSelection",
                    json!({ "selector": selector, "kind": kind }),
                    Present::Started,
                )
            });
            let step = if choices.is_empty() {
                Step::Say("Nothing to select: no model is open and the project has no tags or selectors.yml entries.".into())
            } else {
                Step::Prompt(prompt(format!("dbt {kind} --select …"), choices))
            };
            Choice {
                title: format!("dbt {kind}"),
                step,
            }
        })
        .collect();
    let listing = selections(here, |title, selector| {
        here.run(title, "dbt.select", json!({ "select": selector }), doc())
    });
    let jobs: Vec<Choice> = here
        .jobs
        .iter()
        .map(|(token, label)| {
            here.run(
                &format!("{label} (job {token})"),
                "dbt.cancel",
                json!({ "token": token }),
                Present::Itself,
            )
        })
        .collect();
    let some = |title: &str, choices: Vec<Choice>, question: &str, none: &str| Choice {
        title: title.to_string(),
        step: if choices.is_empty() {
            Step::Say(none.to_string())
        } else {
            Step::Prompt(prompt(question, choices))
        },
    };
    prompt(
        "dbt — the project",
        vec![
            here.run("dbt parse", "dbt.parse", json!({}), Present::Started),
            here.run(
                "dbt source freshness",
                "dbt.sourceFreshness",
                json!({}),
                Present::Started,
            ),
            ask(
                "Run a selection ›",
                prompt("Which dbt command?", kinds),
            ),
            some(
                "List a selection's nodes ›",
                listing,
                "Which selection?",
                "Nothing to select: no model is open and the project has no tags or selectors.yml entries.",
            ),
            some(
                "Cancel a dbt job ›",
                jobs,
                "Cancel which job?",
                "No dbt job is running.",
            ),
            here.run("Models nothing uses", "dbt.deadModels", json!({}), doc()),
            here.run(
                "Every unresolved name",
                "dbt.unresolved",
                json!({}),
                doc(),
            ),
            here.run(
                "Table models that could be incremental",
                "dbt.incrementality",
                json!({}),
                doc(),
            ),
            here.run("The project graph", "dbt.projectGraph", json!({}), doc()),
            here.run(
                "The lineage site (opens in the browser)",
                "dbt.lineageSite",
                json!({ "write": true }),
                Present::Site,
            ),
        ],
    )
}

fn session_group(here: &Here) -> Prompt {
    let doc = || Present::Document;
    prompt(
        "dbt — the session",
        vec![
            here.run("What the server knows", "dbt.status", json!({}), doc()),
            here.run("Health report", "dbt.doctor", json!({}), doc()),
            here.run(
                "Switch profile / target ›",
                "dbt.profiles",
                json!({}),
                Present::Choose(Chooser::Targets),
            ),
            here.run("The profiles", "dbt.profiles", json!({}), doc()),
            here.run("Cache counters", "dbt.cache", json!({}), doc()),
            here.run(
                "Clear the caches",
                "dbt.cache",
                json!({ "action": "clear" }),
                doc(),
            ),
            here.run(
                "Reload cogitator.yml",
                "dbt.reloadConfig",
                json!({}),
                Present::Itself,
            ),
        ],
    )
}

/// The next question from a command's answer, or what to say when the
/// answer offers nothing to choose.
pub fn choose(chooser: Chooser, uri: &str, answer: &Value) -> Result<Prompt, String> {
    let here = Here {
        uri: uri.to_string(),
        ..Here::default()
    };
    match chooser {
        Chooser::Targets => {
            let active = (
                answer.get("activeProfile").and_then(Value::as_str),
                answer.get("activeTarget").and_then(Value::as_str),
            );
            let mut choices = Vec::new();
            for profile in answer
                .get("profiles")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(name) = profile.get("name").and_then(Value::as_str) else {
                    continue;
                };
                if profile.get("allowed").and_then(Value::as_bool) == Some(false) {
                    continue;
                }
                for target in profile
                    .get("targets")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(target) = target.get("name").and_then(Value::as_str) else {
                        continue;
                    };
                    let on = active == (Some(name), Some(target));
                    let title = format!("{name} › {target}{}", if on { " ✓" } else { "" });
                    choices.push(Choice {
                        title: title.clone(),
                        step: Step::Run(Run {
                            title,
                            command: "dbt.setTarget",
                            argument: json!({ "profile": name, "target": target })
                                .as_object()
                                .cloned()
                                .unwrap_or_default(),
                            present: Present::Itself,
                        }),
                    });
                }
            }
            if choices.is_empty() {
                Err("No profile target to switch to: no profiles.yml was found, or none is allowed.".into())
            } else {
                Ok(prompt("Switch to which profile and target?", choices))
            }
        }
        Chooser::Columns => {
            let mut choices = vec![here.run(
                "the whole model",
                "dbt.impact",
                json!({}),
                Present::Document,
            )];
            choices.extend(
                answer
                    .get("columns")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(|c| c.get("name").and_then(Value::as_str))
                    .filter(|name| *name != "*")
                    .map(|name| {
                        here.run(
                            name,
                            "dbt.impact",
                            json!({ "column": name }),
                            Present::Document,
                        )
                    }),
            );
            Ok(prompt("The downstream impact of which column?", choices))
        }
    }
}

/// A prompt as it stands on screen: which page of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Shown {
    pub prompt: Prompt,
    pub offset: usize,
}

/// What an answer to a shown prompt asks for.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// Closed, cancelled, or a button this prompt does not have.
    Nothing,
    /// "More…": the next page of the same prompt.
    Page(Shown),
    Step(Step),
}

impl Shown {
    pub fn new(prompt: Prompt) -> Self {
        Shown { prompt, offset: 0 }
    }

    fn page(&self) -> &[Choice] {
        let end = (self.offset + PAGE).min(self.prompt.choices.len());
        &self.prompt.choices[self.offset.min(end)..end]
    }

    fn has_more(&self) -> bool {
        self.offset + PAGE < self.prompt.choices.len()
    }

    /// The message, with which choices of how many when the prompt pages.
    pub fn message(&self) -> String {
        let total = self.prompt.choices.len();
        if total <= PAGE {
            return self.prompt.message.clone();
        }
        let shown = self.page().len();
        format!(
            "{} ({}–{} of {total})",
            self.prompt.message,
            self.offset + 1,
            self.offset + shown
        )
    }

    /// The buttons of this page: its choices, "More…" when there are more,
    /// and "Cancel".
    pub fn titles(&self) -> Vec<String> {
        let mut titles: Vec<String> = self.page().iter().map(|c| c.title.clone()).collect();
        if self.has_more() {
            titles.push(MORE.to_string());
        }
        titles.push(CANCEL.to_string());
        titles
    }

    /// What the reader's button (`None` when the prompt was closed) asks for.
    pub fn answer(self, title: Option<&str>) -> Answer {
        let Some(title) = title else {
            return Answer::Nothing;
        };
        if title == MORE && self.has_more() {
            return Answer::Page(Shown {
                offset: self.offset + PAGE,
                prompt: self.prompt,
            });
        }
        let Shown { prompt, offset } = self;
        let end = (offset + PAGE).min(prompt.choices.len());
        prompt
            .choices
            .into_iter()
            .skip(offset)
            .take(end.saturating_sub(offset))
            .find(|c| c.title == title)
            .map(|c| Answer::Step(c.step))
            .unwrap_or(Answer::Nothing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn here() -> Here {
        Here {
            uri: "file:///p/models/orders.sql".into(),
            position: Some((3, 4)),
            model: Some("orders".into()),
            ctes: vec!["source".into(), "renamed".into()],
            tags: vec!["daily".into()],
            selectors: vec!["nightly".into()],
            jobs: vec![],
        }
    }

    fn titles(prompt: &Prompt) -> Vec<&str> {
        prompt.choices.iter().map(|c| c.title.as_str()).collect()
    }

    fn sub<'a>(prompt: &'a Prompt, title: &str) -> &'a Step {
        &prompt
            .choices
            .iter()
            .find(|c| c.title == title)
            .unwrap_or_else(|| panic!("no choice {title:?} in {:?}", titles(prompt)))
            .step
    }

    fn prompt_of(step: &Step) -> &Prompt {
        match step {
            Step::Prompt(p) => p,
            other => panic!("not a prompt: {other:?}"),
        }
    }

    fn run_of(step: &Step) -> &Run {
        match step {
            Step::Run(r) => r,
            other => panic!("not a run: {other:?}"),
        }
    }

    #[test]
    fn the_groups_follow_the_file() {
        assert_eq!(
            titles(&root(&here())),
            vec!["Model ›", "Project ›", "Session ›"]
        );
        let yaml = Here {
            uri: "file:///p/models/schema.yml".into(),
            ..Here::default()
        };
        let menu = root(&yaml);
        assert_eq!(titles(&menu), vec!["Project ›", "Session ›"]);
        assert!(menu.message.contains("open a model's .sql file"));
    }

    #[test]
    fn a_ctes_sql_is_asked_per_cte_and_shown_by_the_server() {
        let model = prompt_of(sub(&root(&here()), "Model ›")).clone();
        let ctes = prompt_of(sub(&model, "A CTE's SQL, no warehouse ›")).clone();
        assert_eq!(titles(&ctes), vec!["source", "renamed", "the final select"]);
        let run = run_of(sub(&ctes, "renamed"));
        assert_eq!(run.command, "dbt.cteSql");
        assert_eq!(
            Value::Object(run.argument.clone()),
            json!({ "uri": "file:///p/models/orders.sql", "cte": "renamed", "show": true })
        );
        assert_eq!(run.present, Present::Itself);
        let last = run_of(sub(&ctes, "the final select"));
        assert!(!last.argument.contains_key("cte"));
    }

    #[test]
    fn the_strategies_are_offered_with_the_configured_one_first() {
        let model = prompt_of(sub(&root(&here()), "Model ›")).clone();
        let strategies = prompt_of(sub(&model, "What an incremental run executes ›")).clone();
        assert_eq!(
            titles(&strategies),
            vec![
                "as configured",
                "append",
                "merge",
                "delete+insert",
                "insert_overwrite",
                "microbatch"
            ]
        );
        let merge = run_of(sub(&strategies, "merge"));
        assert_eq!(merge.argument["strategy"], "merge");
        assert!(!run_of(sub(&strategies, "as configured"))
            .argument
            .contains_key("strategy"));
    }

    #[test]
    fn a_selection_run_asks_the_command_then_the_selection() {
        let project = prompt_of(sub(&root(&here()), "Project ›")).clone();
        let kinds = prompt_of(sub(&project, "Run a selection ›")).clone();
        assert_eq!(
            titles(&kinds),
            vec!["dbt run", "dbt test", "dbt build", "dbt compile"]
        );
        let selections = prompt_of(sub(&kinds, "dbt build")).clone();
        assert_eq!(
            titles(&selections),
            vec![
                "orders (this model)",
                "orders+ (and downstream)",
                "+orders (and upstream)",
                "selector:nightly",
                "tag:daily"
            ]
        );
        let run = run_of(sub(&selections, "tag:daily"));
        assert_eq!(run.command, "dbt.runSelection");
        assert_eq!(run.argument["selector"], "tag:daily");
        assert_eq!(run.argument["kind"], "build");
        assert_eq!(run.present, Present::Started);
    }

    #[test]
    fn nothing_to_choose_is_said_instead_of_an_empty_prompt() {
        let project = prompt_of(sub(&root(&here()), "Project ›")).clone();
        assert_eq!(
            sub(&project, "Cancel a dbt job ›"),
            &Step::Say("No dbt job is running.".into())
        );
        let busy = Here {
            jobs: vec![(7, "dbt run orders".into())],
            ..here()
        };
        let project = prompt_of(sub(&root(&busy), "Project ›")).clone();
        let jobs = prompt_of(sub(&project, "Cancel a dbt job ›")).clone();
        assert_eq!(titles(&jobs), vec!["dbt run orders (job 7)"]);
        assert_eq!(
            run_of(sub(&jobs, "dbt run orders (job 7)")).argument["token"],
            7
        );
    }

    #[test]
    fn the_targets_come_from_the_profiles_answer() {
        let answer = json!({
            "activeProfile": "shop", "activeTarget": "dev",
            "profiles": [
                {"name": "shop", "allowed": true, "targets": [{"name": "dev"}, {"name": "prod"}]},
                {"name": "other", "allowed": false, "targets": [{"name": "x"}]}
            ]
        });
        let prompt = choose(Chooser::Targets, "file:///p/m.sql", &answer).unwrap();
        assert_eq!(titles(&prompt), vec!["shop › dev ✓", "shop › prod"]);
        let run = run_of(sub(&prompt, "shop › prod"));
        assert_eq!(
            Value::Object(run.argument.clone()),
            json!({ "profile": "shop", "target": "prod" })
        );
        assert!(choose(Chooser::Targets, "", &json!({ "profiles": [] })).is_err());
    }

    #[test]
    fn the_columns_come_from_the_lineage_answer() {
        let answer =
            json!({ "columns": [{"name": "order_id"}, {"name": "*"}, {"name": "amount"}] });
        let prompt = choose(Chooser::Columns, "file:///p/models/orders.sql", &answer).unwrap();
        assert_eq!(
            titles(&prompt),
            vec!["the whole model", "order_id", "amount"]
        );
        let run = run_of(sub(&prompt, "amount"));
        assert_eq!(run.command, "dbt.impact");
        assert_eq!(run.argument["column"], "amount");
        assert_eq!(run.argument["uri"], "file:///p/models/orders.sql");
    }

    #[test]
    fn a_long_prompt_pages() {
        let choices: Vec<Choice> = (0..20)
            .map(|i| Choice {
                title: format!("c{i}"),
                step: Step::Say(format!("{i}")),
            })
            .collect();
        let shown = Shown::new(prompt("Pick", choices));
        assert_eq!(shown.message(), "Pick (1–9 of 20)");
        let titles = shown.titles();
        assert_eq!(titles.len(), PAGE + 2);
        assert_eq!(&titles[PAGE..], &[MORE.to_string(), CANCEL.to_string()]);
        let Answer::Page(second) = shown.clone().answer(Some(MORE)) else {
            panic!("a page");
        };
        assert_eq!(second.message(), "Pick (10–18 of 20)");
        let Answer::Page(third) = second.clone().answer(Some(MORE)) else {
            panic!("a page");
        };
        assert_eq!(third.titles(), vec!["c18", "c19", CANCEL]);
        assert_eq!(
            third.clone().answer(Some("c19")),
            Answer::Step(Step::Say("19".into()))
        );
        // a button of another page, Cancel and a closed prompt do nothing
        assert_eq!(third.clone().answer(Some("c0")), Answer::Nothing);
        assert_eq!(third.clone().answer(Some(CANCEL)), Answer::Nothing);
        assert_eq!(third.answer(None), Answer::Nothing);
    }

    #[test]
    fn every_page_of_the_menu_fits_with_unique_buttons() {
        fn walk(prompt: &Prompt) {
            let mut shown = Shown::new(prompt.clone());
            loop {
                let titles = shown.titles();
                let mut unique = titles.clone();
                unique.sort();
                unique.dedup();
                assert_eq!(unique.len(), titles.len(), "{titles:?}");
                assert!(titles.len() <= PAGE + 2);
                match shown.clone().answer(Some(MORE)) {
                    Answer::Page(next) => shown = next,
                    _ => break,
                }
            }
            for choice in &prompt.choices {
                if let Step::Prompt(p) = &choice.step {
                    walk(p);
                }
            }
        }
        walk(&root(&Here {
            jobs: vec![(1, "a".into()), (2, "b".into())],
            ..here()
        }));
    }
}
