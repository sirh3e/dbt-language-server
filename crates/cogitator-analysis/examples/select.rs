//! What dbt.select answers for a project and a selection, one name a
//! line as `dbt ls --output name` prints them (a source table as
//! `source.table`), sorted — for `just selection-oracle`, which diffs it
//! with dbt's own answer.
//!
//!   cargo run -p cogitator-analysis --example select -- <project dir> "<selection>" ["<exclude>"]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(dir), Some(select)) = (args.first(), args.get(1)) else {
        eprintln!("usage: select <project dir> <selection> [exclude]");
        std::process::exit(2);
    };
    let root = std::path::Path::new(dir);
    let ctx = cogitator_analysis::build_context(dir);
    let manifest_path = root.join("target/manifest.json");
    let manifest = std::fs::read_to_string(&manifest_path)
        .ok()
        .and_then(|t| cogitator_analysis::artifacts::manifest::parse(&t, &manifest_path).ok());
    let universe = cogitator_analysis::features::selection::build_universe(&ctx, manifest.as_ref());
    let result = cogitator_analysis::features::selection::select(
        &ctx,
        &universe,
        select,
        args.get(2).map(String::as_str),
    );
    if let Some(e) = result.error {
        eprintln!("error: {} at {}..{}", e.message, e.start, e.end);
        std::process::exit(1);
    }
    let mut names: Vec<String> = result.nodes.iter().map(|n| n.listed_name()).collect();
    names.sort();
    names.dedup();
    for name in names {
        println!("{name}");
    }
}
