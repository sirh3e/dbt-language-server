//! textDocument/codeLens (FEATURE_REQUESTS lens batch): model files get
//! upstream/downstream counts plus entry points for schema, lineage and
//! per-model dbt execution; macro files get per-macro call-site counts.

use cogitator_lsp::Range;
use serde::Serialize;

use super::{model_name_from_uri, DbtContext};

/// The single argument object every executable lens carries.
#[derive(Debug, Serialize)]
struct CodeLensArgument {
    uri: String,
}

pub fn code_lenses(ctx: &DbtContext, uri: &str) -> Vec<cogitator_lsp::CodeLens> {
    let mut lenses = model_lenses(ctx, uri);
    lenses.extend(macro_lenses(ctx, uri));
    lenses
}

fn model_lenses(ctx: &DbtContext, uri: &str) -> Vec<cogitator_lsp::CodeLens> {
    let Some(model) = model_name_from_uri(uri) else {
        return Vec::new();
    };
    if !ctx.model_detail_map.contains_key(&model) {
        return Vec::new();
    }
    let downstream = ctx.graph.downstream_of(&model).count();
    let upstream = ctx.graph.upstream_of(&model).count();
    let arg = serde_json::to_value(CodeLensArgument {
        uri: uri.to_string(),
    })
    .expect("lens argument serializes");

    let display = |title: String| lens(title, "", Vec::new());
    let command =
        |title: &str, command: &'static str| lens(title.to_string(), command, vec![arg.clone()]);

    let mut lenses = vec![
        display(plural(downstream, "downstream")),
        display(plural(upstream, "upstream")),
        command("Go to schema", "dbt.openSchema"),
        command("Column lineage", "dbt.showLineage"),
        command("dbt run", "dbt.run"),
        command("dbt test", "dbt.test"),
        command("dbt compile", "dbt.compile"),
    ];
    match ctx
        .model_detail_map
        .get(&model)
        .and_then(|d| d.schema_uri.as_ref())
    {
        // no entry yet: offer the scaffold instead of a test count
        None => lenses.push(command("Extract schema YAML", "dbt.extractModelYaml")),
        Some(_) => {
            if let Some((_, tests)) = super::model_tests(ctx, uri) {
                if !tests.is_empty() {
                    lenses.push(command(
                        &format!(
                            "{} test{}",
                            tests.len(),
                            if tests.len() == 1 { "" } else { "s" }
                        ),
                        "dbt.goToTests",
                    ));
                }
            }
        }
    }
    lenses
}

/// One "N call sites" lens per macro defined in this file, anchored at the
/// macro's signature.
fn macro_lenses(ctx: &DbtContext, uri: &str) -> Vec<cogitator_lsp::CodeLens> {
    let path = cogitator_lsp::uri_to_fs_path(uri).unwrap_or_else(|| uri.to_string());
    let mut lenses: Vec<cogitator_lsp::CodeLens> = ctx
        .macro_detail_map
        .values()
        .flat_map(|macros| macros.values())
        // Path equality compares components, so `/` and `\` separators match
        .filter(|m| m.uri.as_path() == std::path::Path::new(path.as_str()))
        .map(|m| {
            let count = ctx.graph.macro_call_sites(&m.name).count();
            cogitator_lsp::CodeLens {
                range: m.range,
                command: cogitator_lsp::Command {
                    title: plural(count, "call site"),
                    command: String::new(),
                    arguments: Vec::new(),
                },
            }
        })
        .collect();
    lenses.sort_by_key(|l| (l.range.start.line, l.range.start.character));
    lenses
}

fn plural(n: usize, noun: &str) -> String {
    match n {
        1 => format!("1 {noun}"),
        n => format!("{n} {noun}s"),
    }
}

fn lens(
    title: String,
    command: &str,
    arguments: Vec<serde_json::Value>,
) -> cogitator_lsp::CodeLens {
    cogitator_lsp::CodeLens {
        range: Range::default(),
        command: cogitator_lsp::Command {
            title,
            command: command.to_string(),
            arguments,
        },
    }
}
