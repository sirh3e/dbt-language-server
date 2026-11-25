# The lineage site

A column-lineage dashboard of a dbt project, in the spirit of dbt docs'
lineage graph and dbt-colibri, built from cogitator's own static analysis:
no `dbt run`, no warehouse, no network. One front end, two places to see it:

- **`cogitator lineage-site`** writes one self-contained `index.html` (the
  data as JSON, the script and the stylesheet inline) that opens from
  `file://` in any browser, plus the data alone as `lineage.json`.
- **VS Code**: `dbt: Show lineage view` draws the same page in a panel beside
  the editor, fed by the server (`dbt.lineageSite`).

Neovim (`:DbtLineageSite`) and Emacs (`M-x cogitator-lineage-site`) ask the
server to write the page into `target/cogitator-lineage/` and open it in the
browser.

## The command

```sh
cogitator lineage-site [--project-dir DIR] [--out DIR] [--open]
```

| Option | Default | |
|---|---|---|
| `--project-dir` | `.` | the directory holding `dbt_project.yml` |
| `--out` | `<project>/target/cogitator-lineage` | where `index.html` and `lineage.json` go |
| `--open` | off | hand the page to the default browser (`xdg-open`, `open`, `start`) |

It prints the page's path on stdout and what was traced on stderr, and exits
1 (with the reason and a solution) when there is no `dbt_project.yml` or the
output directory cannot be written. Profiles, `target/manifest.json`,
`target/catalog.json` and `target/compiled/` are read as the language server
reads them; nothing is run.

## What the page shows

- **The DAG**: every node of `dbt.projectGraph` — models, seeds, snapshots,
  sources, exposures, metrics, semantic models, singular tests and analyses
  (the last two hidden at first) — laid out left to right by depth. Drag or use
  the arrow keys to pan, the wheel or `+`/`-` to zoom, `0` or *Fit* to reset.
  Every node is a button: `Tab` reaches it, `Enter` selects it.
- **A node's details**: kind, file (a link; in VS Code it opens the file),
  materialization (resolved like dbt: `dbt_project.yml`, then the properties
  entry, then `config()`), package, description (`{{ doc() }}` resolved),
  tags, the tests that depend on it, its upstream and downstream neighbours,
  and its columns — name, YAML `data_type` or a proven type, description,
  column tests, and how the column is made.
- **Column lineage**: selecting a column highlights every column it reads
  (upstream, to the sources and seeds) and every column that reads it
  (downstream), across models; the panel lists both, nearest first, and the
  graph dims everything else. CTEs are collapsed to their model: a hop is
  always model column → model column.
- **Search**: `/` focuses the box; it finds nodes and `model.column`s by every
  word typed. Arrow keys move through the results, `Enter` picks one, `Esc`
  clears. `Escape` elsewhere leaves a column's lineage.
- **Light and dark**: the system's choice, or *Theme* (auto → light → dark).

## How a column is made

Each output column carries one of these, read off the project schema job's
provenance (the same facts the type checks use):

| Transform | Means |
|---|---|
| passed through | one upstream column, unchanged, under its own name |
| renamed | one upstream column, unchanged, under another name (`renamed from id`) |
| derived | a cast, a function, an aggregate, a window, a set operation or another expression (`derived: sum() aggregate`) |
| a literal | no column feeds it |
| how it is made is not known | the schema job could not tell: opaque SQL, or an upstream whose columns it does not know (a seed without `target/catalog.json`) |

When the schema job has no answer for a model (it did not run, or the model
is outside it) the column says nothing about how it is made; its inputs are
still shown.

## What is not traced, and why it says so

Nothing is guessed. A model whose lineage cogitator cannot claim is drawn
with a dashed border and a `?`, and its panel says why:

- **Jinja control flow** (`{% if %}`, `{% for %}`, …) and no compiled SQL known
  fresh: blanked branches would invent lineage. Run `dbt compile` and
  generate again; a fresh `target/compiled` file is then read instead
  ("traced from fresh compiled SQL").
- **SQL that did not parse completely**: a partial answer would read as a
  complete one.
- **Snapshots**: the snapshot block is Jinja.

An unexpanded `*` (its inputs' columns are not known) is listed as a column
named `*` and counted as undetermined. A column the page knows only because
a downstream model reads it (a seed's, a source table's not documented in
YAML) says "seen downstream only". A relation the project does not define
(a hard-coded table) is listed under "Outside the project".

The counts in the header: *columns traced* are the output columns of traced
models whose inputs are known (or that are literals); *undetermined* are
their other columns (unexpanded `*`, no inputs found) plus the documented
columns of declined models.

## The data

`lineage.json` (and the `site` of `dbt.lineageSite`) is format 1:

```json
{
  "format": 1, "project": "jaffle_shop", "generator": "cogitator 0.6.0",
  "nodes": [{
    "id": "model:stg_customers", "name": "stg_customers", "kind": "model",
    "layer": 1, "uri": "file:///…/stg_customers.sql",
    "path": "models/staging/stg_customers.sql", "package": "jaffle_shop",
    "materialized": "view", "description": "", "tags": [],
    "tests": ["unique_stg_customers_customer_id"],
    "lineage": "traced", "note": null,
    "columns": [{
      "name": "customer_id", "description": "", "dataType": null,
      "documented": true, "produced": true, "tests": ["unique", "not_null"],
      "inputs": [{"node": "model:raw_customers", "column": "id", "relation": null}],
      "star": false, "transform": "unknown", "detail": null
    }]
  }],
  "edges": [{"parent": "model:raw_customers", "child": "model:stg_customers"}],
  "counts": {"nodes": 15, "edges": 14, "columns": 39, "columnsTraced": 25, "…": 0}
}
```

Node ids and edges are `dbt.projectGraph`'s (`parent` is its `from`, the
dependency). A source is one node per source; its columns are named
`table.column`. `lineage` is `traced`, `compiled`, `declined` or
`notApplicable`. Nodes are sorted by layer then id, edges by their ends: the
same project gives the same bytes.

## In VS Code

`dbt: Show lineage view` opens the panel beside the editor and focuses the
active model; switching files follows them, saving a SQL, YAML or CSV file
refreshes the data. Clicking a file in the panel opens it, at the selected
column's first mention when one is selected. The webview's
Content-Security-Policy allows only its own nonce'd script and style; the
extension carries no copy of the page — the script, stylesheet and data all
come from the server.

## Limits

- Lineage is one hop per model, from the model's SQL; a macro that emits
  columns is opaque to it.
- The layout is a layered layout without crossing minimisation beyond two
  barycentre sweeps; very wide projects are best read zoomed in.
- Large projects make large pages: the data is embedded whole (on the
  order of 4–5 KB per model).
