# cogitator-format

A formatter for dbt models — SQL with Jinja in it — standalone, and the
engine of cogitator's `textDocument/formatting` and `cogitator format`.

- **SQL** is laid out on the parse tree of
  [sqlfluff](https://github.com/sqlfluff/sqlfluff)'s Rust parser
  (sqlfluffrs, vendored in [`vendor/sqlfluffrs`](../../vendor/sqlfluffrs)),
  in any of sqlfluff's 28 dialects, at sqlfluff's indent points.
- **Jinja** is read with [minijinja](https://github.com/mitsuhiko/minijinja)'s
  own tokenizer: tags are laid out again from their tokens, blocks indent
  the lines inside them.
- **It never changes what a model means**: the output is lexed again and
  must hold the same tokens, every tag must come back once, and what the
  parser cannot read is declined, not guessed at.

```sh
cogitator-format models/                  # in place
cogitator-format --check --diff models/   # exit 1 if a file would change
cogitator-format - < model.sql            # stdin to stdout
```

```rust
let config = cogitator_format::Config {
    dialect: Some("duckdb".into()),
    ..Default::default()
};
let out = cogitator_format::format("select a,b from {{ ref('x') }}", &config)?;
assert_eq!(out, "select\n    a,\n    b\nfrom {{ ref('x') }}\n");
```

The rules (`cogitator-format.toml`, sqlfluff's names), how they are found
and when a model is declined: [docs/lint-and-format.md](../../docs/lint-and-format.md#cogitator-format).
