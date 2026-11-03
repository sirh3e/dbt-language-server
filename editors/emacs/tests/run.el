;;; run.el --- batch tests for the Emacs integration  -*- lexical-binding: t; -*-

;; Usage:  emacs -Q --batch -L editors/emacs -l editors/emacs/tests/run.el
;; Unit tests need only Emacs 29+; the integration half starts a real
;; server (COGITATOR_BIN, default target/debug/cogitator) through eglot
;; against testdata/jaffle_shop_duckdb.

;;; Code:

(setq debug-on-error nil)

(defvar run--failures 0)
(defun check (ok label &optional detail)
  (if ok
      (princ (format "ok   %s\n" label))
    (setq run--failures (1+ run--failures))
    (princ (format "FAIL %s%s\n" label (if detail (format " — %S" detail) "")))))

(defvar run--here (file-name-directory (or load-file-name buffer-file-name)))
(defvar run--repo (expand-file-name "../../.." run--here))

(require 'cogitator (expand-file-name "../cogitator.el" run--here))
(require 'cogitator-graph (expand-file-name "../cogitator-graph.el" run--here))

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

;; ---------------------------------------------------------------------------
;; integration: eglot against the live server and the jaffle fixture
;; ---------------------------------------------------------------------------

(defvar run--bin (or (getenv "COGITATOR_BIN")
                     (expand-file-name "target/debug/cogitator" run--repo)))
(defvar run--fixture (expand-file-name "testdata/jaffle_shop_duckdb" run--repo))

(unless (file-exists-p run--bin)
  (princ (format "FAIL integration: server binary missing at %s\n" run--bin))
  (kill-emacs 1))

(require 'eglot)
(setq cogitator-server-command (list run--bin)
      cogitator-profiles-dir (expand-file-name ".dbt" run--fixture))
(cogitator-setup-eglot)

(find-file (expand-file-name "models/customers.sql" run--fixture))
(unless (derived-mode-p 'sql-mode) (sql-mode))
(apply #'eglot (append (eglot--guess-contact) '(nil)))
(cl-loop repeat 100 until (eglot-managed-p) do (accept-process-output nil 0.1))
(check (eglot-managed-p) "integration: eglot manages the model buffer")

(defun run--exec (command argument)
  (jsonrpc-request (eglot--current-server-or-lose)
                   :workspace/executeCommand
                   (list :command command :arguments (vector argument))))

;; readiness: poll dbt.modelGraph until the first context refresh lands
(defvar run--graph nil)
(cl-loop repeat 75
         do (setq run--graph
                  (ignore-errors
                    (run--exec "dbt.modelGraph" (cogitator--command-argument))))
         until (and run--graph (plist-get run--graph :model))
         do (sleep-for 0.2))
(check (equal (plist-get run--graph :model) "customers")
       "integration: dbt.modelGraph answers for customers" run--graph)
(let ((upstream (mapcar (lambda (e) (plist-get e :name))
                        (append (plist-get run--graph :upstream) nil))))
  (check (member "stg_customers" upstream)
         "integration: direct upstream lists stg_customers" upstream))

(let ((location (run--exec "dbt.goToSchema" (cogitator--command-argument t))))
  (check (and location (string-suffix-p ".yml" (plist-get location :uri)))
         "integration: dbt.goToSchema returns a schema-YAML Location" location))

;; integration: cogitator-relation copies database.schema.identifier
(goto-char (point-min))
(kill-new "run--sentinel")
(cogitator-relation)
(cl-loop repeat 50
         until (not (equal (current-kill 0) "run--sentinel"))
         do (accept-process-output nil 0.1))
(check (equal (current-kill 0) "jaffle_shop.main.customers")
       "integration: cogitator-relation copies the relation" (current-kill 0))

;; integration: cogitator-query builds the bounded SELECT
(let ((orig (current-buffer)))
  (goto-char (point-min))
  (cogitator-query)
  (cl-loop repeat 50
           until (and (get-buffer "*cogitator-query*")
                      (with-current-buffer "*cogitator-query*"
                        (string-match-p "select \\* from jaffle_shop\\.main\\.customers limit 100"
                                        (buffer-string))))
           do (accept-process-output nil 0.1))
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
(cl-loop repeat 50
         until (plist-get cogitator-graph--state :data)
         do (accept-process-output nil 0.1))
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
(cl-loop repeat 50
         until (not (equal (plist-get (plist-get cogitator-graph--state :data) :model)
                           "customers"))
         do (accept-process-output nil 0.1))
(check (not (equal (plist-get (plist-get cogitator-graph--state :data) :model)
                   "customers"))
       "integration: RET re-centers on the neighbor"
       (plist-get (plist-get cogitator-graph--state :data) :model))

(cogitator-graph-quit)
(check (null cogitator-graph--state) "integration: graph browser closes cleanly")

;; live profiles-dir switch (last: it changes the session's dialect state)
(let ((other-dir (expand-file-name (format "cogitator-el-profiles-%d" (emacs-pid))
                                   temporary-file-directory)))
  (make-directory other-dir t)
  (with-temp-file (expand-file-name "profiles.yml" other-dir)
    (insert "jaffle_shop:\n  target: dev\n  outputs:\n    dev:\n      type: snowflake\n"))
  (let ((switched (run--exec "dbt.setProfilesDir" (list :dir other-dir))))
    (check (equal (plist-get switched :dir) other-dir)
           "integration: dbt.setProfilesDir answers the new dir" switched))
  (let ((report nil))
    (cl-loop repeat 50
             do (setq report (ignore-errors
                               (run--exec "dbt.profiles" (cogitator--command-argument))))
             until (and report
                        (string-prefix-p other-dir (or (plist-get report :path) "")))
             do (sleep-for 0.2))
    (check (and report (string-prefix-p other-dir (or (plist-get report :path) "")))
           "integration: dbt.profiles follows the live dir switch"
           (and report (plist-get report :path)))))

(princ (if (zerop run--failures) "ALL PASSED\n"
         (format "%d failure(s)\n" run--failures)))
(kill-emacs (if (zerop run--failures) 0 1))

;;; run.el ends here
