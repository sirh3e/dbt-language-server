# Highlighting from the server

cogitator colours every file of a dbt project with grammars that know dbt,
and sends the colours to your editor — nothing to install in the editor:

| File | Coloured by |
|---|---|
| models, macros, tests, snapshots, analyses (`.sql`) | tree-sitter-dbt for the Jinja, tree-sitter-sequel for the SQL between the tags |
| compiled SQL (`target/compiled`, `target/run`) | tree-sitter-sequel |
| YAML (`schema.yml`, sources, `dbt_project.yml`, `profiles.yml`, `selectors.yml`) | tree-sitter-yaml, the Jinja and SQL inside its values, and dbt's meaning |

The binding detail is
[`specs/highlighting/00-highlighting.md`](../specs/highlighting/00-highlighting.md).

- [What YAML gets](#what-yaml-gets)
- [Layers](#layers)
- [In your editor](#in-your-editor)
- [Theming the dbt captures](#theming-the-dbt-captures)
- [Macros and tests](#macros-and-tests)

## What YAML gets

```yaml
models:
  - name: customers                         # type.dbt.model
    description: "{{ doc('customers') }}"   # Jinja; customers → string.special.dbt.doc
    config:
      materialized: table                   # keyword.dbt.config
    columns:
      - name: customer_id                   # property.dbt.column
        data_tests:
          - unique                          # function.dbt.test
          - relationships:                  # function.dbt.test
              to: ref('stg_customers')      # to → variable.parameter.dbt, ref(…) → type.dbt.reference
              field: customer_id
          - dbt_utils.expression_is_true:
              expression: "amount > 0"      # SQL
              config:
                where: "{{ var('since') }} < created_at"   # Jinja and SQL
```

- **Jinja** inside any value (`{{ … }}`, `{% … %}`, `{# … #}`), in plain,
  quoted and block (`|`, `>`) scalars.
- **SQL** inside the values that hold SQL: expression tests' `expression`,
  `config.where`, `config.sql_header`, unit tests' `rows` with
  `format: sql`, semantic models' `expr`, metrics' `filter`, saved queries'
  `where`. Other values are never coloured as SQL, so prose stays prose.
- **dbt's meaning**: names of models, snapshots, seeds, analyses,
  exposures, metrics, semantic models, saved queries, macros, sources,
  source tables, columns and tests; test arguments; dbt config keys (from
  dbt's own JSON schemas); `ref()`, `source()` and `doc()` — with
  `.unresolved` when the project has no such model, source or doc block;
  `var()` as `variable.dbt.var`, `.unresolved` when nothing defines the
  var and no default is given ([variables.md](variables.md)).
- `dbt_project.yml`: its keys and `+` config keys, the profile name;
  `profiles.yml`: profile and target names; `selectors.yml`: selector names.

## Layers

Every span says which grammar produced it, its **layer**:

| Layer | From | Wins over |
|---|---|---|
| `yaml` | tree-sitter-yaml | — |
| `sql` | tree-sitter-sequel | `yaml` |
| `jinja` | tree-sitter-dbt | `sql`, `yaml` |
| `dbt` | the server's knowledge of the project | everything |

Editors use the layers to keep their own colours where they are better —
Neovim's and VS Code's YAML grammars colour YAML — and paint the other
layers over them.

## In your editor

| | Colour on open | Once | Options |
|---|---|---|---|
| Neovim | `highlight = { server = "auto" }` (default) | `:DbtHighlight` | `server = "auto" \| "always" \| false`, `layers = { yaml, sql, jinja, dbt }`; `:DbtHighlight! auto\|always\|off` for the session |
| Emacs | `cogitator-highlight-on-open` (default `t`) turns on `cogitator-highlight-mode` | `M-x cogitator-highlight-buffer` | `M-x cogitator-highlight-mode` toggles a buffer |
| VS Code | `cogitator.highlight.onOpen` (`auto`, `always`, `off`) | "dbt: Highlight with tree-sitter-dbt" | — |

- **`auto`** colours SQL buffers your editor's own tree-sitter does not
  (Neovim: without the tree-sitter-dbt parser installed), and every YAML
  file of the project with the `sql`, `jinja` and `dbt` layers over the
  editor's own YAML colours.
- **`always`** colours every buffer with every layer.
- Colours follow your edits: Neovim and VS Code repaint after a short pause,
  Emacs after an idle moment, and an answer for an older text is dropped.
  Long Neovim buffers get their visible window first.
- The explorer's SQL panes and the pickers' previews are always coloured
  (Neovim).

## Theming the dbt captures

The captures are Neovim's tree-sitter group names, dotted so a theme that
only knows the parent still colours them:

| Capture | Falls back to |
|---|---|
| `type.dbt.model` (`.snapshot`, `.seed`, `.analysis`, `.exposure`, `.metric`, `.semantic_model`, `.saved_query`, `.source_table`, `.reference`, `.profile`) | `type` |
| `module.dbt.source` | `module` |
| `property.dbt.column` | `property` |
| `function.dbt.test`, `function.dbt.macro`, `function.dbt.selector` | `function` |
| `variable.parameter.dbt` | `variable.parameter` |
| `variable.dbt.var` | `variable` |
| `string.special.dbt.doc` | `string.special` |
| `keyword.dbt.config` | `keyword` |
| `….unresolved` | the capture without it |

Neovim, for example:

```lua
vim.api.nvim_set_hl(0, "@type.dbt.model", { link = "Structure" })
vim.api.nvim_set_hl(0, "@property.dbt.column", { italic = true })
vim.api.nvim_set_hl(0, "@type.dbt.reference.unresolved", { undercurl = true, sp = "Red" })
```

Emacs maps captures to `font-lock-*` faces (`.unresolved` to
`font-lock-warning-face`); VS Code to theme colours (`.unresolved` to
`editorWarning.foreground`).

## In any editor: semantic tokens for YAML

Editors that colour only through the standard
`textDocument/semanticTokens` (Zed, Helix, any stock LSP client) get the
dbt-aware YAML colours too: on a `.yml`/`.yaml` file the server answers
the same captures as `dbt.highlight` (without its YAML base layer, which
the editor paints itself), mapped to standard token types:

| Capture | Token type | Modifiers |
|---|---|---|
| a model's, seed's, snapshot's, exposure's… `name`, `type.dbt.source_table` | `type` | `dbt`, `declaration` |
| `type.dbt.reference` (`ref()`/`source()` in YAML), `type.dbt.profile` | `type` | `dbt` |
| `module.dbt.source` | `namespace` | `dbt`, `declaration` |
| `property.dbt.column` | `property` | `dbt`, `declaration` |
| `function.dbt.macro` | `macro` | `dbt`, `declaration` |
| `function.dbt.test` | `function` | `dbt` |
| `variable.parameter.dbt` | `parameter` | `dbt` |
| `keyword.dbt.config` | `keyword` | `dbt` |
| `string.special.dbt.doc` | `string` | `dbt` |
| `….unresolved` | as above | + `unresolved` |

The Jinja and SQL inside values come along as `keyword`, `function`,
`variable`, `string`, `number` and `operator`. `/range` requests are
answered too, for SQL as well.

## Macros and tests

- **Neovim's explorer** (`:DbtBrowse`, then `.` for every kind) selects
  macros and tests as well as models:
  - a macro's card has its signature, package, callers and calls; its side
    panes show what it calls and who calls it; the dbt SQL pane starts at
    its definition and marks it; its compiled pane lists its callers
    (`<CR>` opens one);
  - a singular test shows what it refs, and its compiled SQL once dbt
    compiled it.
- **`:DbtNode [kind:]name`** (Neovim), **`M-x cogitator-node-info`** (Emacs)
  and **"dbt: Show node info"** (VS Code) show any node's card — a macro,
  a singular or generic test (where YAML applies it), a source, an
  exposure — through `dbt.nodeInfo` ([commands.md](commands.md#dbtnodeinfo)).
- **Previews** in Neovim's pickers colour a macro's file and a YAML test's
  file and mark the definition's lines.
