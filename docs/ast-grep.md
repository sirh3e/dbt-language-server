# ast-grep for dbt

[ast-grep](https://ast-grep.github.io) searches, rewrites and lints code by
its syntax tree rather than its text. cogitator brings it to dbt projects
on the grammar the server already parses with,
[tree-sitter-dbt](https://github.com/sirh3e/tree-sitter-dbt): structural
search and rewrite through your editor, your project's ast-grep rules as
diagnostics, and the same rules in the ast-grep CLI for CI.

- [What a pattern matches](#what-a-pattern-matches)
- [Searching from the editor](#searching-from-the-editor)
- [Rules as diagnostics](#rules-as-diagnostics)
- [The ast-grep CLI](#the-ast-grep-cli)
- [Highlighting](#highlighting)

## What a pattern matches

Two languages:

| Language | Grammar | Files | Matches |
|---|---|---|---|
| `dbt` (default) | tree-sitter-dbt | models, macros, snapshots, tests, analyses | the **Jinja**: `{{ ref($M) }}`, `{{ config($$$ARGS) }}`, `{% if is_incremental() %}` |
| `sql` | tree-sitter-sequel | `target/compiled`, `target/run` | the **SQL** dbt compiled to: `select $$$C from $T` |

To tree-sitter-dbt, the SQL between the tags is one stretch of text, so a
`dbt` pattern cannot match SQL structure — `select $A from $T` finds
nothing in a model. Search the compiled SQL for that, with `sql`.

A `sql` pattern must be a **whole statement**: the grammar parses nothing
smaller, so `count(*)` alone matches nothing. For a smaller piece, write a
rule with ast-grep's `context` and `selector`.

Metavariables are written as everywhere in ast-grep: `$M` one node,
`$$$ARGS` any number of them, `$_` one that is not captured. Neither
grammar accepts `$` in a name, so patterns are parsed with `_` in its
place (the *expando character*); `_M` in a pattern is the same as `$M`.

## Searching from the editor

| | Search | Rewrite |
|---|---|---|
| Neovim | `:DbtAstGrep {{ ref($M) }}` → quickfix; `:DbtFind astgrep` / `<leader>da` → the picker | `:DbtAstGrep {{ ref($M) }} => {{ ref('core', $M) }}` lists the matches and asks |
| Emacs | `M-x cogitator-ast-grep` → a grep buffer | the same `pattern => rewrite`, after `y-or-n-p` |
| VS Code | "dbt: Structural search (ast-grep)" → a quick pick | the same, after a confirmation |

Prefix the pattern with `sql:` to search the compiled SQL:
`sql: select * from $T`. Behind all three is the `dbt.astGrep` command
([commands.md](commands.md#dbtastgrep)): at most 500 matches, each with its
range, its text, the metavariables it bound and, with a rewrite, what it
becomes; with `apply: true` the server sends the edits through
`workspace/applyEdit`, so your editor applies (and can undo) them.

## Rules as diagnostics

Put an `sgconfig.yml` next to `dbt_project.yml` that names rule directories:

```yaml
ruleDirs:
  - rules
```

Every rule of language `dbt` or `sql` in them is a diagnostic in your
editor (`source: ast-grep`, the rule's id as its code, its `note` under the
message), and a rule's `fix` is a quickfix code action. Rules of other
languages are left to the CLI. The server reads the rules again whenever
`sgconfig.yml` or a rule file changes; `:checkhealth cogitator` /
`dbt.doctor` lists the rules it loaded and any it could not read.

A rule, from [`editors/ast-grep/rules`](../editors/ast-grep/rules):

```yaml
id: this-outside-incremental-guard
language: dbt
severity: warning
message: "{{ this }} outside {% if is_incremental() %}: the first build has no table to read"
note: read the model's own table only inside the incremental guard
rule:
  pattern: "{{ this }}"
  not:
    inside:
      kind: if_statement
      stopBy: end
```

`files:` and `ignores:` globs work as in the CLI. Node kinds (`if_statement`,
`for_statement`, `call`, …) are the grammar's: `:DbtSyntaxTree` shows them
for the current buffer.

## The ast-grep CLI

The same rules run in the ast-grep CLI, for CI or a pre-commit hook.

1. Build the grammars as shared libraries, from the sources the server was
   built with: `just ast-grep-libs` (or `sh editors/ast-grep/build-libs.sh
   [out-dir]`, which needs `cargo`, `python3` and a C compiler) writes
   `target/ast-grep/dbt.so` and `sql.so` (`.dylib` on macOS).
2. Copy [`sgconfig.yml`](../editors/ast-grep/sgconfig.yml),
   [`sgconfig.compiled.yml`](../editors/ast-grep/sgconfig.compiled.yml)
   and [`rules/`](../editors/ast-grep/rules) to your project, and point
   `libraryPath` at the libraries.
3. Run:

```sh
ast-grep scan                                            # dbt rules over the project
ast-grep scan -c sgconfig.compiled.yml target/compiled   # sql rules over the compiled SQL
ast-grep run -l dbt -p '{{ ref($M) }}' models/
```

Why two configs: the CLI gives a file extension to one language only, and
(as of ast-grep 0.42) its `languageGlobs` do not reach custom languages, so
`.sql` is dbt in `sgconfig.yml` and plain SQL in `sgconfig.compiled.yml`.
The server needs no such split: it reads `target/compiled` and
`target/run` as SQL and everything else as dbt.

The example rules are tested twice: by the server's crate
(`crates/cogitator-astgrep/tests/example_rules.rs`) on
`editors/ast-grep/tests/`, and they give the same findings in the CLI.

## Highlighting

The same grammar colours code: `dbt.highlight` returns tree-sitter-dbt's
own highlights (the grammar's `highlights.scm`, with the SQL between the
tags through the SQL grammar it injects). The Neovim explorer's dbt SQL
and compiled SQL panes and the picker previews are painted with it,
`:DbtHighlight` colours any buffer, and Emacs (`M-x
cogitator-highlight-buffer`) and VS Code ("dbt: Highlight with
tree-sitter-dbt") do the same — with nothing to install in the editor.
