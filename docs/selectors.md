# Node selection

cogitator evaluates dbt's node-selection language — what `--select`,
`--exclude` and `selectors.yml` say — over the project it has already
read, without running dbt. One engine
(`crates/cogitator-analysis/src/selectors/`) serves every feature that
reads a selector:

- `dbt.select {selector}` lists the nodes a selector picks, as
  `dbt ls --select` would: Neovim `:DbtSelect` (quickfix) and
  `:DbtFind select` (picker), Emacs `M-x cogitator-select`, VS Code
  "dbt: Select nodes" (a quick pick that jumps to each node).
- In `selectors.yml`:
  - completion of methods (`method:` values and `method:` prefixes in a
    string) and of their values: tags, paths, file names, packages, node
    names, `config.<key>` keys and the values in use, resource types,
    sources, selector names;
  - diagnostics: an unknown method or key, a string that does not parse,
    a `selector:` reference to nothing, a cycle between selectors, two
    selectors of one name, more than one `default: true` (errors); a
    selector that matches no node (a warning, on pull diagnostics); a
    `state:`/`result:`/`source_status:` criterion (information: it needs
    a previous run);
  - a "N nodes match" code lens on each definition (it runs `dbt.select`)
    and a hover listing the first ten nodes.
- Tag names complete wherever dbt reads tags: `tags:` and `+tags:` in
  dbt_project.yml and properties files.
- `workspace/symbol`'s `tag:x` query (nodes whose tag contains `x`, or
  matches it when `x` is a glob), the "Run this test" lenses' selectors and
  `dbt.runSelection`'s validation go through the same parser.

## What is supported

| Syntax | Meaning |
|---|---|
| `a b` | union |
| `a,b` | intersection |
| `+x`, `N+x` | x and all its ancestors (N generations) |
| `x+`, `x+N` | x and all its descendants (N generations) |
| `@x` | x, its descendants, and all of their ancestors |
| `method:value` | one method; no method means `path` for a value with `/`, `file` for a `.sql`/`.py`/`.csv` name, `fqn` otherwise |
| `selector:name` | a selectors.yml definition (also `method: selector`) |

Methods: `fqn` (the name, `name.v2`, or a dotted prefix of
package/directories/name where a `*` part matches everything below), `tag`,
`path` (a file, or every file under a directory, relative to its project),
`file` (file name or stem), `package` (`this` is the root project),
`config.<key>` (dotted keys reach into `meta`; a list config holds the
value; booleans compare case-insensitively), `resource_type`, `source`
(`src`, `src.table`, `pkg.src.table`), `exposure`, `metric`,
`semantic_model`, `unit_test`, `test_type` (`generic`, `singular`, `unit`;
`schema` and `data` are aliases), `test_name`, `group`, `access`,
`version` (`latest`, `prerelease`, `old`, `none`) and `wildcard`. Values
match with Python's `fnmatchcase`, as dbt's do: `*`, `?`, `[…]`,
case-sensitive.

selectors.yml definitions: a string in the syntax above; a method mapping
(`method`, `value`, `parents`, `children`, `parents_depth`,
`children_depth`, `childrens_parents`, `indirect_selection`, `exclude`) or
the one-key shorthand `{tag: nightly}`; `union:` and `intersection:` lists
with at most one `exclude:` element.

Tests come along by dbt's indirect selection: `eager` (the default: any
test of a selected node), `cautious` (tests all of whose parents are
selected), `buildable` (parents selected or ancestors of the selection)
and `empty`, per criterion and resolved at each union, intersection and
difference as dbt-core's `NodeSelector` does.

## How the project's nodes are read

Models, seeds and snapshots come from the context; their config is
layered as dbt layers it — dbt_project.yml's resource section along the
node's fqn (deeper wins, `+key` or a plain key that is no directory), the
properties entry's `config:` and `tags`/`meta`/`docs`/`access`/`group`,
then the SQL `config(materialized=…)` — with `tags` adding up and `meta`
merging, and dbt's defaults (`materialized: view` for models). Versioned
models are one node per version (`defined_in`, else `<name>_v<N>`, else
`<name>` for the latest). Sources are one node per table. Generic tests
come from the properties files (a `relationships` test also depends on its
`to: ref(…)`), unit tests from `unit_tests:`, singular tests and analyses
from their paths. Edges are each file's own `ref()` and `source()` calls,
and exposures', metrics' and semantic models' `depends_on`.

## Not supported

- **`state:`, `result:`, `source_status:`** need a previous run's
  manifest, results or freshness (`--state`); they are reported and select
  nothing.
- **`saved_query:`**: saved queries are not read; it selects nothing.
- **Config set in SQL beyond `materialized` and `tags`**: other keys of a
  `{{ config(...) }}` call are not read, so `config.schema:x` sees only the
  YAML and dbt_project.yml layers.
- **Disabled nodes** (`enabled: false`) are still selectable; dbt drops
  them from the graph.
- **`--exclude`** is a CLI flag, not part of a selector string: use an
  `exclude:` in selectors.yml.
- **Packages' tests and analyses**: an installed package's models, seeds
  and snapshots are nodes with their edges, but only the root project's
  singular tests and analyses are.
- **Tag inheritance by tests**: a generic test gets the tags of its own
  config (dbt_project.yml's `data_tests:` section), not its parent's — as
  in dbt; select a model's tests through indirect selection.
- **`path:` globs** match the project's known files, not the file system:
  a directory with no node in it matches nothing.
- Test unique ids lack dbt's hash suffix.
