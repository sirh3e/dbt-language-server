# Documentation

| Document | For |
|---|---|
| [neovim.md](neovim.md) | Using cogitator in Neovim: installing with lazy.nvim, the model explorer, fuzzy finding, the DAG, options, troubleshooting |
| [incrementality.md](incrementality.md) | Which `table` models could be incremental, the rewrite cogitator offers, splitting a model at its frontier, reading the S-expressions |
| [commands.md](commands.md) | Every `dbt.*` workspace command: argument, result, side effects, and which editors expose it |
| [editor-parity.md](editor-parity.md) | Contributors: how a new command reaches every editor, and the test that holds it |
| [go-to-schema.md](go-to-schema.md) | `dbt.goToSchema` in detail, with a JSON-RPC example |

Elsewhere:

- `:h cogitator` — the Neovim plugin's own help; `editors/*/README.md` —
  installing each editor's integration.
- The top-level [README](../README.md) — the server's features and its
  configuration file, `cogitator.yml`.
- [`specs/`](../specs) — the binding specifications: the editor contract
  (`specs/neovim-plugin/00-plugin.md`) and the IR pipeline
  (`specs/ir-pipeline/`).
