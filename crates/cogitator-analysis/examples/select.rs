//! What dbt.select answers for a project and a selection, one name a
//! line as `dbt ls --output name` prints them (a source table as
//! `source.table`, a version as `model.v1`), sorted — for
//! `oracles/selection.sh`, which diffs it with dbt's own answer.
//!
//!   cargo run -p cogitator-analysis --example select -- <project dir> "<selection>" ["<exclude>"]

use cogitator_analysis::features::selectors_yml::{select_nodes, SelectRequest};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(dir), Some(select)) = (args.first(), args.get(1)) else {
        eprintln!("usage: select <project dir> <selection> [exclude]");
        std::process::exit(2);
    };
    let ctx = cogitator_analysis::build_context(dir);
    let read = |path: &std::path::Path| std::fs::read_to_string(path).ok();
    let universe = cogitator_analysis::selectors::universe(&ctx, &read);
    let answer = select_nodes(
        &universe,
        &ctx.selectors.definitions,
        &SelectRequest {
            select,
            exclude: args.get(2).map(String::as_str),
            vocabulary: false,
        },
    );
    if let Some(e) = answer.error {
        eprintln!("error: {} at {}..{}", e.message, e.start, e.end);
        std::process::exit(1);
    }
    let mut names: Vec<String> = answer.nodes.into_iter().map(|n| n.label).collect();
    names.sort();
    names.dedup();
    for name in names {
        println!("{name}");
    }
}
