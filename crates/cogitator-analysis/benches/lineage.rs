//! Analysis benches (09 §5): context build over the shared fixture
//! (cold and scan-cache-warm) and column lineage for a real model.
//! Run with `just bench`; CI compiles them only.

use std::path::PathBuf;

use criterion::{black_box, criterion_group, criterion_main, Criterion};

use cogitator_analysis::{build_context, lineage, ContextBuilder, Documents};

fn fixture_root() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/jaffle_shop_duckdb")
        .to_str()
        .unwrap()
        .to_string()
}

fn context_build(c: &mut Criterion) {
    let root = fixture_root();
    c.bench_function("context_build_cold", |b| {
        b.iter(|| build_context(black_box(&root)))
    });
    c.bench_function("context_build_warm", |b| {
        let mut builder = ContextBuilder::default();
        let _ = builder.build(&root);
        b.iter(|| builder.build(black_box(&root)))
    });
}

fn column_lineage(c: &mut Criterion) {
    let root = fixture_root();
    let ctx = build_context(&root);
    let path = PathBuf::from(&root).join("models/customers.sql");
    let text = std::fs::read_to_string(&path).unwrap();
    let uri = cogitator_lsp::fs_path_to_uri(&path);
    let mut docs = Documents::default();
    docs.open(uri.clone(), &text, &ctx.dialect);
    let doc = docs.get(&uri).unwrap();
    c.bench_function("column_lineage_customers", |b| {
        b.iter(|| lineage::column_lineage(black_box(&doc.parsed), &ctx.dialect))
    });
}

criterion_group!(benches, context_build, column_lineage);
criterion_main!(benches);
