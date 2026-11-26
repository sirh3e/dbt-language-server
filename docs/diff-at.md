# Comparing a model with its past: `dbt.diffAt`

`dbt.diffAt {uri, rev?, what?, show?}` compares a model as it is now (the
open buffer, saved or not) with the same model at another revision.

## Which revision

- **A git revision** — `HEAD` (the default), a branch, a tag, a sha,
  `main~3`. The server runs `git show <commit>:<path>` in the model's
  repository (10 s timeout, no prompts). It says so, as an error with a
  reason and a fix, when git is not installed, the project is not in a git
  repository, the revision names no commit, or the file did not exist at
  that revision.
- **`last-run`** — what dbt last saw: `raw_code` from `target/manifest.json`
  for the template, and for the compiled view the file under
  `target/compiled/<package>/<path>` (else the `compiled_code` that
  `run_results.json` or the manifest recorded). The manifest is read on
  demand; nothing is kept in memory.

## Which view

| `what` | Compared |
|---|---|
| `sql` (default) | the template text |
| `compiled` | both sides rendered with `dbt.expandJinja`'s in-process renderer (the same stubs for `ref`, `source`, `var`, `config`, `is_incremental()`), so the diff shows what the template change does; for `last-run` the old side is what dbt really compiled, against the current render — differences in stubbed macros can show up there |
| `lineage` | the column lineage of both texts: one line per output column (`name <- model.column, …`) and, in `columns`, the columns added, removed and those whose upstream sources changed. A side without lineage (Jinja control blocks, SQL the front end cannot bind) is reported in `columns.unknown`, never guessed |

The answer is `{model, rev, what, before, after, diff, identical, columns?, note?}`;
`rev` names the commit it resolved to (`HEAD (abc1234567)`). With
`show: true` the diff is written to
`target/.cogitator/diff/<model>@<rev>.<what>.diff` and opened.

## In the editors

- Neovim: `:DbtDiffAt [rev] [sql|compiled|lineage]` (either order).
- Emacs: `M-x cogitator-diff-at` asks for the revision and the view.
- VS Code: "dbt: Compare this model with a git revision or the last run".

## Not done

- The breaking-change preview still compares with the last build, not with
  a git revision.
- Renamed files are not followed through history: the path must exist at
  the revision.
