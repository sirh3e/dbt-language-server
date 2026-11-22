# Documentation

| Document | For |
|---|---|
| [neovim.md](neovim.md) | Using cogitator in Neovim: installing with lazy.nvim, the model explorer, fuzzy finding, the DAG, options, troubleshooting |
| [incrementality.md](incrementality.md) | Which `table` models could be incremental, the rewrite cogitator offers, splitting a model at its frontier, reading the S-expressions |
| [variables.md](variables.md) | Project variables: where a value comes from (scopes, packages, overrides), the value inline after each `var()`, definition, references, the quick fix |
| [highlighting.md](highlighting.md) | Colours from the server for models, macros, tests, compiled SQL and YAML property files: the layers, dbt-aware captures, options per editor, theming; macros and tests in the explorer |
| [ast-grep.md](ast-grep.md) | Structural search and rewrite with ast-grep over tree-sitter-dbt, your ast-grep rules as diagnostics, the ast-grep CLI for CI, and the server's highlighting |
| [properties-yaml.md](properties-yaml.md) | Properties files (`schema.yml`…): schema-driven completion of keys, values, tests and columns, the unproduced-column warning, adding missing columns |
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
