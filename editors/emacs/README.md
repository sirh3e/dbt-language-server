# cogitator for Emacs

Emacs (29+) integration for the cogitator dbt language server. Works with
the built-in **eglot** client and with **lsp-mode**; the dbt commands and
the model-graph browser work under either.

## 1. Install the server

The integration spawns a `cogitator` binary; put one on your PATH:

```sh
curl -fsSL https://raw.githubusercontent.com/sirh3e/dbt-language-server/main/install.sh | sh
```

(verifies the download against the release's `SHA256SUMS` and
installs to `~/.local/bin`). Alternatively unpack an archive from the
[releases page](https://github.com/sirh3e/dbt-language-server/releases)
or build from source: `cargo build --release -p cogitator`.

Not on PATH? `(setq cogitator-server-command '("/path/to/cogitator"))`.

## 2. Install the elisp

The lisp lives in this repository under `editors/emacs` — clone and
add it to `load-path` (snippets below), or with `use-package`:

```elisp
(use-package cogitator
  :load-path "~/src/dbt-language-server/editors/emacs"
  :config
  (require 'cogitator-graph)
  (cogitator-setup-eglot)
  (add-hook 'sql-mode-hook #'eglot-ensure)
  (add-hook 'yaml-mode-hook #'eglot-ensure))
```

## Setup (eglot)

```elisp
(add-to-list 'load-path "/path/to/dbt-language-server/editors/emacs")
(require 'cogitator)
;; optional configuration, before setup:
;; (setq cogitator-server-command '("/path/to/cogitator"))
;; (setq cogitator-profiles-dir "~/elsewhere/dbt-profiles")
;; (setq cogitator-fusion "/path/to/dbt")      ; or t
(cogitator-setup-eglot)
(add-hook 'sql-mode-hook #'eglot-ensure)
(add-hook 'yaml-mode-hook #'eglot-ensure)
```

The dbt project root (`dbt_project.yml` upward) is registered with
project.el, so eglot picks the right workspace root even without VC.

## Setup (lsp-mode)

```elisp
(require 'cogitator)
(cogitator-setup-lsp-mode)
(add-hook 'sql-mode-hook #'lsp-deferred)
```

## Commands

| command | effect |
|---|---|
| `M-x cogitator-run` | dbt run for the current model (async, results as messages) |
| `M-x cogitator-test` | dbt test |
| `M-x cogitator-compile` | dbt compile; success opens the compiled SQL |
| `M-x cogitator-schema` | jump to the model's properties-YAML entry |
| `M-x cogitator-lineage` | column lineage as a message |
| `M-x cogitator-graph` | model-graph browser (below) |

Suggested bindings:

```elisp
(with-eval-after-load 'cogitator
  (keymap-set sql-mode-map "C-c d g" #'cogitator-graph)
  (keymap-set sql-mode-map "C-c d s" #'cogitator-schema)
  (keymap-set sql-mode-map "C-c d r" #'cogitator-run))
```

## Model-graph browser

`M-x cogitator-graph` (requires `(require 'cogitator-graph)`) opens three
side-by-side windows: direct **upstream** models left, the **current
model** centered, direct **downstream** models right.

| key | effect |
|---|---|
| `n`/`p` (or `j`/`k`) | select an entry |
| `h` / `l` / `TAB` | move between the panes |
| `RET` | re-center the graph on the selected neighbor |
| `o` | open the selected entry's file (center pane: the model) |
| `q` | close and restore the previous window layout |

Sources are tagged `[source]` (no lineage behind them); dangling refs show
`(missing)`.

## Tests

```
just emacs-test
```

runs unit tests plus a live eglot session against
`testdata/jaffle_shop_duckdb` (needs `cargo build -p cogitator` first; the
recipe does it).
