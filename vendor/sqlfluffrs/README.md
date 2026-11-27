# vendor/sqlfluffrs

sqlfluff's Rust lexer and parser — the crates `sqlfluffrs_types`,
`sqlfluffrs_dialects`, `sqlfluffrs_lexer` and `sqlfluffrs_parser` of
[sqlfluff/sqlfluffrs](https://github.com/sqlfluff/sqlfluff/tree/main/sqlfluffrs)
— at the commit [VENDORED.toml](VENDORED.toml) names, for
[cogitator-format](../../crates/cogitator-format).

**Why vendored**: sqlfluffrs is not on crates.io, and its dialect grammars
are not in git: `sqlfluffrs_dialects/build.rs` generates them with Python
(`import sqlfluff`). Here they are generated once, by
[update.sh](update.sh) (`just vendor-sqlfluffrs [<commit>]`), so building
cogitator needs neither Python nor the network.

**What differs from upstream**, and nothing else: the PyO3 bridge's
optional dependencies and features, the dev-dependencies, tests and
benches are dropped; `sqlfluffrs_dialects` has no `build.rs`; the crates
have a workspace of their own (`Cargo.toml` here, outside cogitator's).
**Never edit these sources**: run the script.

sqlfluff is MIT licensed ([LICENSE.md](LICENSE.md)), as cogitator is.
