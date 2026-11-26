# cogitator in Neovim

The first-party Neovim plugin (`editors/nvim`, Neovim 0.11+) wires the
cogitator language server into Neovim and adds what an editor-neutral LSP
cannot: a model explorer, fuzzy pickers, a project DAG, and `:Dbt…`
commands for every server feature. This guide is task-first; `:h cogitator`
has every option and key, and
[`specs/neovim-plugin/00-plugin.md`](../specs/neovim-plugin/00-plugin.md) is
the binding behaviour.

- [Install](#install)
- [Everyday use](#everyday-use)
- [The model explorer](#the-model-explorer)
- [Fuzzy finding](#fuzzy-finding)
- [Graphs, lineage and analysis](#graphs-lineage-and-analysis)
- [Nothing waits](#nothing-waits)
- [Options](#options)
- [Troubleshooting](#troubleshooting)

## Install

### 1. The server

```sh
curl -fsSL https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.sh | sh
```

It verifies the download against the release's `SHA256SUMS` and installs
`cogitator` to `~/.local/bin` (`COGITATOR_BIN_DIR` to change it,
`COGITATOR_VERSION` for a tag). On Windows, `install.ps1`. Or build it:
`cargo build --release -p cogitator`. The lazy.nvim spec below can do this
step for you.

### 2. The plugin with lazy.nvim

One line:

```lua
{ "sirh3e/dbt-language-server", opts = {} }
```

The plugin lives in `editors/nvim` of this repository; the repository's own
[`lazy.lua`](../lazy.lua) puts it on the runtimepath, makes `:h cogitator`
work and passes `opts` to `require("cogitator").setup()`.

A fuller spec ([`editors/nvim/examples/lazy.lua`](../editors/nvim/examples/lazy.lua)),
to copy to `~/.config/nvim/lua/plugins/cogitator.lua`:

```lua
return {
  "sirh3e/dbt-language-server",
  -- start when a SQL or YAML file is read, early enough for the server
  -- to attach to that first buffer
  event = { "BufReadPre *.sql", "BufReadPre *.yml", "BufReadPre *.yaml", "BufNewFile *.sql" },
  -- opt-in: install the server from the latest release on install and on
  -- every :Lazy update (drop it if you install the server yourself)
  build = vim.fn.has("win32") == 1 and "powershell -NoProfile -ExecutionPolicy Bypass -File install.ps1"
    or "sh install.sh",
  opts = {
    -- cmd = { vim.fn.expand("~/.local/bin/cogitator") }, -- when not on PATH
    -- profiles_dir = "~/.dbt",
    -- fusion = false,
  },
}
```

Load it on `BufReadPre`/`BufNewFile`, not `ft`: the server is enabled on the
`FileType` event, which `ft` loading would arrive after, so the first SQL
buffer would stay unattached.

**Try it first** without touching your config:

```sh
nvim -u editors/nvim/examples/minimal_init.lua path/to/models/some_model.sql
```

([`minimal_init.lua`](../editors/nvim/examples/minimal_init.lua) bootstraps
lazy.nvim and the plugin into a throwaway directory under your cache.)

### Other plugin managers

```vim
" vim-plug
Plug 'sirh3e/dbt-language-server', { 'rtp': 'editors/nvim' }
```
```lua
-- a plain checkout
vim.opt.rtp:append("~/src/dbt-language-server/editors/nvim")
require("cogitator").setup({})
```

### 3. Check it

`:checkhealth cogitator` reports the binary and its version, the project
root, which `profiles.yml` was found, the tree-sitter grammar, and the
server's own report (`dbt.doctor`). The server attaches to `sql` and `yaml`
buffers inside a dbt project (a `dbt_project.yml` upward).

## Everyday use

Hover, completion, go to definition (`ref()`, `source()`, macros, columns),
references (models, sources, macros, docs blocks, columns), rename (models,
file-local names — Jinja variables, macro parameters, CTEs and aliases, see
[local-names.md](local-names.md) — vars, columns — see
[column-rename.md](column-rename.md) — and the project's own macros), the macro call hierarchy (incoming and
outgoing), document links, signature help, diagnostics, formatting and
code actions are the standard LSP ones. On top:

| Key | Command | Does |
|---|---|---|
| `<leader>dr` / `dt` / `dc` | `:DbtRun` / `:DbtTest` / `:DbtCompile` | dbt on the current model (compile opens the compiled SQL) |
| | `:DbtPreview [n]` | `dbt show`: the model's first rows |
| | `:DbtRunSelection [sel]`, `:DbtCancel` | dbt with any selector; cancel a job |
| | `:DbtSelect [sel]` | dbt node selection in the navigator, without running dbt: `tag:x`, `+m`, `a,b`, `--exclude x` (`:DbtFind select` too) |
| `<leader>ds` / `dq` | `:DbtSchema` / `:DbtTests` | the model's YAML entry; one of its tests |
| `<leader>dy` | `:DbtExtract` | scaffold the model's YAML, or add a version |
| `<leader>dR` / `dx` | `:DbtRelation` / `:DbtQuery` | yank `db.schema.table`; a bounded `select *` on it |
| `<leader>dL` | | run the code lens on this line (run / test / compile / tests) |
| `<leader>dv` / `dV` | | expand / shrink the selection along the SQL's structure |
| `<leader>dT` / `dP` | `:DbtTarget` / `:DbtProjects` | switch profile target; jump between projects |
| `<leader>dS` | `:DbtStatus` | what the server knows and has been doing |

`:Dbt <subcommand>` reaches every command with completion; `:Dbt` alone lists
them. `keymaps = false` in `setup()` turns the `<leader>d` maps off.

## The model explorer

`:DbtBrowse` (`<leader>db`) shows one model at a time — the one in your
buffer, or one you pick:

```
┌ upstream ⯇ (n) ┐┌ model · a › b › c ──┐┌ ⯈ downstream (n) ┐
│ direct parents ││ the model's card    ││ direct children  │
└────────────────┘└─────────────────────┘└──────────────────┘
┌ dbt SQL · x.sql ───────────────┐┌ compiled SQL ───────────────────┐
│ the model's file (Jinja)       ││ what dbt compiled it to         │
└────────────────────────────────┘└─────────────────────────────────┘
```

- **The card** shows the model's kind and file, its description, tags, the
  relation it builds, its versions, documented columns and tests. `<CR>` on
  a test opens its YAML, on a version its file, elsewhere the model.
- **Walk the lineage**: `h`/`l` enter the upstream/downstream pane; pressed
  again there they make the entry the model and keep going (`3h` walks three
  levels); `<CR>` on an entry makes it the model. `[` (or `-`) goes back,
  `]` forward; the card's title is the trail (`a › b › c`).
- **Jump anywhere**: `f` or `/` opens the models picker (fuzzy).
- **The SQL panes**: `p` enters the dbt SQL, `h`/`l` move between it and the
  compiled SQL, `<Esc>` returns; `J`/`K` scroll both together from any pane.
  Both are coloured by tree-sitter-dbt through the server — the Jinja and
  the SQL between the tags — in your colourscheme's tree-sitter colours,
  with nothing to install. A model dbt has not compiled says so — press `c`
  to compile.
- **Several models at once**: `<Space>` marks the model or a side entry; `y`
  yanks the marked names, `r` runs them in one `dbt run`.
- **Macros and tests**: `.` lets the explorer take every kind of node. A
  macro shows its signature and callers, starts its dbt SQL pane at its
  definition, and lists its callers in the compiled pane (`<CR>` opens
  one); a singular test shows what it refs and its compiled SQL. See
  [highlighting.md](highlighting.md#macros-and-tests).

| Key | Does |
|---|---|
| `o` `s` `v` `t` | open the model / in a split / vsplit / tab |
| `c` / `P` | dbt compile it / dbt parse the project |
| `g` / `L` | show it in `:DbtDag` / its column-lineage graph |
| `.` | let the picker and walks take sources, exposures… too |
| `?` / `q` | list the keys / close |

`:DbtGraph` (`<leader>dg`) is a lighter three-pane view of just the lineage.

## Fuzzy finding

`:DbtFind` shows a list in **your** picker — fzf-lua, snacks.picker or
telescope, whichever is installed (in that order) — or in a built-in
fzf-style one: a prompt, the ranked list with the matched letters
highlighted, and a preview. Choose with `setup({ picker = "fzf-lua" |
"snacks" | "telescope" | "builtin" | "auto" })`.

| Key | Find | Try typing |
|---|---|---|
| `<leader>df` | models | `stg cust` |
| `<leader>dd` | definitions: models, sources, macros, selectors | `times_five` |
| `<leader>dk` | documented columns, as `model.column` | `^customers.customer` |
| `<leader>du` / `dU` | everything upstream / downstream of this model, nearest first | `↑1` |
| `<leader>da` | ast-grep matches of a structural pattern (asked first) — see [ast-grep.md](ast-grep.md) | `ref` |
| `<leader>dF` | choose what to find: also `sources`, `all`, `macros`, `lineage`, `refs` (this file's `ref()`s, CTEs, macros), `tests`, `tags`, `selectors`, `astgrep` | |

The query is fzf's: `stg cust` (both, fuzzy), `'exact`, `^prefix`,
`suffix$`, `!not`, `a | b`.

| Key | Does |
|---|---|
| `<CR>` | open (for a definition: jump to it; for a column: the model on that column) |
| `<C-v>` `<C-x>` `<C-t>` | vsplit / split / tab |
| `<C-g>` | show it in the explorer (for a column: its lineage graph) |
| `<C-y>` / `<C-r>` | yank the names / dbt run them |
| `<Tab>` | mark several |

Previews of `.sql` and YAML files are coloured by the server like the
explorer's panes (in the built-in picker, fzf-lua and telescope), and a
macro's or test's preview marks its definition.

Every other list the plugin shows (`:Dbt`, `:DbtTests`, `:DbtTags`,
`:DbtCancel`, targets, projects) uses the same picker.

## Where things are used, and renaming them

- `:DbtReferences` (`<leader>dw`, `grr`) — everywhere the model, macro,
  source or column under the cursor is used, in a yazi-style dialog: a
  group per file, a line per use, a preview of the lines around it.
  `:DbtReferences!` lists the uses of the file's own model wherever the
  cursor is. `/` filters the files and lines with fzf's syntax.
- `:DbtMacros` (`<leader>dm`) — every macro; choose one to see where it is
  called.
- `:DbtRename [name]` (`<leader>de`, `grn`) — rename the model or macro
  under the cursor (else the file's model; `!` always it) across the
  project. The new name is asked for, then the whole edit is shown —
  every file's diff and the file rename — and applied only when you
  choose "apply". A package's macro is refused with the reason.

The first entry of a references dialog renames what it lists. With
`ui.picker = false`, `grr` and `grn` keep Neovim's defaults.

## Graphs, lineage and analysis

- `:DbtDag` (`<leader>dD`) — the whole project as a layered DAG in the dbt
  docs colours; `h`/`l` along edges, `j`/`k` within a layer, `/` search,
  `f` focus on a neighbourhood, `c` expand columns, `L` column lineage.
- `:DbtColumnLineage [column]` — a column's lineage across models.
- `:DbtLineage` (`<leader>dl`), `:DbtImpact [column]` — lineage as a
  message; what breaks downstream if this model or column changes.
- `:DbtPlan` — the model's query plan, the optimiser's findings, and its
  incrementality analysis.
- `:DbtIncrementality`, `:DbtSplit` — which table models could be
  incremental; split one at its frontier. See
  [incrementality.md](incrementality.md).
- `:DbtIncrementalDiff` — an incremental model rendered with
  `is_incremental()` false and true, as a diff in a split (also the
  "Compare incremental / full refresh" code lens).
- `:DbtAstGrep {{ ref($M) }}` — structural search with ast-grep over the
  project into quickfix; `pattern => rewrite` rewrites after asking;
  `sql:` searches the compiled SQL. Your `sgconfig.yml` rules are
  diagnostics with quickfixes. See [ast-grep.md](ast-grep.md).
- `:DbtHighlight` — colour any buffer from the server; `:DbtHighlight!
  auto|always|off` sets when buffers are coloured on open. dbt files and
  YAML property files are coloured on open by default
  ([highlighting.md](highlighting.md)).
- `:DbtNode [kind:]name` — any node's card: a macro, a test, a source…
- `:DbtDeadModels`, `:DbtExpandJinja`, `:DbtSyntaxTree`, `:DbtDoctor`.

## Nothing waits

The plugin never blocks the editor on the server: every request is
asynchronous. Where Neovim needs an answer at once, it answers from what was
fetched in the background — column-name completion for `:Dbt impact` and
`:DbtColumnLineage` (fetched when a buffer is entered, after insert mode and
on save) and `:checkhealth` (the server's report, fetched at attach and after
each refresh). `<leader>dv` selects when the server answers, and not if you
moved on. A large `:DbtDag` colours what is on screen first and the rest in
small slices while you keep working.

## Options

`require("cogitator").setup({ … })` (or lazy.nvim's `opts`), with the
defaults:

```lua
{
  cmd = { "cogitator" },          -- the server; flags are appended
  profiles_dir = nil,             -- where profiles.yml lives
  manager_path = nil,             -- where the per-project store lives
  query_limit = 100,              -- :DbtQuery row cap
  fusion = false,                 -- true, or "/path/to/dbt", for dbt Fusion
  tree_sitter = true,
  debug = false,
  filetypes = { "sql", "yaml" },
  root_markers = { "dbt_project.yml" },
  attach_outside_project = false,
  lens = { enabled = true, events = { "BufEnter", "CursorHold", "InsertLeave" } },
  keymaps = true,
  semantic_tokens = true,
  inlay_hints = true,  -- column lineage, relations after ref(), var() values, macro params
  lineage = { depth = 3, max_requests = 60 },
  dag = { focus_depth = 2, follow = false, refresh_on_save = true },
  grammar = { highlight = true, auto_install = false },
  picker = "auto",                -- "fzf-lua" | "snacks" | "telescope" | "builtin"
  navigator = { show_all_kinds = false },
  -- colours from the server: "auto" | "always" | false, per layer
  highlight = { server = "auto", layers = { yaml = true, sql = true, jinja = true, dbt = true } },
}
```

An unknown key is an error at startup, so a typo does not go unnoticed.
Project settings for the server itself (lint rules, profiles, limits) go in
`cogitator.yml` at the project root — see the top-level README.

## Troubleshooting

- **Nothing attaches** — `:checkhealth cogitator`: is the binary found, is
  there a `dbt_project.yml` upward of the file? With lazy.nvim, load on
  `BufReadPre`, not `ft`.
- **"cogitator is not attached to this buffer"** — the command needs a SQL or
  YAML buffer of the project.
- **The compiled SQL pane is empty** — dbt has not compiled the model; `c`
  in the explorer or `:DbtCompile`.
- **`:h cogitator` is missing after a manual install** — `:helptags
  path/to/editors/nvim/doc`.
- **What is the server doing?** — `:DbtStatus`, and `:LspLog` (set
  `log.client_level: debug` in `cogitator.yml` for per-request timing).
