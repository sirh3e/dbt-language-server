# File operations and declaration

cogitator advertises LSP 3.16's `workspace.fileOperations`, so an editor
that sends them (VS Code does; Neovim 0.11 and Emacs's eglot do not by
themselves) keeps the project consistent when you work in its file tree.
It also answers `textDocument/declaration`, which separates a model's
YAML properties entry from its SQL.

## Which files

The filters cover the default dbt resource directories, `models`,
`macros`, `snapshots`, `seeds`, `tests` and `analyses` (anywhere below
the workspace), and their `.sql`, `.py`, `.yml`, `.yaml` and `.csv`
files; renames and deletes cover the folders below them too. A project
with custom `model-paths` still gets correct answers for the files the
editor does send: the server decides from the project, not the glob.

## Renaming or moving a file

Before the editor renames a file, it asks `workspace/willRenameFiles`.
When the file is a model (or a seed) and its new name is a different
model name, the answer edits every `ref('old')` — in SQL, in YAML Jinja
and in plain YAML scalars such as relationship tests' `to:` — and the
model's properties-YAML `name:`. The editor then renames the file
itself, so the answer holds no file operation (unlike
`textDocument/rename`, which also renames the file).

Nothing is edited when:

- the file moves to another folder under the same name — dbt sees the
  same model;
- it is a macro file (a macro is named by its `{% macro %}` header), a
  properties YAML file, a snapshot (named by its `{% snapshot %}`
  block), or a folder;
- the new name is not a valid model name, or changes the file's kind
  (`.sql` to `.py`).

Several files renamed at once are merged: each file gets one edit.

## Deleting a file

`workspace/willDeleteFiles` answers an empty edit — nothing is rewritten —
but when a deleted model still has dependents, one warning per request
names them, for example:

> Deleting a model with dependents: `stg_payments` is still ref()'d by
> customers, orders. Those refs will dangle.

A deleted folder counts every model below it, and a dependent deleted
in the same request does not count.

## After the fact

`workspace/didCreateFiles`, `didRenameFiles` and `didDeleteFiles` each
refresh the project context of the roots they touch. A new model under
a model path with no properties entry gets its **Extract schema YAML**
lens and code action as soon as that refresh ends; nothing is written
for you. This does not depend on `workspace/didChangeWatchedFiles`,
whose dynamic registration the server does not request.

## Declaration and definition

| Cursor on | Declaration | Definition |
|---|---|---|
| a `ref('model')` | the model's `name:` in its properties YAML | the model's SQL file |
| a model file (no special token) | its own properties entry | — |
| a documented column in SQL | its YAML column entry | as before |
| a column entry in properties YAML | that entry | the select item of the model's SQL producing it |
| a `source()`, macro, var, doc | the same as definition | unchanged |

Where there is no YAML entry, declaration answers what definition does.
The producing select item is the last `… as column` or
`[qualifier.]column,` item of the model's SQL (its final select comes
after its CTEs), else its first mention, else the file's start.
