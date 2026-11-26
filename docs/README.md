# Documentation

| Document | For |
|---|---|
| [neovim.md](neovim.md) | Using cogitator in Neovim: installing with lazy.nvim, the model explorer, fuzzy finding, the DAG, options, troubleshooting |
| [incrementality.md](incrementality.md) | Which `table` models could be incremental, the rewrite cogitator offers, splitting a model at its frontier, reading the S-expressions |
| [variables.md](variables.md) | Project variables: where a value comes from (scopes, packages, overrides), the value inline after each `var()`, definition, references, the quick fix |
| [local-names.md](local-names.md) | Renaming, references and highlights of names local to a file: Jinja `set` and loop variables, macro parameters (with their keyword call sites), CTEs and table aliases; scoping and refusals |
| [highlighting.md](highlighting.md) | Colours from the server for models, macros, tests, compiled SQL and YAML property files: the layers, dbt-aware captures, options per editor, theming; macros and tests in the explorer |
| [ast-grep.md](ast-grep.md) | Structural search and rewrite with ast-grep over tree-sitter-dbt, your ast-grep rules as diagnostics, the ast-grep CLI for CI, and the server's highlighting |
| [search.md](search.md) | Structural search over the models' SQL (`dbt.search`): joins, `select *`, the relations and columns a model reads, as a small query language over the HIR |
| [column-rename.md](column-rename.md) | Renaming a model's output column across the project: what changes at every depth, where the walk stops, and when it is refused |
| [selectors.md](selectors.md) | dbt's node-selection language in cogitator: `dbt.select`, selectors.yml completion, diagnostics, hover and lenses, what is supported and what is not |
| [properties-yaml.md](properties-yaml.md) | Properties files (`schema.yml`…): schema-driven completion of keys, values, tests and columns, the unproduced-column warning, adding missing columns |
| [yaml-sync.md](yaml-sync.md) | Syncing schema YAML with the SQL project-wide (`dbt.syncYaml`, `cogitator yaml-sync`): missing entries and columns, descriptions inherited down the column lineage, stale columns, the CI check |
| [authoring-actions.md](authoring-actions.md) | Writing models faster: scaffold a staging model from a source or ref, extract a CTE with its documentation, inline a model as a CTE, inherit a column's description from upstream |
| [file-operations.md](file-operations.md) | Renaming, moving, deleting and creating model files from the editor's file tree, and declaration vs definition (YAML entry vs SQL) |
| [lint-and-format.md](lint-and-format.md) | Jinja and SQL syntax errors as you type (and when they decline), `cogitator lint` and `cogitator format` for CI: what runs, output formats, exit codes |
| [macros.md](macros.md) | Dispatched macros (go to implementation, which implementation the active adapter picks) and the macro call checks (argument counts, unknown and duplicate keywords, dispatches without a default) |
| [column-reach.md](column-reach.md) | Who reads a column: downstream columns, tests, semantic models, metrics, exposures and the dashboards of a BI manifest (`dbt.impact`, the column hover) |
| [column-completion.md](column-completion.md) | Column completion with each column's type and relation beside it and its description in the documentation panel (shared with the hover); details for models, sources, macros and functions |
| [snippets.md](snippets.md) | Model-aware snippets: an `is_incremental()` block on the model's real growing column, an incremental `config()` with its real merge key, clients without snippet support |
| [type-checks.md](type-checks.md) | Type checks under every profile (text compared with dates, sums over text, mismatched join keys), proven vs declared severities, the type inlay hints |
| [join-fanout.md](join-fanout.md) | `join.fanout`: joins that may duplicate rows, what proves a key unique (tests, constraints, combination tests), and the two quick fixes |
| [breaking-changes.md](breaking-changes.md) | The breaking-change preview: a model's columns against its last built schema, and what that breaks downstream |
| [preview.md](preview.md) | The rows of one CTE: the "Preview rows" lenses, which SQL runs, the row limits, cancelling, per editor |
| [lineage-site.md](lineage-site.md) | The column-lineage dashboard: `cogitator lineage-site` (a self-contained HTML page) and the live view in VS Code, what is traced, what is marked unknown, and why |
| [diff-at.md](diff-at.md) | Comparing a model with a git revision or the last dbt run: the template, the rendered SQL, or the column lineage (`dbt.diffAt`) |
| [cost.md](cost.md) | What a build costs: bytes and slots in the last-run lens, `dbt.costReport` with the critical path and "make it incremental", the optional query-history export |
| [profile.md](profile.md) | Profiling a column (off by default): the sampled query per adapter, DuckDB or `dbt show`, the profile in the hover |
| [commands.md](commands.md) | Every `dbt.*` workspace command: argument, result, side effects, and which editors expose it |
| [editor-parity.md](editor-parity.md) | Contributors: how a new command reaches every editor, and the test that holds it |
| [go-to-schema.md](go-to-schema.md) | `dbt.goToSchema` in detail, with a JSON-RPC example |

Elsewhere:

- `:h cogitator` — the Neovim plugin's own help; `editors/*/README.md` —
  installing each editor's integration.
- The top-level [README](../README.md) — the server's features and its
  configuration file, `cogitator.yml`.
- [`specs/`](../specs) — the binding specifications: the editor contract
  (`specs/neovim-plugin/00-plugin.md`), the IR pipeline
  (`specs/ir-pipeline/`) and highlighting (`specs/highlighting/`).
