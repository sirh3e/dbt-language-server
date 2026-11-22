# 00 — Highlighting from the server: macros, tests and YAML property files

How the cogitator server colours dbt code with tree-sitter grammars, and how
the editors show it. Binding for `dbt.highlight` (v2), `dbt.nodeInfo`, the
Neovim plugin's highlight-on-open (`cogitator.tshl`), the explorer's macro
and test views, the picker previews, and their Emacs and VS Code
counterparts. Extends what #46 introduced; tracked by the GitHub epic
"tree-sitter highlighting for macros, tests and YAML property files".

## 0. Goals and non-goals

**Goals**

1. Every file a dbt project is made of is coloured by grammars that know
   dbt: models, macros, singular tests, snapshots and analyses (Jinja over
   SQL), dbt's compiled SQL, and the YAML property files — including the
   Jinja and the SQL that live inside YAML values.
2. The YAML is coloured by what it *means* to dbt as well as by YAML
   syntax: a model's name, a column, a test, a doc block, a source and a
   reference look different from a plain key.
3. Macros and tests are first-class in the Neovim explorer: selectable,
   with a card, their source and (for tests) compiled SQL, and their
   neighbours.
4. Nothing to install in the editor: the colours come from the server,
   which links the grammars.
5. Every editor gets it: Neovim, Emacs and VS Code paint the same answer;
   Zed keeps its own grammars (it has no hook for server-side painting
   beyond semantic tokens, §12).

**Non-goals**

- No new grammar. tree-sitter-dbt (pinned in `Cargo.toml`),
  tree-sitter-sequel and tree-sitter-yaml are used as published; where a
  grammar falls short (SQL is text to tree-sitter-dbt, YAML has no
  injections query), the server composes them.
- A Neovim parser that is installed and running keeps priority for the
  languages it covers (§7): the server's colours fill what Neovim cannot.
- No change to editing: highlighting is decoration, never text.
- Not a replacement for `textDocument/semanticTokens`, which stays the
  SQL token stream it is today (§12 discusses YAML semantic tokens).

**Glossary**

- *Span*: a range with one capture name.
- *Capture*: the name a highlights query gives a node (`keyword.conditional`).
  Editors map captures to faces; Neovim's are the `@<capture>` groups, which
  fall back along the dots by themselves (`@type.dbt.model` → `@type.dbt` →
  `@type`).
- *Layer*: which grammar produced a span — `jinja`, `sql`, `yaml` or `dbt`
  (the server's own dbt-aware captures, §5).
- *Injection*: highlighting a region of one document with another grammar.

## 1. Current state (#46)

`dbt.highlight` takes `{uri}`, `{text, language?: "dbt"|"sql"}` or
`{model?, compiled: true}` and answers
`{language, spans: [{range, capture}], truncated}`, ranges in UTF-16:

- `dbt` runs tree-sitter-dbt's `highlights.scm`; its `injections.scm` hands
  the text between the tags (one combined document) to tree-sitter-sequel's
  `highlights.scm`.
- `sql` runs tree-sitter-sequel's alone.
- `@spell`/`@nospell` captures are removed from the queries before use
  (tree-sitter-highlight lets the last capture on a node win).
- At most 50 000 spans; texts over 2 MiB answer no spans with
  `truncated: true`.

Today's captures (the union of the two queries, after the spell captures):

| Grammar | Captures |
|---|---|
| tree-sitter-dbt | `boolean comment constant.builtin function function.builtin function.call function.method.call keyword keyword.conditional keyword.function keyword.import keyword.operator keyword.repeat label markup.raw number number.float operator punctuation.bracket punctuation.delimiter string string.escape tag.delimiter variable variable.builtin variable.member variable.parameter` |
| tree-sitter-sequel | `attribute boolean comment conditional field float function.call keyword keyword.operator number operator parameter punctuation.bracket punctuation.delimiter storageclass string type type.builtin type.qualifier variable` |

Painted today: the explorer's dbt SQL and compiled SQL panes, `.sql`
picker previews (built-in, fzf-lua, telescope), Neovim `:DbtHighlight`,
Emacs `M-x cogitator-highlight-buffer`, VS Code "dbt: Highlight with
tree-sitter-dbt". Not covered: YAML; macros and tests outside a preview;
highlighting without an explicit command.

## 2. `dbt.highlight` v2: the wire contract

All additions are **append-only**: a v1 client sending a v1 argument gets a
v1-compatible answer (the new answer fields are extra keys).

**Argument** — one object, one of:

| Form | Meaning |
|---|---|
| `{uri, language?, range?, version?}` | the open document's text (else the file on disk) |
| `{text, language?, range?}` | any text (a copy, a pane) |
| `{model?, compiled: true, range?}` | the SQL dbt compiled a model to (or the `uri`'s model) |
| `{node: {kind, name}, compiled?: bool, range?}` | **new**: a graph node's source file, or its compiled SQL (§6) |

- `language`: `"dbt"`, `"sql"` or **`"yaml"`** (new). When absent:
  - from the uri or node file: `.yml`/`.yaml` → `yaml`; a path under
    `target/compiled/` or `target/run/` → `sql`; any other `.sql` → `dbt`;
  - `{text}` without a language stays `dbt` (v1 behaviour);
  - `compiled: true` → `sql`.
  Unknown names stay `InvalidParams` with Guidance (v1).
- `range`: `{start: {line}, end: {line}}`, 0-based, end exclusive. The server
  highlights the **whole** document (injections need it) but answers only
  the spans that intersect the range. For the visible-window-first paint
  (§7). Absent: every span.
- `version`: an integer the client chooses (Neovim: `changedtick`; Emacs:
  `buffer-chars-modified-tick`; VS Code: `document.version`). Echoed in the
  answer; used as part of the cache key (§9).

**Answer**

```jsonc
{
  "language": "yaml",
  "version": 42,                 // echoed, or absent
  "spans": [
    {"range": {...}, "capture": "type.dbt.model", "layer": "dbt"}
  ],
  "truncated": false,
  "layers": ["yaml", "jinja", "sql", "dbt"]   // which layers ran
}
```

- `layer` is new on each span: `jinja` (tree-sitter-dbt), `sql`
  (tree-sitter-sequel), `yaml` (tree-sitter-yaml), `dbt` (§5). Clients may
  ignore it; Neovim uses it for one namespace per layer, so a layer can be
  turned off (§7).
- Spans are sorted by start, never overlap, and the innermost wins
  (§3.3).
- `null` stays the answer for `compiled: true` of a model dbt has not
  compiled, and **also** for a `node` whose kind has no compiled SQL (a
  macro).

## 3. YAML highlighting

### 3.1 Base layer: tree-sitter-yaml

- Dependency: `tree-sitter-yaml = "0.7"` (it depends on
  `tree-sitter-language ^0.1` only, so it runs on the workspace's
  tree-sitter 0.27). It exports `LANGUAGE` and `HIGHLIGHTS_QUERY`; it has
  **no** injections query.
- A third `HighlightConfiguration` in `cogitator-treesitter::highlight`
  (`yaml`), with the spell captures removed as for the others and the union
  of names extended. Its captures:
  `attribute boolean comment constant.builtin label number property punctuation.bracket punctuation.delimiter punctuation.special string type`.
- Keys are `property`, values `string`/`number`/`boolean`/`constant.builtin`
  (null), anchors and aliases `label`, tags `type`.

### 3.2 Jinja inside YAML values (layer `jinja`)

dbt renders Jinja in YAML property values: `{{ doc('orders') }}` in a
description, `{{ var('x') }}` / `{{ env_var('X') }}` in configs, `{% if %}`
rarely. tree-sitter-yaml sees one scalar.

Rule: after the base layer, walk the YAML tree; for every scalar node
(`plain_scalar`, `double_quote_scalar`, `single_quote_scalar`,
`block_scalar`) whose text contains `{{`, `{%` or `{#`:

1. Take the scalar's **content**:
   - plain: the node text;
   - quoted: the text between the quotes. Escapes are left as they are (a
     `\"` inside Jinja is rare; a mis-coloured escape is acceptable and
     documented);
   - block (`|`, `>`, with chomping/indent indicators): the lines after the
     header, each with the block's indentation removed; a line map keeps
     each content line's offset in the document.
2. Highlight the content with the `dbt` configuration — which injects the
   SQL between the tags as usual; for a description that is prose, the
   "SQL" between tags is prose too and tree-sitter-sequel colours little of
   it, which is fine. To avoid colouring prose as SQL, **the YAML injection
   drops spans of layer `sql`** unless the key is one of §4's SQL keys.
3. Map each span back through the offsets (UTF-16 columns recomputed on the
   document's lines) and give it layer `jinja`.
4. The scalar's own `string` span is split around the Jinja spans (§3.3),
   so text outside the tags stays a string.

### 3.3 Precedence and merging

Spans of all layers are merged into one list with **innermost wins**: a
`jinja` span inside a `yaml` `string` span replaces that part of it; a
`dbt` span (§5) replaces the `yaml` span of the same node; an `sql` span
(§4) inside a scalar replaces the `string` span. Implemented as an
interval paint: lay spans down by increasing priority
(`yaml` < `sql` < `jinja` < `dbt`), each overwriting the stretch it covers,
then emit maximal runs of one capture. Ties of equal priority keep the
earlier span. The 50 000-span cap applies after merging.

## 4. SQL inside YAML (layer `sql`)

Some YAML values are SQL. The server colours them with tree-sitter-sequel,
and Jinja inside them with tree-sitter-dbt (§3.2 applies inside).

**Which values** — matched on the path of keys from the document root
(`*` any key or index; `…` any depth):

| Path | Where it comes from | Kind |
|---|---|---|
| `…/data_tests/*/expression_is_true/expression`, `…/tests/*/*/expression` | dbt-utils expression tests | expression |
| `…/config/where`, `…/data_tests/*/*/config/where` | test configs | expression (a predicate) |
| `…/config/sql_header`, `models/*/config/sql_header` | model config | statement(s) |
| `unit_tests/*/given/*/rows`, `unit_tests/*/expect/rows` **when the sibling `format` is `sql`** | unit-test fixtures | statement |
| `semantic_models/*/{dimensions,measures,entities}/*/expr` | semantic layer | expression |
| `metrics/*/filter`, `metrics/*/type_params/*/filter` | metric filters | expression (often Jinja `{{ Dimension(...) }}`) |
| `saved_queries/*/query_params/where/*` | saved queries | expression |

**Statements** are highlighted as they are. **Expressions** are wrapped:
the server highlights `select <expr>` and shifts every span back by the
length of `select ` on the first line (spans inside the prefix are
dropped). A list-valued key (`where: [a, b]`) takes each item on its own.

Unknown keys are never treated as SQL: false positives would colour prose.
The table lives in one place (`cogitator-treesitter::highlight::yaml_sql`),
with a test per row on fixture YAML.

## 5. dbt-aware captures (layer `dbt`)

From the marked YAML the server already parses (`cogitator-schemas::yaml`,
`properties.rs`), the names that *mean* something in dbt get captures of
their own — dotted names under standard captures, so an editor that only
knows the parent still colours them.

| Node | Capture | Parent an editor falls back to |
|---|---|---|
| a model's `name` (models, snapshots, seeds, analyses entries) | `type.dbt.model` (`.snapshot`, `.seed`, `.analysis`) | `type` |
| a source's `name` | `module.dbt.source` | `module` |
| a source table's `name` | `type.dbt.source_table` | `type` |
| a column's `name` | `property.dbt.column` | `property` |
| a test's name under `data_tests`/`tests` (`unique`, `relationships`, `dbt_utils.expression_is_true`) | `function.dbt.test` | `function` |
| a test argument key (`to`, `field`, `values`, `expression`) | `variable.parameter.dbt` | `variable.parameter` |
| a macro's `name` in `macros:` | `function.dbt.macro` | `function` |
| an exposure / metric / semantic model / saved query `name` | `type.dbt.exposure` (`.metric`, `.semantic_model`, `.saved_query`) | `type` |
| a `ref('x')` / `source('a','b')` value in YAML (e.g. `relationships.to`, `depends_on`) | `type.dbt.reference` | `type` |
| `doc('x')` inside a description | `string.special.dbt.doc` | `string.special` |
| `var('x')` inside a value (its name) | `variable.dbt.var` (resolved by the file's var scope; a default argument counts as resolved) | `variable` |
| a dbt config key (`materialized`, `tags`, `enabled`, `contract`, `access`, …) | `keyword.dbt.config` | `keyword` |
| a reference that does not resolve in the project | the capture above + `.unresolved` (e.g. `type.dbt.reference.unresolved`) | the capture above |

- References are resolved with the context's maps (`model_detail_map`,
  `source_detail_map`, `doc_detail_map`, macros); `.unresolved` adds no
  diagnostic — the existing dangling-ref diagnostics stay the place for that.
- Config keys: the list of dbt's config keys comes from the dbt JSON schemas
  the server already embeds (`schemas/dbt/`), not a hand-made list.
- `dbt` spans have the highest priority (§3.3).
- Files that are not dbt property files (`dbt_project.yml`, `profiles.yml`,
  `packages.yml`, `selectors.yml`) get the base and Jinja layers, plus their
  own small capture sets: `dbt_project.yml` paths (`model-paths` …) as
  `keyword.dbt.config`, profile/target names as `type.dbt.profile`,
  selector names as `function.dbt.selector`.

## 6. Macros and tests in the explorer

### 6.1 `dbt.nodeInfo` (new command)

`{node: {kind, name}}` → a card's data for any graph node, so the explorer
(and the other editors) can show macros and tests as they show models.
`dbt.modelInfo` stays as it is (the explorer keeps using it for models).

| Kind | Answer |
|---|---|
| `model` | `dbt.modelInfo`'s answer, plus `kind` |
| `macro` | `{kind, name, package, description, params: [{name, default?}], uri, range, callers: [{kind, name, uri}], calls: [{name, uri}], usedByTests: [...]}` — `callers` from incoming `MacroCall` edges, `calls` from the macro's own calls |
| `test` (singular, `tests/…/*.sql`) | `{kind, name, uri, refs: [{kind, name, uri}], compiledUri?, description?}` — refs from the test's outgoing edges; `compiledUri` = `target/compiled/<project>/tests/…` when it exists |
| `generic_test` (a `{% test x(...) %}` definition) | `{kind, name, package, params, uri, range, appliedTo: [{model, column?, uri, range}]}` — where YAML applies it (from the properties' column/model tests) |
| `source`, `exposure`, `metric`, `semantic_model`, `analysis` | `{kind, name, uri, range?, description?, refs, upstream, downstream}` (the graph's neighbours) |

- Generic test definitions are indexed as their own kind: a `{% test name %}`
  block defines the test `name` (dbt's macro `test_name`); if the context
  does not yet index `{% test %}` blocks separately from macros, this
  command's issue adds it.
- `null` with Guidance for an unknown node, with the closest names
  suggested (as `compiledSql` does).
- A user command (§10): every editor wraps it.

### 6.2 The explorer

- `.` (or `navigator.show_all_kinds`) already lets non-model nodes be
  selected; macros and tests become **selectable centres**: `f`/`/` list
  them in the picker (`all` source), walking reaches them (a test is
  downstream of the models it refs; a macro's callers are listed beside it).
- **Card** per kind (`explorer/card.lua`): a kind line, then
  - macro: signature `name(a, b='x')`, package, description, "called by"
    count and the first callers, "calls";
  - singular test: what it refs, whether it is compiled;
  - generic test: signature, where it is applied (model.column lines that
    jump, like the card's test lines today).
- **Side panes**: macro → left: macros it calls; right: its callers.
  Test → left: the models it refs; right: — (a test has no downstream).
- **Bottom panes**:
  - dbt SQL: the node's file, **scrolled to and highlighting the
    definition's range** (a macro in a file of many: the `{% macro %}`
    block's lines are marked with a sign, the view starts there);
  - compiled: a test → its compiled SQL (coloured `sql`); a macro → "a macro
    compiles into the models that call it" and the callers, each `<CR>`
    opening that model in the explorer.
- Fetching: `dbt.nodeInfo` replaces `dbt.modelInfo` for non-models; the
  compiled pane uses `dbt.highlight {node, compiled: true}` for tests.

## 7. Highlight on open

### 7.1 Neovim

Option (new, in `config.lua`):

```lua
highlight = {
  server = "auto",   -- "auto" | "always" | false
  layers = { yaml = true, jinja = true, sql = true, dbt = true },
}
```

- `"auto"`: on attach, paint
  - a `sql` buffer **when Neovim's tree-sitter-dbt parser is not running on
    it** (`cogitator.treesitter.start` returned false or the grammar is
    disabled);
  - **every** YAML buffer of the project (Neovim's yaml parser knows neither
    Jinja nor dbt; the server's `yaml` layer is then skipped when Neovim's
    yaml highlighter is active, and only `jinja`/`sql`/`dbt` are painted on
    top of it).
- `"always"`: paint every attached buffer with every layer; regex syntax
  off.
- `false`: only on `:DbtHighlight`.
- **Painting**: one extmark namespace per layer
  (`cogitator.tshl.<layer>`), priority `yaml` 120 < `sql` 124 < `jinja`
  126 < `dbt` 128 (above semantic tokens' 125 where the server's capture is
  more precise; `yaml` below, so Neovim's own yaml colours can show through
  if it wins). `layers` turns a layer off.
- **Repaint**: on `TextChanged`/`InsertLeave`, debounced (150 ms), with
  `version = changedtick`; an answer for an older tick is dropped. For a
  buffer over 2 000 lines, first `range` = the visible window ± 100 lines,
  then the rest.
- `:DbtHighlight` keeps toggling the current buffer; `:DbtHighlight!` sets
  the mode for the session.
- `:checkhealth cogitator` reports the mode and which buffers are painted
  by the server vs by Neovim's parsers.

### 7.2 Emacs

`cogitator-highlight-mode`: a buffer-local minor mode that paints (as
`cogitator-highlight-buffer` does) and repaints on `after-change-functions`
(idle timer, 0.3 s), with `version` = `buffer-chars-modified-tick`.
`cogitator-highlight-on-open` (defcustom, default `t`) turns it on in the
project's `sql-mode` and `yaml-mode`/`yaml-ts-mode` buffers when cogitator
manages them. Faces by capture as today, extended for §5's captures
(`type.dbt.*` → `font-lock-type-face`, `property.dbt.column` →
`font-lock-property-name-face`, `function.dbt.test` →
`font-lock-function-call-face`, `.unresolved` → `font-lock-warning-face`).

### 7.3 VS Code

Setting `cogitator.highlight.onOpen` (`"auto"` default, `"always"`,
`"off"`): decorations for dbt SQL and YAML documents on open and after edits
(debounced, by `document.version`). VS Code's own TextMate grammars stay;
the decorations are drawn over them (`auto` skips the `yaml` layer).

## 8. Picker previews of macros and tests

- `macros`, `definitions` and `tests` sources preview the **definition**:
  the file, scrolled to the macro's / test's line, coloured by the server
  (`code` = the file's language, §2's rule), the definition's lines marked.
- Generic tests applied in YAML (the `tests` source, `dbt.modelTests`
  entries): the YAML file, coloured as `yaml` with all layers, landing on
  the test's entry.
- Previews ask for the visible range first (§2 `range`) when the file is
  over 2 000 lines.
- All backends that expose the preview buffer (built-in, fzf-lua,
  telescope) paint it; snacks keeps its own `ft` highlighting.

## 9. Performance and limits

- Grammars and query configurations are built once (`OnceLock`).
- **Cache**: `DerivedCache` keeps the last answer per `(uri, version,
  language)` for open documents and per `(path, mtime, size, language)` for
  files on disk; a `range` request is served from the cached full answer.
  At most 64 entries, least recently used evicted.
- Limits stay: 2 MiB of text, 50 000 spans after merging; past them the
  answer is `truncated: true` with the spans so far (in `range` order when a
  range is given).
- Budget (debug build, measured in tests as a guard, not a hard SLA): a
  1 000-line model ≤ 20 ms, a 1 000-line schema YAML with Jinja ≤ 40 ms.
- Requests are offloaded (`offload_blocking`) as today; a client's newer
  `version` makes older answers useless but the server does not cancel —
  clients drop them.

## 10. Editor parity

- `dbt.highlight` stays a **user** command (already wrapped in all three
  editors); v2 changes no wrapper's contract.
- `dbt.nodeInfo` is a new **user** command: Neovim (the explorer, and
  `:DbtNode [kind:]name` showing the card in a float), Emacs
  (`M-x cogitator-node-info`, a help buffer), VS Code ("dbt: Show node
  info", a quick pick of nodes, then the card as markdown). Classified in
  `editors/parity.toml`, a section in `docs/commands.md`, a row in
  `specs/neovim-plugin/00-plugin.md` — the parity test enforces all three.

## 11. Testing

- **Unit** (`cogitator-treesitter`): insta snapshots of `(text, capture,
  layer)` for fixture files: a macro file with two macros and a `{% test %}`
  block; a singular test; a schema YAML with models, columns, tests,
  descriptions with `doc()`, a source with tables, a unit test with
  `format: sql`, a semantic model with `expr`; `dbt_project.yml`. Cases for
  every §4 row and every §5 capture, the expression wrapping offsets,
  block-scalar indentation mapping, quoted scalars, UTF-16 columns, the
  merge rules (§3.3), truncation and `range` filtering.
- **e2e** (`crates/cogitator/tests/e2e_highlight.rs`): v1 arguments unchanged;
  `language` inferred from the uri; `range`; `version` echoed and cached;
  `node` forms; `dbt.nodeInfo` for each kind on the jaffle fixture (macro
  callers, the singular test's refs, a generic test's applications).
- **Neovim**: highlight-on-open paints a YAML buffer with `@type.dbt.model`
  and `@property.dbt.column`; an older tick's answer is dropped; layers off;
  the explorer selects the fixture's macro and test, card lines, panes;
  previews land on the definition.
- **Emacs / VS Code**: the mode/setting paints a YAML buffer; the node-info
  wrappers answer for a macro.
- **Perf guard**: the §9 budget asserted with a generous factor in a test
  marked to run in CI's release job only.

## 12. Rollout and open questions

Order (each a sub-issue of the epic; later ones depend on earlier ones):

1. `dbt.highlight` v2 contract, inference, `range`, `version`, cache (§2, §9).
2. YAML base layer (§3.1, §3.3).
3. Jinja inside YAML (§3.2).
4. SQL inside YAML (§4).
5. dbt-aware captures (§5).
6. `dbt.nodeInfo` (§6.1).
7. Neovim explorer for macros and tests (§6.2) — after 6.
8. Neovim highlight on open (§7.1) — after 1–2.
9. Neovim picker previews of macros and tests (§8) — after 1.
10. Emacs and VS Code: highlight on open, node-info wrappers, docs (§7.2,
    §7.3, §10).

Open questions:

- **Semantic tokens for YAML**: Zed (and any stock client) could get the §5
  captures through `textDocument/semanticTokens` if the legend grew
  (`type`, `property`, `function`, `modifier: dbt`). Deferred: it changes a
  capability every client sees.
- **Neovim's own injections**: a `queries/yaml/injections.scm` shipped by
  the plugin could hand YAML scalars with Jinja to the installed
  tree-sitter-dbt parser — no server round-trip, but no §4/§5 layers.
  Possibly complementary in `auto` mode later.
- **Quoted-scalar escapes** inside Jinja (§3.2) are not unescaped; revisit
  if a real project shows mis-colouring.

## 13. Implementation notes

Where the implementation (epic #58) settles what the text above leaves
open:

- **Layers from one highlighter.** Each grammar's configuration is set up
  with the capture names behind a run of placeholders that match nothing,
  so a tree-sitter-highlight event's index says which grammar produced it,
  also inside tree-sitter-dbt's SQL injection (`highlight/mod.rs`).
- **Spans** are split at line breaks and trimmed of the blanks around
  them, so every span sits on one line.
- **The cache** keys open documents by their generation (the server's own
  edit counter) rather than the client's `version`, which is echoed only;
  files by their stamp (mtime, size).
- **Neovim repaints** send the buffer's text (`{text, language, version}`),
  so an answer always matches what is on screen, whatever the timing of
  `didChange`.
- **Macros** are no node of `dbt.projectGraph`: the explorer learns them
  from `workspace/symbol` and adds them when every kind is shown; side
  panes re-centre on any neighbour they name.
- **Generic tests** have no graph node either: `dbt.nodeInfo` finds their
  `{% test %}` blocks in the project's macro and test paths and every macro
  file the context knows; `:DbtNode generic_test:name` shows their card.
- **`quiet`** on `dbt.nodeInfo` answers an unknown node with `null` alone,
  for a client that tries kinds in turn.
