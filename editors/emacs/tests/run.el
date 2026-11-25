;;; run.el --- batch tests for the Emacs integration  -*- lexical-binding: t; -*-

;; Usage:  emacs -Q --batch -L editors/emacs -l editors/emacs/tests/run.el
;; Unit tests need only Emacs 29+; the integration half starts a real
;; server (COGITATOR_BIN, default target/debug/cogitator) through eglot
;; against a per-run COPY of testdata/jaffle_shop_duckdb, inside a
;; sandboxed HOME. Readiness is the dbt.status refresh barrier, and
;; every wait is labeled with a deadline (tests/helpers.el).

;;; Code:

(setq debug-on-error nil)

(defvar run--here (file-name-directory (or load-file-name buffer-file-name)))
(defvar run--repo (expand-file-name "../../.." run--here))

(load (expand-file-name "helpers.el" run--here) nil t)
(require 'cogitator (expand-file-name "../cogitator.el" run--here))
(require 'cogitator-graph (expand-file-name "../cogitator-graph.el" run--here))

(defalias 'check #'cogitator-test-check)

;; ---------------------------------------------------------------------------
;; unit: flag assembly and the executeCommand argument (spec §1.2, §1.6)
;; ---------------------------------------------------------------------------

(let ((cogitator-server-command '("cogitator"))
      (cogitator-tree-sitter nil)
      (cogitator-debug t)
      (cogitator-fusion "/opt/dbt"))
  (check (equal (cogitator--server-command)
                '("cogitator" "--no-tree-sitter" "--debug" "--fusion" "/opt/dbt"))
         "config: flags assembled from options" (cogitator--server-command)))
(let ((cogitator-fusion t))
  (check (equal (cogitator--server-command) '("cogitator" "--fusion"))
         "config: fusion = t adds bare --fusion"))
(let ((cogitator-profiles-dir "/data/profiles"))
  (check (equal (cogitator--initialization-options) '(:profilesDir "/data/profiles"))
         "config: profiles dir becomes initializationOptions"))
(check (null (cogitator--initialization-options))
       "config: no init options when nothing set")

(with-temp-buffer
  (setq buffer-file-name "/tmp/arg-shape.sql")
  (insert "select 1\nfrom t")
  (goto-char (point-min))
  (forward-line 1)
  (forward-char 3)
  (let ((arg (cogitator--command-argument t)))
    (check (and (equal (plist-get arg :uri) "file:///tmp/arg-shape.sql")
                (equal (plist-get arg :position) '(:line 1 :character 3)))
           "commands: argument object is { uri, position } (0-based)" arg))
  (setq buffer-file-name nil))

(check (equal (cogitator--normalize
               (let ((ht (make-hash-table :test #'equal)))
                 (puthash "name" "x" ht)
                 (puthash "list" (vector 1) ht)
                 ht))
              '(:name "x" :list [1]))
       "client: lsp-mode hash tables normalize to plists")

;; unit: the lineage page's message
(check (equal (cogitator--lineage-site-message
               '(:written (:path "/p/target/cogitator-lineage/index.html")
                 :counts (:columnsTraced 25 :columns 39 :columnsUndetermined 14 :modelsDeclined 1)))
              "cogitator: lineage page /p/target/cogitator-lineage/index.html — 25 of 39 columns traced, 14 undetermined, 1 models declined")
       "lineage-site: message says where the page is and what was traced")
(check (null (cogitator--lineage-site-message nil))
       "lineage-site: no page, no message")

;; ---------------------------------------------------------------------------
;; integration: eglot against the live server and a fixture COPY
;; ---------------------------------------------------------------------------

(defvar run--bin (cogitator-test-server-bin run--repo))
(unless (file-exists-p run--bin)
  (princ (format "FAIL integration: server binary missing at %s\n" run--bin))
  (kill-emacs 1))

;; sandbox FIRST: the eglot child inherits the environment, so HOME and
;; friends must point into the throwaway dir before the server spawns
(cogitator-test-sandbox)
(defvar run--fixture (cogitator-test-copy-fixture run--repo))

(require 'eglot)
(setq cogitator-server-command (list run--bin)
      cogitator-profiles-dir (expand-file-name ".dbt" run--fixture))
(cogitator-setup-eglot)

(find-file (expand-file-name "models/customers.sql" run--fixture))
(unless (derived-mode-p 'sql-mode) (sql-mode))
(cogitator-test-eglot-start)
(cogitator-test-wait "integration: eglot manages the model buffer" 15
                     #'eglot-managed-p)
(check (eglot-managed-p) "integration: eglot manages the model buffer")

(defalias 'run--exec #'cogitator-test-exec)

;; THE readiness barrier (spec §8): refresh.count >= 1 means the first
;; project scan completed — not the modelGraph-answers proxy
(cogitator-test-wait-refresh 1 30)

(let ((graph (run--exec "dbt.modelGraph" (cogitator--command-argument))))
  (check (equal (plist-get graph :model) "customers")
         "integration: dbt.modelGraph answers for customers" graph)
  (let ((upstream (mapcar (lambda (e) (plist-get e :name))
                          (append (plist-get graph :upstream) nil))))
    (check (member "stg_customers" upstream)
           "integration: direct upstream lists stg_customers" upstream)))

(let ((location (run--exec "dbt.goToSchema" (cogitator--command-argument t))))
  (check (and location (string-suffix-p ".yml" (plist-get location :uri)))
         "integration: dbt.goToSchema returns a schema-YAML Location" location))

;; integration: cogitator-relation copies database.schema.identifier
(goto-char (point-min))
(kill-new "run--sentinel")
(cogitator-relation)
(cogitator-test-wait "integration: cogitator-relation answers" 10
                     (lambda () (not (equal (current-kill 0) "run--sentinel"))))
(check (equal (current-kill 0) "jaffle_shop.main.customers")
       "integration: cogitator-relation copies the relation" (current-kill 0))

;; integration: cogitator-query builds the bounded SELECT
(let ((orig (current-buffer)))
  (goto-char (point-min))
  (cogitator-query)
  (cogitator-test-wait
   "integration: cogitator-query scratch buffer" 10
   (lambda ()
     (and (get-buffer "*cogitator-query*")
          (with-current-buffer "*cogitator-query*"
            (string-match-p "select \\* from jaffle_shop\\.main\\.customers limit 100"
                            (buffer-string))))))
  (check (and (get-buffer "*cogitator-query*")
              (with-current-buffer "*cogitator-query*"
                (string-match-p "select \\* from jaffle_shop\\.main\\.customers limit 100"
                                (buffer-string))))
         "integration: cogitator-query builds the bounded SELECT")
  (pop-to-buffer orig))

;; integration: dbt.cache wire shape
(let ((caches (run--exec "dbt.cache"
                         (append (cogitator--command-argument)
                                 '(:action "stats")))))
  (check (and (plist-get caches :profiles)
              (numberp (plist-get (plist-get caches :profiles) :entries)))
         "integration: dbt.cache reports the derived-cache surfaces" caches))

;; integration: dbt.lineageSite writes the page under target/
(let ((site (run--exec "dbt.lineageSite"
                       (append (cogitator--command-argument) '(:write t)))))
  (check (and (file-exists-p (plist-get (plist-get site :written) :path))
              (> (plist-get (plist-get site :counts) :columnsTraced) 0))
         "integration: dbt.lineageSite writes the page" site))

;; pickers + doctor parity: the data each one feeds on, plus rendering
(let ((graph (run--exec "dbt.projectGraph" (cogitator--command-argument))))
  (check (seq-some (lambda (n) (member "nightly" (append (plist-get n :tags) nil)))
                   (append (plist-get graph :nodes) nil))
         "integration: projectGraph carries tags for the tags picker"))
(let ((symbols (append (jsonrpc-request (eglot--current-server-or-lose)
                                        :workspace/symbol '(:query "selector:"))
                       nil)))
  (check (> (length symbols) 0)
         "integration: selector: symbols exist for the selectors picker"))
(let ((buf (cogitator--profiles-render
            '(:path "/tmp/profiles.yml" :activeProfile "p" :activeTarget "dev"
              :profiles [(:name "p" :allowed t
                          :targets [(:name "dev" :type "duckdb"
                                     :auth (:method "password" :issues []))])]))))
  (check (with-current-buffer buf
           (and (string-match-p "active: p/dev" (buffer-string))
                (string-match-p "dev — duckdb, password" (buffer-string))))
         "unit: profiles report renders profiles and targets"))
(let ((orig (current-buffer)))
  (cogitator-doctor)
  (check (and (get-buffer "*cogitator-doctor*")
              (with-current-buffer "*cogitator-doctor*"
                (string-match-p "cogitator binary" (buffer-string))))
         "integration: doctor reports binary and candidates")
  (pop-to-buffer orig))

;; integration: code actions are the executable lens set, command-only
(let* ((zero '(:line 0 :character 0))
       (actions (append
                 (jsonrpc-request (eglot--current-server-or-lose)
                                  :textDocument/codeAction
                                  (list :textDocument (list :uri (concat "file://" (buffer-file-name)))
                                        :range (list :start zero :end zero)
                                        :context '(:diagnostics [])))
                 nil))
       (titles (mapcar (lambda (a) (plist-get a :title)) actions)))
  (check (and (member "dbt run" titles) (member "Go to schema" titles))
         "integration: code actions offer the dbt command set" titles))

;; unit: the preview table is the server's markdown
(check (equal (cogitator--preview-cte-render '(:model "m" :table "| id |\n| --- |\n| 1 |\n"))
              "| id |\n| --- |\n| 1 |\n")
       "preview-cte: renders the server's table")
(check (equal (cogitator--preview-cte-render '(:model "m")) "")
       "preview-cte: no table renders empty")

;; unit: split plan and incrementality rendering
(let ((text (cogitator--split-render
             '(:model "m" :sexp "(split m (model m__c))"
               :part (:name "m__c" :cte "c" :text "select 1\n")))))
  (check (and (string-prefix-p "(split m" text)
              (string-match-p ";; m__c.sql" text)
              (string-match-p "select 1" text))
         "split: renders the plan and the new model" text))
(check (equal (cogitator--split-render '(:model "m" :sexp "(split m (refused no-boundary))" :part nil))
              "(split m (refused no-boundary))\n")
       "split: a refusal is the S-expression alone")
(let ((choices (cogitator--incrementality-choices
                '(:incremental 1
                  :models [(:model "big" :uri "file:///p/big.sql" :lastRunSeconds 12.5
                            :view (:verdict "append" :strategy "append" :summary "append by ts"))
                           (:model "ranked" :uri "file:///p/ranked.sql"
                            :view (:verdict "keep" :strategy nil :reasons [(:reason "a LIMIT")]))]))))
  (check (equal (mapcar #'car choices)
                '("big — append (append) [last run 12.5s]: append by ts"
                  "ranked — keep: a LIMIT"))
         "incrementality: labels name verdict, strategy, run time and why" choices))

;; integration: the incrementality commands over the wire
(let ((report (run--exec "dbt.incrementality" (cogitator--command-argument))))
  (check (and (numberp (plist-get report :incremental))
              (vectorp (plist-get report :models)))
         "integration: dbt.incrementality lists the table models" report))
(let ((plan (run--exec "dbt.splitPlan" (cogitator--command-argument))))
  (check (or (null plan) (eq plan :null) (stringp (plist-get plan :sexp)))
         "integration: dbt.splitPlan answers a plan or null" plan))

;; unit + integration: dbt.incrementalDiff
(check (equal (cogitator--incremental-diff-render
               '(:model "m" :identical :json-false :diff "--- a\n+++ b\n" :fullRefresh "x"))
              "--- a\n+++ b\n")
       "incremental diff: a change shows the unified diff")
(check (string-match-p "changes nothing"
                       (cogitator--incremental-diff-render
                        '(:model "m" :identical t :diff "" :fullRefresh "select 1\n")))
       "incremental diff: an unchanged model says so")
(let ((diff (run--exec "dbt.incrementalDiff"
                       (list :uri (concat "file://"
                                          (expand-file-name "models/staging/stg_customers.sql"
                                                            run--fixture))))))
  (check (and (stringp (plist-get diff :fullRefresh))
              (stringp (plist-get diff :incremental)))
         "integration: dbt.incrementalDiff renders both ways" diff))

;; unit + integration: the syntax tree
(check (equal (cogitator--syntax-line
               '(:depth 1 :kind "select" :field "body" :missing :json-false
                 :range (:start (:line 0 :character 0) :end (:line 1 :character 4))))
              "  body: select [1:0 - 2:4]")
       "syntax: a tree line has depth, field, kind and range")
(check (string-match-p "more nodes"
                       (cogitator--syntax-render '(:nodes [] :truncated t)))
       "syntax: a cut tree says so")
(let ((tree (run--exec "dbt.syntaxTree" (cogitator--command-argument))))
  (check (and (vectorp (plist-get tree :nodes))
              (> (length (plist-get tree :nodes)) 0)
              (equal (plist-get (aref (plist-get tree :nodes) 0) :depth) 0))
         "integration: dbt.syntaxTree answers the model's tree" (plist-get tree :truncated)))

;; unit + integration: highlighting from the server
(check (eq (cogitator--highlight-face "keyword.conditional") 'font-lock-keyword-face)
       "highlight: a dotted capture takes its prefix's face")
(check (eq (cogitator--highlight-face "variable.parameter") 'font-lock-variable-name-face)
       "highlight: the longest prefix wins")
(check (null (cogitator--highlight-face "spell"))
       "highlight: a capture of no colour has no face")
(with-temp-buffer
  (insert "-- \U0001F980 x\nselect 1")
  (check (= (cogitator--utf16-point 0 5) 5) "highlight: a surrogate pair is one character"
         (cogitator--utf16-point 0 5))
  (check (= (cogitator--highlight-apply
             '(:spans [(:range (:start (:line 1 :character 0) :end (:line 1 :character 6))
                        :capture "keyword")]))
            1)
         "highlight: a span becomes an overlay")
  (check (eq (get-char-property 10 'face) 'font-lock-keyword-face)
         "highlight: the overlay carries the face" (get-char-property 10 'face)))
(let ((result (run--exec "dbt.highlight" (list :text "select {{ ref('a') }}" :language "dbt"))))
  (check (seq-some (lambda (s) (equal (plist-get s :capture) "tag.delimiter"))
                   (plist-get result :spans))
         "integration: dbt.highlight colours the Jinja" (plist-get result :language)))

;; unit + integration: highlight v2, dbt captures, node info (specs/highlighting)
(check (eq (cogitator--highlight-face "type.dbt.model") 'font-lock-type-face)
       "highlight: a dbt capture falls back to its parent's face")
(check (eq (cogitator--highlight-face "type.dbt.reference.unresolved") 'font-lock-warning-face)
       "highlight: an unresolved reference is a warning")
(check (equal (mapcar #'cogitator--highlight-language
                      '("/p/models/schema.yml" "/p/target/compiled/p/a.sql" "/p/models/a.sql" nil))
              '("yaml" "sql" "dbt" "dbt"))
       "highlight: the language of a file")
(with-temp-buffer
  (insert "name: customers")
  (cogitator--highlight-apply
   '(:spans [(:range (:start (:line 0 :character 0) :end (:line 0 :character 4))
              :capture "property" :layer "yaml")
             (:range (:start (:line 0 :character 6) :end (:line 0 :character 15))
              :capture "type.dbt.model" :layer "dbt")])
   '("yaml"))
  (let ((ovs (seq-filter (lambda (o) (overlay-get o 'cogitator-highlight))
                         (overlays-in (point-min) (point-max)))))
    (check (and (= (length ovs) 1) (= (overlay-get (car ovs) 'priority) 13))
           "highlight: a skipped layer is left out, the dbt layer on top" (length ovs))))
(check (equal (cogitator--node-parse "macro:times_five") '("macro" . "times_five"))
       "node: kind and name")
(check (equal (cogitator--node-parse " orders ") '(nil . "orders"))
       "node: a name alone")
(let ((text (cogitator--node-render
             '(:kind "macro" :name "cents" :params [(:name "col" :default :null)]
               :callers [(:kind "model" :name "orders")] :calls []))))
  (check (and (string-match-p "cents(col)" text) (string-match-p "called by (1)" text)
              (string-match-p "orders  \\[model\\]" text))
         "node: a macro's card" text))
(let ((info (run--exec "dbt.nodeInfo" (list :node (list :kind "macro" :name "times_five")))))
  (check (equal (plist-get info :kind) "macro")
         "integration: dbt.nodeInfo answers for a macro" info))
(let* ((schema (expand-file-name "models/schema.yml" run--fixture))
       (result (run--exec "dbt.highlight" (list :uri (concat "file://" schema)))))
  (check (and (equal (plist-get result :language) "yaml")
              (seq-some (lambda (s) (equal (plist-get s :capture) "type.dbt.model"))
                        (plist-get result :spans)))
         "integration: a YAML file is highlighted with dbt captures" (plist-get result :language)))

(check (eq (cogitator--highlight-face "variable.dbt.var") 'font-lock-variable-name-face)
       "highlight: a var in YAML takes the variable face")
(check (eq (cogitator--highlight-face "variable.dbt.var.unresolved") 'font-lock-warning-face)
       "highlight: an undefined var is a warning")

;; unit + integration: ast-grep
(check (equal (cogitator--ast-grep-parse "{{ ref($M) }} => {{ ref('x', $M) }}")
              '(:pattern "{{ ref($M) }}" :rewrite "{{ ref('x', $M) }}" :language "dbt"))
       "astgrep: a pattern and its rewrite")
(check (equal (plist-get (cogitator--ast-grep-parse "sql: select $$$C from $T") :language) "sql")
       "astgrep: sql: picks the SQL grammar")
(check (null (cogitator--ast-grep-parse "  ")) "astgrep: no pattern, no query")
(check (equal (cogitator--ast-grep-line
               '(:uri "file:///p/models/a.sql" :text "{{ ref('a') }}" :replacement :null
                 :range (:start (:line 2 :character 5) :end (:line 2 :character 19))))
              "/p/models/a.sql:3:6: {{ ref('a') }}")
       "astgrep: a match is a grep line")
(let ((found (run--exec "dbt.astGrep" (append (cogitator--command-argument)
                                               (list :pattern "{{ ref($M) }}")))))
  (check (= (length (plist-get found :matches)) 4)
         "integration: dbt.astGrep finds the model's refs" (length (plist-get found :matches))))

;; unit + integration: node selection
(check (equal (cogitator--select-line
               '(:name "orders" :version "1" :resourceType "model" :tags ["nightly"]))
              "orders.v1  [model]  #nightly")
       "select: a version is listed as dbt names it")
(check (equal (cogitator--select-line
               '(:name "items" :resourceType "source" :graphId "source:raw" :tags []))
              "raw.items  [source]")
       "select: a source table is source.table")
(let ((found (run--exec "dbt.select" (append (cogitator--command-argument)
                                              (list :select "+customers")))))
  (check (and (> (length (plist-get found :nodes)) 3)
              (plist-get (plist-get found :counts) :model))
         "integration: dbt.select answers +customers" found))

;; unit + integration: unresolved names
(check (equal (cogitator--unresolved-line
               '(:kind "ref" :name "ghost" :uri "file:///p/models/a.sql"
                 :message "model 'ghost' not found in the project"
                 :range (:start (:line 2 :character 14) :end (:line 2 :character 19))))
              "/p/models/a.sql:3:15: [ref] ghost — model 'ghost' not found in the project")
       "unresolved: an item is a grep line")
(let ((found (run--exec "dbt.unresolved" (cogitator--command-argument))))
  (check (and (numberp (plist-get found :count))
              (= (plist-get found :count) (length (plist-get found :items))))
         "integration: dbt.unresolved lists the project's unresolved names" found))

;; unit + integration: breaking changes
(check (equal (cogitator--breaking-lines
               '(:diff (:removed [(:name "customer_id" :possiblyRenamedTo "customer_key")])
                 :impacts [(:column "customer_id" :change "removed" :severity "error"
                            :tests ["unique"]
                            :downstream [(:model "customers" :column "customer_id"
                                          :contracted t)])]))
              '("[error] customer_id removed (possibly renamed to customer_key) — tested: unique"
                "  read by customers.customer_id (contracted)"))
       "breaking: a change and its downstream reads are lines")
(let ((found (run--exec "dbt.breakingChanges" (cogitator--command-argument))))
  (check (stringp (plist-get found :status))
         "integration: dbt.breakingChanges answers a status" found))

;; integration: dbt.select with a selector, as the lenses send it
(let ((found (run--exec "dbt.select" (append (cogitator--command-argument)
                                             (list :selector "tag:nightly")))))
  (check (and (numberp (plist-get found :count))
              (equal (plist-get (aref (plist-get found :nodes) 0) :name) "customers"))
         "integration: dbt.select picks the nightly model" found))

;; unit: test labels
(check (equal (cogitator--test-label '(:name "unique" :column "id" :version 2))
              "unique on id (v2)")
       "tests: label carries column and version")

;; integration: tests navigation wire shape
(let* ((result (run--exec "dbt.modelTests" (cogitator--command-argument)))
       (tests (append (plist-get result :tests) nil)))
  (check (equal (plist-get result :model) "customers")
         "integration: dbt.modelTests answers for customers")
  (check (seq-find (lambda (test)
                     (and (equal (plist-get test :name) "unique")
                          (equal (plist-get test :column) "customer_id")))
                   tests)
         "integration: unique test on customer_id is listed" tests))

;; integration: server status
(let ((s (run--exec "dbt.status" (cogitator--command-argument))))
  (check (and (equal (plist-get (plist-get s :server) :backend) "tree-sitter")
              (> (plist-get (plist-get s :project) :models) 0)
              (>= (plist-get (plist-get s :refresh) :count) 1))
         "integration: dbt.status reports backend and project" s))

;; unit: target-choice labels from a dbt.profiles report
(let ((choices (cogitator--target-choices
                '(:activeProfile "p" :activeTarget "dev"
                  :profiles [(:name "p" :allowed t
                              :targets [(:name "dev" :type "snowflake"
                                         :auth (:method "key-pair" :issues ["x"]))])
                             (:name "hidden" :allowed :json-false :targets [])]))))
  (check (and (= 1 (length choices))
              (string-match-p "p/dev — snowflake, key-pair  ⚠ 1  (active)"
                              (caar choices)))
         "target: choices show auth method, issues and active marker" choices))

;; integration: the auth manager over the wire
(let ((report (run--exec "dbt.profiles" (cogitator--command-argument))))
  (check (equal (plist-get report :activeProfile) "jaffle_shop")
         "integration: dbt.profiles reports the active profile" report)
  (check (equal (plist-get report :activeTarget) "dev")
         "integration: dbt.profiles reports the active target"))
(let ((switched (run--exec "dbt.setTarget" '(:target "dev"))))
  (check (equal (plist-get switched :target) "dev")
         "integration: dbt.setTarget switches the target" switched))

;; the graph browser: three panes, re-centerable, restorable
(cogitator-graph)
(cogitator-test-wait "integration: graph browser fetches its data" 10
                     (lambda () (plist-get cogitator-graph--state :data)))
(check (plist-get cogitator-graph--state :data)
       "integration: graph browser fetched its data")
(check (= 3 (length (cogitator-graph--pane-windows)))
       "integration: graph browser shows three panes")
(let ((upstream-buf (nth 0 (plist-get cogitator-graph--state :buffers))))
  (check (string-match-p "stg_customers"
                         (with-current-buffer upstream-buf (buffer-string)))
         "integration: upstream pane lists stg_customers"
         (with-current-buffer upstream-buf (buffer-string))))

;; re-center on the first upstream entry
(select-window (nth 0 (cogitator-graph--pane-windows)))
(goto-char (point-min))
(cogitator-graph-recenter)
(cogitator-test-wait
 "integration: RET re-centers on the neighbor" 10
 (lambda () (not (equal (plist-get (plist-get cogitator-graph--state :data) :model)
                        "customers"))))
(check (not (equal (plist-get (plist-get cogitator-graph--state :data) :model)
                   "customers"))
       "integration: RET re-centers on the neighbor"
       (plist-get (plist-get cogitator-graph--state :data) :model))

(cogitator-graph-quit)
(check (null cogitator-graph--state) "integration: graph browser closes cleanly")

;; live profiles-dir switch (last: it changes the session's dialect state)
(let ((other-dir (expand-file-name "profiles-switch" cogitator-test-tmp)))
  (make-directory other-dir t)
  (with-temp-file (expand-file-name "profiles.yml" other-dir)
    (insert "jaffle_shop:\n  target: dev\n  outputs:\n    dev:\n      type: snowflake\n"))
  (let ((switched (run--exec "dbt.setProfilesDir" (list :dir other-dir))))
    (check (equal (plist-get switched :dir) other-dir)
           "integration: dbt.setProfilesDir answers the new dir" switched))
  (let ((report nil))
    (cogitator-test-wait
     "integration: dbt.profiles follows the live dir switch" 30
     (lambda ()
       (setq report (ignore-errors
                      (run--exec "dbt.profiles" (cogitator--command-argument))))
       (and report
            (string-prefix-p other-dir (or (plist-get report :path) ""))))
     (lambda () (and report (plist-get report :path))))
    (check (and report (string-prefix-p other-dir (or (plist-get report :path) "")))
           "integration: dbt.profiles follows the live dir switch"
           (and report (plist-get report :path)))))

(cogitator-test-finish)

;;; run.el ends here
