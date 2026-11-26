;;; cogitator.el --- Emacs integration for the cogitator dbt language server  -*- lexical-binding: t; -*-

;; Keywords: languages, tools
;; Package-Requires: ((emacs "29.1"))
;; Version: 0.1.0

;;; Commentary:

;; Emacs support for cogitator (spec: specs/neovim-plugin/00-plugin.md §1
;; holds the server contract).  Works with the built-in eglot client
;; (`cogitator-setup-eglot') and with lsp-mode (`cogitator-setup-lsp-mode').
;; The dbt commands (`cogitator-run', `cogitator-test', `cogitator-compile',
;; `cogitator-schema', `cogitator-lineage') and the model-graph browser
;; (`cogitator-graph' in cogitator-graph.el) speak to whichever client
;; manages the buffer.

;;; Code:

(require 'cl-lib)
(require 'subr-x)
(require 'url-util)
(require 'seq)
(declare-function grep-mode "grep")

(defgroup cogitator nil
  "The cogitator dbt language server."
  :group 'tools
  :prefix "cogitator-")

(defcustom cogitator-server-command '("cogitator")
  "Server executable and base arguments; flags are appended from the options."
  :type '(repeat string))

(defcustom cogitator-profiles-dir nil
  "Directory containing profiles.yml, sent as initializationOptions.profilesDir.
When nil the server searches its own candidate order ($DBT_PROFILES_DIR,
<root>/profiles.yml, <root>/.dbt/profiles.yml, ~/.dbt/profiles.yml)."
  :type '(choice (const nil) directory))

(defcustom cogitator-fusion nil
  "dbt Fusion mode: nil (off), t (binary `dbt'), or a path to the binary."
  :type '(choice (const nil) (const t) string))

(defcustom cogitator-tree-sitter t
  "When nil, pass --no-tree-sitter (phase-1 pipeline fallback)."
  :type 'boolean)

(defcustom cogitator-debug nil
  "When non-nil, pass --debug (log.txt next to the server executable)."
  :type 'boolean)

(defcustom cogitator-major-modes
  '(sql-mode sql-ts-mode yaml-mode yaml-ts-mode)
  "Major modes the server is registered for."
  :type '(repeat symbol))

(defcustom cogitator-query-limit 100
  "Row cap for the bounded SELECT that `cogitator-query' builds."
  :type 'natnum)

;; ---------------------------------------------------------------------------
;; server contract helpers (spec §1.2, §1.3, §1.6)
;; ---------------------------------------------------------------------------

(defun cogitator--server-command ()
  "Full server command with the flags derived from the options."
  (append cogitator-server-command
          (and (not cogitator-tree-sitter) '("--no-tree-sitter"))
          (and cogitator-debug '("--debug"))
          (pcase cogitator-fusion
            ('nil nil)
            ('t '("--fusion"))
            ((pred stringp) (list "--fusion" cogitator-fusion)))))

(defun cogitator--initialization-options ()
  "The initializationOptions plist, or nil when nothing is configured."
  (when cogitator-profiles-dir
    (list :profilesDir (expand-file-name cogitator-profiles-dir))))

(defun cogitator--project-root (&optional file)
  "The dbt project root above FILE (default: this buffer), or nil."
  (when-let* ((start (or file buffer-file-name default-directory)))
    (locate-dominating-file start "dbt_project.yml")))

(defun cogitator--command-argument (&optional with-position)
  "The one argument object every dbt.* command takes (spec §1.6)."
  (append
   (list :uri (concat "file://" (buffer-file-name)))
   (when with-position
     (list :position (list :line (1- (line-number-at-pos nil t))
                           :character (- (point) (line-beginning-position)))))))

;; ---------------------------------------------------------------------------
;; client layer: workspace/executeCommand through eglot or lsp-mode
;; ---------------------------------------------------------------------------

(declare-function eglot-managed-p "eglot")
;; compile-time only: gives the byte compiler eglot's class, functions and
;; variables (eglot ships with Emacs >= 29); runtime loading stays lazy
(eval-when-compile (require 'eglot))
(declare-function eglot--current-server-or-lose "eglot")
(declare-function jsonrpc-async-request "jsonrpc")
(declare-function lsp-request-async "lsp-mode")

(defun cogitator--normalize (value)
  "Normalize a client result to plists: lsp-mode hands out hash tables."
  (cond
   ((hash-table-p value)
    (let (plist)
      (maphash (lambda (k v)
                 (setq plist (plist-put plist (intern (concat ":" k))
                                        (cogitator--normalize v))))
               value)
      plist))
   ((vectorp value) (vconcat (mapcar #'cogitator--normalize value)))
   (t value)))

(defun cogitator--execute (command argument callback)
  "Send workspace/executeCommand COMMAND with ARGUMENT; CALLBACK gets the result.
Errors and a missing client are reported, not raised.  Results reach
CALLBACK as plists whichever client is in charge."
  (cond
   ((and (featurep 'eglot) (eglot-managed-p))
    (jsonrpc-async-request
     (eglot--current-server-or-lose)
     :workspace/executeCommand
     (list :command command :arguments (vector argument))
     :success-fn callback
     :error-fn (lambda (err)
                 (message "cogitator: %s failed: %s" command
                          (plist-get err :message)))))
   ((bound-and-true-p lsp-mode)
    (lsp-request-async
     "workspace/executeCommand"
     (list :command command :arguments (vector argument))
     (lambda (result) (funcall callback (cogitator--normalize result)))
     :error-handler (lambda (err)
                      (message "cogitator: %s failed: %S" command err))))
   (t (user-error "cogitator is not managing this buffer (start eglot or lsp-mode)"))))

(defun cogitator--request (method-kw method-str params callback)
  "Send an arbitrary LSP request through eglot (METHOD-KW) or lsp-mode
\(METHOD-STR); CALLBACK gets the result as plists."
  (cond
   ((and (featurep 'eglot) (eglot-managed-p))
    (jsonrpc-async-request
     (eglot--current-server-or-lose) method-kw params
     :success-fn callback
     :error-fn (lambda (err)
                 (message "cogitator: %s failed: %s" method-str
                          (plist-get err :message)))))
   ((bound-and-true-p lsp-mode)
    (lsp-request-async
     method-str params
     (lambda (result) (funcall callback (cogitator--normalize result)))
     :error-handler (lambda (err)
                      (message "cogitator: %s failed: %S" method-str err))))
   (t (user-error "cogitator is not managing this buffer (start eglot or lsp-mode)"))))

(defun cogitator--visit-location (location)
  "Open the LSP LOCATION (plist with :uri and :range)."
  (let* ((file (cogitator--uri-to-path (plist-get location :uri)))
         (start (plist-get (plist-get location :range) :start)))
    (find-file file)
    (widen)
    (goto-char (point-min))
    (forward-line (plist-get start :line))
    (forward-char (plist-get start :character))))

(defun cogitator--uri-to-path (uri)
  "file:// URI → filesystem path (percent-decoded)."
  (url-unhex-string (string-remove-prefix "file://" uri)))

;; ---------------------------------------------------------------------------
;; the dbt commands (spec §5.1)
;; ---------------------------------------------------------------------------

;;;###autoload
(defun cogitator-run ()
  "dbt run for the current model (async; results arrive as messages)."
  (interactive)
  (cogitator--execute "dbt.run" (cogitator--command-argument) #'ignore))

;;;###autoload
(defun cogitator-test ()
  "dbt test for the current model."
  (interactive)
  (cogitator--execute "dbt.test" (cogitator--command-argument) #'ignore))

;;;###autoload
(defun cogitator-compile ()
  "dbt compile for the current model; success opens the compiled SQL."
  (interactive)
  (cogitator--execute "dbt.compile" (cogitator--command-argument) #'ignore))

;;;###autoload
(defun cogitator-parse ()
  "dbt parse for the whole project; success refreshes the server context."
  (interactive)
  (cogitator--execute "dbt.parse" (cogitator--command-argument) #'ignore))

;;;###autoload
(defun cogitator-set-profiles-dir (dir)
  "Switch the running server's profiles directory to DIR — no restart.
The server resets its profile/target overrides and refreshes.  For a
persistent per-project setting use dir-locals on
`cogitator-profiles-dir' instead."
  (interactive "DProfiles directory: ")
  (cogitator--execute
   "dbt.setProfilesDir" (list :dir (expand-file-name dir))
   (lambda (result)
     (if (null result)
         (message "cogitator: profiles directory not switched (see the server message)")
       (message "cogitator: profiles directory now %s" (plist-get result :dir))))))

;;;###autoload
(defun cogitator-query ()
  "Build a bounded SELECT on the relation at point in a sql-mode buffer.
The statement is ready to send through your own `sql-connection' /
SQL client; cogitator never talks to the warehouse itself."
  (interactive)
  (cogitator--execute
   "dbt.relation" (cogitator--command-argument t)
   (lambda (r)
     (if (null r)
         (message "cogitator: no relation here — not a dbt model, ref() or source()")
       (let ((statement (format "select * from %s limit %d"
                                (plist-get r :relation) cogitator-query-limit))
             (buf (get-buffer-create "*cogitator-query*")))
         (with-current-buffer buf
           (let ((inhibit-read-only t))
             (erase-buffer)
             (when (fboundp 'sql-mode)
               (unless (derived-mode-p 'sql-mode)
                 (sql-mode)))
             (insert statement "\n")))
         (pop-to-buffer buf)
         (message "cogitator: query for %s (limit %d)"
                  (plist-get r :relation) cogitator-query-limit))))))

;;;###autoload
(defun cogitator-tags ()
  "Pick a dbt tag, then jump to a model carrying it (dbt.projectGraph)."
  (interactive)
  (cogitator--execute
   "dbt.projectGraph" (cogitator--command-argument)
   (lambda (graph)
     (let ((by-tag (make-hash-table :test #'equal))
           (tags nil))
       (seq-do (lambda (node)
                 (seq-do (lambda (tag)
                           (push node (gethash tag by-tag)))
                         (plist-get node :tags)))
               (plist-get graph :nodes))
       (maphash (lambda (tag _) (push tag tags)) by-tag)
       (if (null tags)
           (message "cogitator: no tags declared in this project")
         (let* ((tag (completing-read "dbt tag: " (sort tags #'string<) nil t))
                (nodes (gethash tag by-tag))
                (name (completing-read
                       (format "model with #%s: " tag)
                       (sort (mapcar (lambda (n) (plist-get n :name)) nodes)
                             #'string<)
                       nil t))
                (node (seq-find (lambda (n) (equal (plist-get n :name) name))
                                nodes))
                (uri (and node (plist-get node :uri))))
           (if uri
               (find-file (cogitator--uri-to-path uri))
             (message "cogitator: %s has no file on disk" name))))))))

;;;###autoload
(defun cogitator-selectors ()
  "Pick a selectors.yml entry and jump to it (workspace/symbol)."
  (interactive)
  (cogitator--request
   :workspace/symbol "workspace/symbol" '(:query "selector:")
   (lambda (symbols)
     (let ((symbols (append symbols nil)))
       (if (null symbols)
           (message "cogitator: no selectors.yml entries in this project")
         (let* ((name (completing-read
                       "dbt selector: "
                       (sort (mapcar (lambda (s) (plist-get s :name)) symbols)
                             #'string<)
                       nil t))
                (symbol (seq-find (lambda (s) (equal (plist-get s :name) name))
                                  symbols)))
           (when symbol
             (cogitator--visit-location (plist-get symbol :location)))))))))

(defun cogitator--profiles-render (report)
  "Render a redacted dbt.profiles REPORT into *cogitator-profiles*.
Returns the buffer (separated from `cogitator-profiles' for testing)."
  (let ((buf (get-buffer-create "*cogitator-profiles*")))
    (with-current-buffer buf
      (let ((inhibit-read-only t))
        (erase-buffer)
        (insert (format "profiles.yml: %s\n"
                        (or (plist-get report :path) "(not found)")))
        (insert (format "active: %s/%s\n\n"
                        (or (plist-get report :activeProfile) "?")
                        (or (plist-get report :activeTarget) "?")))
        (seq-do
         (lambda (profile)
           (insert (format "%s%s\n" (plist-get profile :name)
                           (if (eq (plist-get profile :allowed) t)
                               ""
                             "  (outside the allowlist)")))
           (seq-do
            (lambda (target)
              (let ((auth (plist-get target :auth)))
                (insert (format "  %s — %s, %s%s\n"
                                (plist-get target :name)
                                (or (plist-get target :type) "?")
                                (or (plist-get auth :method) "?")
                                (let ((n (length (append (plist-get auth :issues) nil))))
                                  (if (> n 0) (format "  ⚠ %d issue(s)" n) ""))))))
            (plist-get profile :targets)))
         (plist-get report :profiles))
        (special-mode)))
    buf))

;;;###autoload
(defun cogitator-profiles ()
  "Show the redacted dbt.profiles inventory in a buffer."
  (interactive)
  (cogitator--execute
   "dbt.profiles" (cogitator--command-argument)
   (lambda (report)
     (pop-to-buffer (cogitator--profiles-render report)))))

;;;###autoload
(defun cogitator-server-doctor ()
  "The server's own health report (dbt.doctor), pretty-printed."
  (interactive)
  (cogitator--execute
   "dbt.doctor" (cogitator--command-argument)
   (lambda (report)
     (let ((buf (get-buffer-create "*cogitator-server-doctor*")))
       (with-current-buffer buf
         (let ((inhibit-read-only t))
           (erase-buffer)
           (insert (pp-to-string report))
           (goto-char (point-min)))
         (special-mode))
       (pop-to-buffer buf)))))

;; ---------------------------------------------------------------------------
;; is_incremental(): the model rendered both ways (dbt.incrementalDiff)
;; ---------------------------------------------------------------------------

(defun cogitator--incremental-diff-render (result)
  "The text of a dbt.incrementalDiff RESULT: the unified diff, or a note
and the SQL when `is_incremental()' changes nothing."
  (let ((identical (plist-get result :identical)))
    (if (and identical (not (eq identical :json-false)))
        (format "-- %s: is_incremental() changes nothing in the rendered SQL\n\n%s"
                (plist-get result :model)
                (plist-get result :fullRefresh))
      (plist-get result :diff))))

;;;###autoload
(defun cogitator-incremental-diff ()
  "This model rendered with `is_incremental()' false and true, as a diff."
  (interactive)
  (cogitator--execute
   "dbt.incrementalDiff" (cogitator--command-argument)
   (lambda (result)
     (if (or (null result) (eq result :null))
         (message "cogitator: no incremental diff for this buffer")
       (let ((buf (get-buffer-create "*cogitator-incremental-diff*")))
         (with-current-buffer buf
           (let ((inhibit-read-only t))
             (erase-buffer)
             (insert (cogitator--incremental-diff-render result))
             (goto-char (point-min)))
           (diff-mode)
           (setq buffer-read-only t)
           (local-set-key (kbd "q") #'quit-window))
         (pop-to-buffer buf))))))

;; ---------------------------------------------------------------------------
;; the rows of a CTE (dbt.previewCte)
;; ---------------------------------------------------------------------------

(defun cogitator--preview-cte-render (result)
  "The text of a dbt.previewCte RESULT: the server's markdown table."
  (let ((table (plist-get result :table)))
    (if (stringp table) table "")))

;;;###autoload
(defun cogitator-preview-cte (&optional limit)
  "The rows of the CTE at point (or the final select past every CTE).
The model runs up to that CTE through `dbt show --inline'; with a prefix
argument LIMIT, that many rows (default: cogitator.yml preview.rows)."
  (interactive "P")
  (let ((argument (cogitator--command-argument t)))
    (when limit
      (setq argument (plist-put argument :limit (prefix-numeric-value limit))))
    (cogitator--execute
     "dbt.previewCte" argument
     (lambda (result)
       (let ((buf (get-buffer-create "*cogitator-preview*")))
         (with-current-buffer buf
           (let ((inhibit-read-only t))
             (erase-buffer)
             (insert (cogitator--preview-cte-render result))
             (goto-char (point-min)))
           (special-mode))
         (pop-to-buffer buf))))))

;; ---------------------------------------------------------------------------
;; incrementality: the split plan and the project-wide list
;; (specs/ir-pipeline/10 §6)
;; ---------------------------------------------------------------------------

(defun cogitator--split-render (plan)
  "The text of a dbt.splitPlan PLAN: its S-expression, then the new model."
  (let ((part (plist-get plan :part)))
    (concat (plist-get plan :sexp) "\n"
            (when (and part (not (eq part :null)))
              (format "\n;; %s.sql — press a to split, q to close\n%s"
                      (plist-get part :name)
                      (string-trim-right (plist-get part :text)))))))

(declare-function eglot-code-actions "eglot")

(defun cogitator--apply-split (source)
  "Offer the split code action of the model in buffer SOURCE."
  (pop-to-buffer source)
  (if (and (featurep 'eglot) (eglot-managed-p))
      (eglot-code-actions (point-min) (point-min) "refactor.extract" t)
    (message "cogitator: run the code action \"Split …\" on this model")))

;;;###autoload
(defun cogitator-split-plan ()
  "This table model split at its incrementality frontier (dbt.splitPlan).
Shows the plan as an S-expression and the new model's file; \\`a' applies it."
  (interactive)
  (let ((source (current-buffer)))
    (cogitator--execute
     "dbt.splitPlan" (cogitator--command-argument)
     (lambda (plan)
       (if (or (null plan) (eq plan :null))
           (message "cogitator: nothing to split (not a table, or nothing keeps it one)")
         (let ((buf (get-buffer-create "*cogitator-split*")))
           (with-current-buffer buf
             (let ((inhibit-read-only t))
               (erase-buffer)
               (insert (cogitator--split-render plan))
               (goto-char (point-min)))
             (lisp-data-mode)
             (setq buffer-read-only t)
             (local-set-key (kbd "q") #'quit-window)
             (let ((part (plist-get plan :part)))
               (when (and part (not (eq part :null)))
                 (local-set-key (kbd "a")
                                (lambda ()
                                  (interactive)
                                  (quit-window)
                                  (cogitator--apply-split source))))))
           (pop-to-buffer buf)))))))

(defun cogitator--incrementality-choices (report)
  "((LABEL . ROW) …) for a dbt.incrementality REPORT, in its order."
  (mapcar
   (lambda (row)
     (let* ((view (plist-get row :view))
            (strategy (plist-get view :strategy))
            (seconds (plist-get row :lastRunSeconds))
            (reasons (append (plist-get view :reasons) nil))
            (summary (plist-get view :summary))
            (why (cond ((stringp summary) summary)
                       (reasons (mapconcat (lambda (r) (plist-get r :reason)) reasons "; ")))))
       (cons (concat (plist-get row :model) " — " (plist-get view :verdict)
                     (when (stringp strategy) (format " (%s)" strategy))
                     (when (numberp seconds) (format " [last run %.1fs]" seconds))
                     (when why (concat ": " why)))
             row)))
   (append (plist-get report :models) nil)))

;;;###autoload
(defun cogitator-incrementality ()
  "Pick a table model by whether it could be incremental, and visit it."
  (interactive)
  (cogitator--execute
   "dbt.incrementality" (cogitator--command-argument)
   (lambda (report)
     (let ((choices (cogitator--incrementality-choices report)))
       (if (null choices)
           (message "cogitator: no table models in this project")
         (let* ((pick (completing-read
                       (format "table models (%s could be incremental): "
                               (plist-get report :incremental))
                       (mapcar #'car choices) nil t))
                (row (cdr (assoc pick choices))))
           (when row
             (find-file (cogitator--uri-to-path (plist-get row :uri))))))))))

;; ---------------------------------------------------------------------------
;; syntax tree (dbt.syntaxTree): the open document's tree-sitter-dbt nodes
;; ---------------------------------------------------------------------------

(defun cogitator--syntax-line (node)
  "One tree line for NODE, as :DbtSyntaxTree draws it.
Indentation by depth, the field, the kind and its 1-based line range."
  (let* ((range (plist-get node :range))
         (start (plist-get range :start))
         (end (plist-get range :end))
         (field (plist-get node :field))
         (kind (plist-get node :kind)))
    (format "%s%s%s [%d:%d - %d:%d]"
            (make-string (* 2 (plist-get node :depth)) ?\s)
            (if (stringp field) (concat field ": ") "")
            (if (eq (plist-get node :missing) t) (concat "MISSING " kind) kind)
            (1+ (plist-get start :line)) (plist-get start :character)
            (1+ (plist-get end :line)) (plist-get end :character))))

(defun cogitator--syntax-render (tree)
  "The text of a dbt.syntaxTree TREE: a line per node, and a note when cut."
  (concat (mapconcat #'cogitator--syntax-line (append (plist-get tree :nodes) nil) "\n")
          "\n"
          (when (eq (plist-get tree :truncated) t)
            "… more nodes than the server sends; the rest are left out\n")))

(defvar-local cogitator--syntax-source nil
  "The source buffer of a *cogitator-syntax* buffer.")
(defvar-local cogitator--syntax-nodes nil
  "The nodes of a *cogitator-syntax* buffer, one per line.")

(defun cogitator-syntax-jump ()
  "Jump to the source range of the node on this tree line."
  (interactive)
  (let ((node (nth (1- (line-number-at-pos)) cogitator--syntax-nodes))
        (source cogitator--syntax-source))
    (when (and node (buffer-live-p source))
      (let ((start (plist-get (plist-get node :range) :start)))
        (pop-to-buffer source)
        (goto-char (point-min))
        (forward-line (plist-get start :line))
        (forward-char (min (plist-get start :character)
                           (- (line-end-position) (point))))))))

;;;###autoload
(defun cogitator-syntax-tree ()
  "The tree-sitter-dbt tree of this model (dbt.syntaxTree) in a buffer.
RET jumps to a node's place in the model, q closes."
  (interactive)
  (let ((source (current-buffer)))
    (cogitator--execute
     "dbt.syntaxTree" (cogitator--command-argument)
     (lambda (tree)
       (let ((buf (get-buffer-create "*cogitator-syntax*")))
         (with-current-buffer buf
           (let ((inhibit-read-only t))
             (erase-buffer)
             (insert (cogitator--syntax-render tree))
             (goto-char (point-min)))
           (special-mode)
           (setq cogitator--syntax-source source
                 cogitator--syntax-nodes (append (plist-get tree :nodes) nil))
           (local-set-key (kbd "RET") #'cogitator-syntax-jump))
         (pop-to-buffer buf))))))

;; ---------------------------------------------------------------------------
;; highlighting from the server (dbt.highlight, specs/highlighting)
;; ---------------------------------------------------------------------------

(defcustom cogitator-highlight-on-open t
  "Colour the project's SQL and YAML buffers from the server when opened.
The server highlights with tree-sitter-dbt (the Jinja), tree-sitter-sequel
(the SQL between the tags) and tree-sitter-yaml, plus dbt-aware captures
in YAML (models, columns, tests, docs).  Turns on
`cogitator-highlight-mode' in buffers cogitator manages."
  :type 'boolean)

(defconst cogitator--highlight-faces
  '(("comment" . font-lock-comment-face)
    ("string" . font-lock-string-face)
    ("string.special" . font-lock-doc-face)
    ("number" . font-lock-constant-face)
    ("boolean" . font-lock-constant-face)
    ("constant" . font-lock-constant-face)
    ("keyword" . font-lock-keyword-face)
    ("operator" . font-lock-keyword-face)
    ("function" . font-lock-function-name-face)
    ("function.call" . font-lock-function-call-face)
    ("function.dbt.test" . font-lock-function-call-face)
    ("type" . font-lock-type-face)
    ("module" . font-lock-constant-face)
    ("variable.parameter" . font-lock-variable-name-face)
    ("variable" . font-lock-variable-name-face)
    ("field" . font-lock-property-name-face)
    ("property" . font-lock-property-name-face)
    ("property.dbt.column" . font-lock-property-name-face)
    ("label" . font-lock-constant-face)
    ("attribute" . font-lock-preprocessor-face)
    ("tag" . font-lock-preprocessor-face)
    ("punctuation" . font-lock-punctuation-face))
  "Capture → face, by the longest matching prefix of the capture's name.
A capture ending in `.unresolved' (a reference the project lacks) takes
`font-lock-warning-face'.")

(defun cogitator--highlight-face (capture)
  "The face of a tree-sitter CAPTURE name (\"keyword.conditional\"), or nil."
  (if (string-suffix-p ".unresolved" capture)
      'font-lock-warning-face
    (let ((best nil) (best-len -1))
      (dolist (entry cogitator--highlight-faces)
        (let ((prefix (car entry)))
          (when (and (> (length prefix) best-len)
                     (or (string= capture prefix)
                         (string-prefix-p (concat prefix ".") capture)))
            (setq best (cdr entry) best-len (length prefix)))))
      (and best (facep best) best))))

(defconst cogitator--highlight-priorities
  '(("yaml" . 10) ("sql" . 11) ("jinja" . 12) ("dbt" . 13))
  "Overlay priority by layer: a later layer wins where two meet.")

(defun cogitator--highlight-language (file)
  "The grammar FILE is coloured with: yaml, sql (compiled) or dbt."
  (cond ((null file) "dbt")
        ((string-match-p "\\.ya?ml\\'" file) "yaml")
        ((string-match-p "/target/\\(compiled\\|run\\)/" file) "sql")
        (t "dbt")))

(defun cogitator--utf16-point (line character)
  "The buffer position of LSP LINE (0-based) and UTF-16 CHARACTER."
  (save-excursion
    (goto-char (point-min))
    (forward-line line)
    (let ((units 0) (limit (line-end-position)))
      (while (and (< units character) (< (point) limit))
        (setq units (+ units (if (> (char-after) #xFFFF) 2 1)))
        (forward-char 1))
      (point))))

(defun cogitator--highlight-apply (result &optional skip)
  "Paint dbt.highlight's RESULT into the current buffer as overlays.
Spans of a layer in SKIP (a list of layer names) are left out.
Returns how many spans were painted."
  (remove-overlays (point-min) (point-max) 'cogitator-highlight t)
  (let ((count 0))
    (seq-doseq (span (plist-get result :spans))
      (let* ((range (plist-get span :range))
             (start (plist-get range :start))
             (end (plist-get range :end))
             (layer (or (plist-get span :layer) "jinja"))
             (face (cogitator--highlight-face (plist-get span :capture))))
        (when (and face (not (member layer skip)))
          (let ((ov (make-overlay
                     (cogitator--utf16-point (plist-get start :line) (plist-get start :character))
                     (cogitator--utf16-point (plist-get end :line) (plist-get end :character)))))
            (overlay-put ov 'cogitator-highlight t)
            (overlay-put ov 'face face)
            (overlay-put ov 'priority (or (cdr (assoc layer cogitator--highlight-priorities)) 10))
            (setq count (1+ count))))))
    count))

(defvar cogitator-highlight-mode) ; defined below by `define-minor-mode'

(defvar-local cogitator--highlight-tick nil
  "The `buffer-chars-modified-tick' the last highlight request was for.")
(defvar-local cogitator--highlight-timer nil
  "The idle timer of the next repaint.")

(defun cogitator--highlight-request (&optional quiet)
  "Ask the server to colour this buffer; answers for an older text are dropped.
With QUIET, say nothing when it is painted."
  (let* ((buffer (current-buffer))
         (tick (buffer-chars-modified-tick))
         (language (cogitator--highlight-language (buffer-file-name)))
         ;; YAML keeps the major mode's own colours when it has them
         (skip (and (equal language "yaml") font-lock-mode '("yaml"))))
    (setq cogitator--highlight-tick tick)
    (cogitator--execute
     "dbt.highlight"
     (list :text (buffer-substring-no-properties (point-min) (point-max))
           :language language :version tick)
     (lambda (result)
       (when (buffer-live-p buffer)
         (with-current-buffer buffer
           (when (equal cogitator--highlight-tick tick)
             (let ((count (cogitator--highlight-apply result skip)))
               (unless quiet
                 (message "cogitator: %d spans coloured by the server" count))))))))))

(defun cogitator--highlight-changed (&rest _)
  "Repaint this buffer after an idle moment."
  (when (timerp cogitator--highlight-timer)
    (cancel-timer cogitator--highlight-timer))
  (let ((buffer (current-buffer)))
    (setq cogitator--highlight-timer
          (run-with-idle-timer
           0.3 nil
           (lambda ()
             (when (buffer-live-p buffer)
               (with-current-buffer buffer
                 (when cogitator-highlight-mode
                   (cogitator--highlight-request t)))))))))

;;;###autoload
(define-minor-mode cogitator-highlight-mode
  "Colour this buffer from the server (dbt.highlight), and keep up with edits."
  :lighter " dbt-hl"
  (if cogitator-highlight-mode
      (progn
        (add-hook 'after-change-functions #'cogitator--highlight-changed nil t)
        (cogitator--highlight-request t))
    (remove-hook 'after-change-functions #'cogitator--highlight-changed t)
    (when (timerp cogitator--highlight-timer)
      (cancel-timer cogitator--highlight-timer))
    (setq cogitator--highlight-tick nil)
    (remove-overlays (point-min) (point-max) 'cogitator-highlight t)))

(defun cogitator--highlight-on-open ()
  "Turn on `cogitator-highlight-mode' in a project's SQL or YAML buffer
cogitator manages; off again when it stops managing it."
  (let ((managed (or (and (fboundp 'eglot-managed-p) (eglot-managed-p))
                     (bound-and-true-p lsp-mode))))
    (cond
     ((not managed)
      (when cogitator-highlight-mode (cogitator-highlight-mode -1)))
     ((and cogitator-highlight-on-open
           (buffer-file-name)
           (string-match-p "\\.\\(sql\\|ya?ml\\)\\'" (buffer-file-name))
           (locate-dominating-file (buffer-file-name) "dbt_project.yml"))
      (cogitator-highlight-mode 1)))))

;;;###autoload
(defun cogitator-highlight-buffer (&optional clear)
  "Colour this buffer once, as the server highlights it.
The Jinja by tree-sitter-dbt, the SQL between the tags by the SQL grammar
it injects, YAML by tree-sitter-yaml with dbt's own captures; compiled SQL
\(target/compiled) as plain SQL.  With a prefix argument CLEAR, take the
colours off.  `cogitator-highlight-mode' keeps them up to date."
  (interactive "P")
  (if clear
      (progn (setq cogitator--highlight-tick nil)
             (remove-overlays (point-min) (point-max) 'cogitator-highlight t))
    (cogitator--highlight-request)))

;; ---------------------------------------------------------------------------
;; node info (dbt.nodeInfo): macros, tests, sources…
;; ---------------------------------------------------------------------------

(defconst cogitator--node-kinds
  '("model" "macro" "test" "generic_test" "source" "exposure" "metric" "semantic_model" "analysis")
  "The kinds `cogitator-node-info' tries, in order, when none is given.")

(defun cogitator--node-parse (text)
  "TEXT as `[kind:]name' → (KIND . NAME), KIND nil when not given."
  (let ((text (string-trim (or text ""))))
    (if (string-match "\\`\\([a-z_]+\\):\\(.+\\)\\'" text)
        (cons (match-string 1 text) (string-trim (match-string 2 text)))
      (cons nil text))))

(defun cogitator--node-signature (name params)
  "A macro's or generic test's signature: NAME(PARAMS)."
  (format "%s(%s)" name
          (mapconcat (lambda (p)
                       (if (stringp p) p
                         (concat (plist-get p :name)
                                 (let ((d (plist-get p :default)))
                                   (if (stringp d) (concat "=" d) "")))))
                     (append params nil) ", ")))

(defun cogitator--node-render (info)
  "The text of a dbt.nodeInfo INFO card."
  (let* ((kind (or (plist-get info :kind) "model"))
         (name (or (plist-get info :name) (plist-get info :model)))
         (lines (list (format "%s  [%s]" name kind)))
         (entries (lambda (title list)
                    (push "" lines)
                    (push (format "%s (%d)" title (length list)) lines)
                    (seq-doseq (e list)
                      (push (format "  %s  [%s]" (plist-get e :name) (plist-get e :kind)) lines)))))
    (let ((uri (plist-get info :uri)))
      (when (stringp uri) (push (cogitator--uri-to-file uri) lines)))
    (let ((desc (plist-get info :description)))
      (when (and (stringp desc) (not (string-empty-p desc)))
        (push "" lines) (push desc lines)))
    (pcase kind
      ("macro"
       (push "" lines)
       (push (cogitator--node-signature name (plist-get info :params)) lines)
       (funcall entries "called by" (plist-get info :callers))
       (funcall entries "calls" (plist-get info :calls)))
      ("generic_test"
       (push "" lines)
       (push (format "{%% test %s %%}" (cogitator--node-signature name (plist-get info :params))) lines)
       (push "" lines)
       (push (format "applied to (%d)" (length (plist-get info :appliedTo))) lines)
       (seq-doseq (a (plist-get info :appliedTo))
         (push (format "  %s%s" (plist-get a :model)
                       (let ((c (plist-get a :column))) (if (stringp c) (concat "." c) "")))
               lines)))
      (_
       (funcall entries (if (equal kind "test") "tests" "upstream")
                (or (plist-get info :refs) (plist-get info :upstream)))
       (unless (member kind '("test" "analysis"))
         (funcall entries "downstream" (plist-get info :downstream)))))
    (concat (string-join (nreverse lines) "\n") "\n")))

;;;###autoload
(defun cogitator-node-info (query)
  "Show a node's card (dbt.nodeInfo): QUERY is `[kind:]name'.
A macro: its signature, callers and calls; a test: what it tests; a
generic test: where YAML applies it.  Without a kind the kinds are tried
in order."
  (interactive (list (read-string "node ([kind:]name): ")))
  (let* ((parsed (cogitator--node-parse query))
         (kinds (if (car parsed) (list (car parsed)) cogitator--node-kinds))
         (name (cdr parsed)))
    (when (string-empty-p name)
      (user-error "cogitator: give a node, e.g. macro:cents_to_dollars"))
    (cl-labels
        ((try (rest)
           (if (null rest)
               (message "cogitator: no node named %s" name)
             (cogitator--execute
              "dbt.nodeInfo"
              (list :node (list :kind (car rest) :name name) :quiet (if (cdr kinds) t :json-false))
              (lambda (info)
                (if (or (null info) (eq info :null))
                    (try (cdr rest))
                  (let ((buf (get-buffer-create "*cogitator-node*")))
                    (with-current-buffer buf
                      (let ((inhibit-read-only t))
                        (erase-buffer)
                        (insert (cogitator--node-render info))
                        (goto-char (point-min)))
                      (special-mode))
                    (pop-to-buffer buf))))))))
      (try kinds))))

;; ---------------------------------------------------------------------------
;; ast-grep through the server (dbt.astGrep)
;; ---------------------------------------------------------------------------

(defconst cogitator-ast-grep-default "{{ ref($M) }}"
  "The pattern `cogitator-ast-grep' offers first.")

(defun cogitator--ast-grep-parse (text)
  "TEXT as `[sql:]pattern [=> rewrite]' → (:pattern :rewrite :language), or nil."
  (let* ((text (string-trim (or text "")))
         (language "dbt"))
    (cond ((string-match "\\`sql:[ \t]*" text)
           (setq language "sql" text (substring text (match-end 0))))
          ((string-match "\\`dbt:[ \t]*" text)
           (setq text (substring text (match-end 0)))))
    (let* ((arrow (string-match "[ \t]*=>[ \t]*" text))
           (pattern (string-trim (if arrow (substring text 0 arrow) text)))
           (rewrite (and arrow (string-trim (substring text (match-end 0))))))
      (unless (string-empty-p pattern)
        (list :pattern pattern :rewrite rewrite :language language)))))

(defun cogitator--ast-grep-line (match)
  "One grep-style line for a dbt.astGrep MATCH: file:line:column: text."
  (let* ((start (plist-get (plist-get match :range) :start))
         (text (replace-regexp-in-string "[ \t]*\n[ \t]*" " " (plist-get match :text)))
         (replacement (plist-get match :replacement)))
    (format "%s:%d:%d: %s%s"
            (cogitator--uri-to-file (plist-get match :uri))
            (1+ (plist-get start :line))
            (1+ (plist-get start :character))
            text
            (if (stringp replacement) (concat "  →  " replacement) ""))))

(defun cogitator--uri-to-file (uri)
  "The file name of a file:// URI."
  (url-unhex-string (replace-regexp-in-string "\\`file://" "" uri)))

;;;###autoload
(defun cogitator-ast-grep (query)
  "Structural search with ast-grep (dbt.astGrep) over the project.
QUERY is `[sql:]pattern [=> rewrite]': a pattern over tree-sitter-dbt
\(Jinja; `$M' a metavariable), or prefixed `sql:' a whole SQL statement
over the compiled SQL.  Matches land in a grep buffer; with a rewrite,
it is applied to every match after asking."
  (interactive (list (read-string "ast-grep ([sql:]pattern [=> rewrite]): "
                                  cogitator-ast-grep-default)))
  (let ((parsed (cogitator--ast-grep-parse query)))
    (unless parsed
      (user-error "cogitator: give a pattern, e.g. %s" cogitator-ast-grep-default))
    (let ((argument (list :uri "" :pattern (plist-get parsed :pattern)
                          :language (plist-get parsed :language))))
      (when (plist-get parsed :rewrite)
        (setq argument (append argument (list :rewrite (plist-get parsed :rewrite)))))
      (cogitator--execute
       "dbt.astGrep" argument
       (lambda (result)
         (let ((matches (append (plist-get result :matches) nil))
               (buf (get-buffer-create "*cogitator-ast-grep*")))
           (with-current-buffer buf
             (let ((inhibit-read-only t))
               (erase-buffer)
               (insert (format "-*- ast-grep %s: %d matches -*-\n\n"
                               (plist-get parsed :pattern) (length matches)))
               (insert (mapconcat #'cogitator--ast-grep-line matches "\n") "\n"))
             (require 'grep)
             (grep-mode))
           (display-buffer buf)
           (when (and (plist-get parsed :rewrite) matches
                      (y-or-n-p (format "Rewrite %d matches into %s? "
                                        (length matches) (plist-get parsed :rewrite))))
             (cogitator--execute
              "dbt.astGrep" (append argument (list :apply t))
              (lambda (_) (message "cogitator: rewrote %d matches" (length matches)))))))))))

;; node selection (dbt.select): dbt's --select syntax, as `dbt ls' answers it
;; ---------------------------------------------------------------------------

(defun cogitator--select-line (node)
  "One completion line for a dbt.select NODE: its name, kind and tags."
  (let ((version (plist-get node :version))
        (graph (plist-get node :graphId))
        (tags (append (plist-get node :tags) nil)))
    (format "%s  [%s]%s"
            (cond ((and (stringp version) (not (string-empty-p version)))
                   (format "%s.v%s" (plist-get node :name) version))
                  ((and (equal (plist-get node :resourceType) "source") (stringp graph))
                   (format "%s.%s" (string-remove-prefix "source:" graph) (plist-get node :name)))
                  (t (plist-get node :name)))
            (replace-regexp-in-string "_" " " (plist-get node :resourceType))
            (if tags (concat "  #" (mapconcat #'identity tags " #")) ""))))

(defvar cogitator-select-history nil "Selections typed into `cogitator-select'.")

;;;###autoload
(defun cogitator-select (selection)
  "Nodes a dbt SELECTION picks (dbt.select), as `dbt ls --select' would.
SELECTION is dbt's syntax: tag:nightly, +dim_items, @m, a,b (both),
a b (either), stg_*, config.materialized:incremental, and an inline
`--exclude resource_type:test'.  Visit the node you choose."
  (interactive (list (read-string "dbt select: " nil 'cogitator-select-history)))
  (cogitator--execute
   "dbt.select" (append (cogitator--command-argument) (list :select selection))
   (lambda (result)
     (let* ((err (plist-get result :error))
            (nodes (append (plist-get result :nodes) nil))
            (choices (mapcar (lambda (n) (cons (cogitator--select-line n) n)) nodes)))
       (cond
        ((and err (not (eq err :null)))
         (message "cogitator: %s (at %d)" (plist-get err :message) (plist-get err :start)))
        ((null choices) (message "cogitator: %s selects nothing" selection))
        (t
         (let* ((pick (completing-read (format "%s — %d nodes: " selection (length choices))
                                       (mapcar #'car choices) nil t))
                (node (cdr (assoc pick choices)))
                (uri (plist-get node :uri))
                (range (plist-get node :range)))
           (when (stringp uri)
             (find-file (cogitator--uri-to-path uri))
             (when (and range (not (eq range :null)))
               (goto-char (point-min))
               (forward-line (plist-get (plist-get range :start) :line)))))))))))

;; unresolved names (dbt.unresolved)
;; ---------------------------------------------------------------------------

(defun cogitator--unresolved-line (item)
  "One grep-style line for a dbt.unresolved ITEM.
It reads file:line:column: [kind] name — message."
  (let ((start (plist-get (plist-get item :range) :start)))
    (format "%s:%d:%d: [%s] %s — %s"
            (cogitator--uri-to-file (plist-get item :uri))
            (1+ (plist-get start :line))
            (1+ (plist-get start :character))
            (plist-get item :kind)
            (plist-get item :name)
            (or (plist-get item :message) ""))))

;; ---------------------------------------------------------------------------
;; schema-YAML sync (dbt.syncYaml)
;; ---------------------------------------------------------------------------

(defcustom cogitator-sync-yaml-prune nil
  "Whether `cogitator-sync-yaml' removes columns a model no longer produces."
  :type 'boolean
  :group 'cogitator)

(defcustom cogitator-sync-yaml-inherit-types nil
  "Whether `cogitator-sync-yaml' fills missing data_type from proven types."
  :type 'boolean
  :group 'cogitator)

(defun cogitator--sync-yaml-from (from)
  "The column FROM of an inherited description, as text."
  (if (equal (plist-get from :kind) "source")
      (format "source %s.%s.%s" (plist-get from :source) (plist-get from :table)
              (plist-get from :column))
    (format "%s.%s" (plist-get from :name) (plist-get from :column))))

(defun cogitator--sync-yaml-lines (result)
  "The preview lines of a dbt.syncYaml RESULT: a line per change, stale
column and skip of each model, then every file's unified diff."
  (let ((lines (list (format "# dbt.syncYaml: %d change(s) in %d file(s)"
                             (or (plist-get result :changes) 0)
                             (length (plist-get result :files))))))
    (dolist (model (append (plist-get result :models) nil))
      (let (body
            (pruned (append (plist-get model :pruned) nil)))
        (when (eq (plist-get model :createdEntry) t)
          (push "  + entry" body))
        (dolist (column (append (plist-get model :added) nil))
          (push (concat "  + column " column) body))
        (dolist (inherited (append (plist-get model :inherited) nil))
          (push (format "  ~ %s description from %s" (plist-get inherited :column)
                        (cogitator--sync-yaml-from (plist-get inherited :from)))
                body))
        (dolist (typed (append (plist-get model :typed) nil))
          (push (format "  ~ %s data_type %s" (plist-get typed :column)
                        (plist-get typed :dataType))
                body))
        (dolist (column pruned)
          (push (concat "  - column " column) body))
        (when (eq (plist-get model :reordered) t)
          (push "  ~ columns reordered" body))
        (dolist (column (append (plist-get model :stale) nil))
          (unless (member column pruned)
            (push (format "  ! stale column %s (prune removes it)" column) body)))
        (dolist (skip (append (plist-get model :skipped) nil))
          (let ((column (plist-get skip :column)))
            (push (format "  ? %s%s" (if (stringp column) (concat column ": ") "")
                          (plist-get skip :reason))
                  body)))
        (when body
          (setq lines (append lines (list (concat "# " (plist-get model :model)))
                              (nreverse body))))))
    (dolist (file (append (plist-get result :files) nil))
      (setq lines (append lines (list "")
                          (split-string (string-trim-right (or (plist-get file :diff) "") "\n")
                                        "\n"))))
    lines))

(defun cogitator--sync-yaml-argument (this-buffer)
  "The dbt.syncYaml argument: THIS-BUFFER narrows to its model or entries."
  (append (if this-buffer (cogitator--command-argument) (list :uri ""))
          (when cogitator-sync-yaml-prune (list :prune t))
          (when cogitator-sync-yaml-inherit-types (list :inheritTypes t))))

;;;###autoload
(defun cogitator-sync-yaml (&optional this-buffer)
  "Bring the models' schema YAML in line with their SQL (dbt.syncYaml).
Missing entries and columns are added and descriptions inherited down
the column lineage.  The plan opens in a buffer; it is applied once you
confirm.  With a prefix argument THIS-BUFFER, only the model of this
buffer (or the models of this properties file).  See
`cogitator-sync-yaml-prune' and `cogitator-sync-yaml-inherit-types'."
  (interactive "P")
  (let ((argument (cogitator--sync-yaml-argument this-buffer)))
    (cogitator--execute
     "dbt.syncYaml" argument
     (lambda (result)
       (let ((changes (or (plist-get result :changes) 0)))
         (if (zerop changes)
             (message "cogitator: schema YAML is in sync — nothing to change")
           (let ((buf (get-buffer-create "*cogitator-sync-yaml*")))
             (with-current-buffer buf
               (let ((inhibit-read-only t))
                 (erase-buffer)
                 (insert (mapconcat #'identity (cogitator--sync-yaml-lines result) "\n") "\n")
                 (goto-char (point-min)))
               (diff-mode)
               (setq buffer-read-only t)
               (local-set-key (kbd "q") #'quit-window))
             (display-buffer buf)
             (when (yes-or-no-p (format "Apply %d change(s) to %d file(s)? " changes
                                        (length (plist-get result :files))))
               (cogitator--execute
                "dbt.syncYaml" (append argument (list :apply t))
                (lambda (_) (message "cogitator: schema YAML synced")))))))))))

;; ---------------------------------------------------------------------------
;; a column renamed up- and downstream (dbt.renameColumn)
;; ---------------------------------------------------------------------------

(defun cogitator--rename-column-lines (result)
  "The preview lines of a dbt.renameColumn RESULT: what is renamed from
where and why there, then the edits of each file."
  (let* ((origin (plist-get result :origin))
         (lines (list (format "# dbt.renameColumn: %s.%s → %s (%s)"
                              (plist-get result :model) (plist-get result :column)
                              (plist-get result :newName) (plist-get result :direction))
                      (format "# from %s.%s: %s" (plist-get origin :model)
                              (plist-get origin :column) (plist-get origin :stop))
                      (concat "# models: " (mapconcat #'identity
                                                      (append (plist-get result :models) nil)
                                                      ", ")))))
    (dolist (change (append (plist-get (plist-get result :edit) :documentChanges) nil))
      (let ((uri (plist-get (plist-get change :textDocument) :uri))
            (edits (append (plist-get change :edits) nil)))
        (setq lines (append lines (list (format "%s: %d edit(s)" uri (length edits)))))
        (dolist (edit edits)
          (let ((start (plist-get (plist-get edit :range) :start)))
            (setq lines (append lines (list (format "  %d:%d %s"
                                                    (1+ (plist-get start :line))
                                                    (1+ (plist-get start :character))
                                                    (plist-get edit :newText)))))))))
    lines))

;;;###autoload
(defun cogitator-rename-column (new-name &optional downstream-only)
  "Rename the model column at point to NEW-NAME up- and downstream
\(dbt.renameColumn with direction \"both\"): from the model its lineage
says defines it, through every model that passes it on.  With a prefix
argument DOWNSTREAM-ONLY, only this model and its consumers, as
`eglot-rename' does.  The edits are previewed and applied on confirm."
  (interactive
   (list (read-string (format "Rename column %s to: " (or (thing-at-point 'symbol t) "")))
         current-prefix-arg))
  (let ((argument (append (cogitator--command-argument t)
                          (list :newName new-name
                                :direction (if downstream-only "down" "both")))))
    (cogitator--execute
     "dbt.renameColumn" argument
     (lambda (result)
       (if (not (plist-get result :edit))
           (message "cogitator: no model column at point")
         (let ((buf (get-buffer-create "*cogitator-rename-column*")))
           (with-current-buffer buf
             (let ((inhibit-read-only t))
               (erase-buffer)
               (insert (mapconcat #'identity (cogitator--rename-column-lines result) "\n") "\n")
               (goto-char (point-min)))
             (setq buffer-read-only t)
             (local-set-key (kbd "q") #'quit-window))
           (display-buffer buf)
           (when (yes-or-no-p (format "Rename %s.%s to %s? " (plist-get result :model)
                                      (plist-get result :column) new-name))
             (cogitator--execute
              "dbt.renameColumn" (append argument (list :apply t))
              (lambda (_) (message "cogitator: column renamed — save the buffers"))))))))))

;; ---------------------------------------------------------------------------

;;;###autoload
(defun cogitator-unresolved (&optional kinds)
  "Every unresolved ref, source, doc, macro and var (dbt.unresolved).
With a prefix argument, read KINDS (space separated: ref source
source_table doc macro var) to list only those.  The names land in a
grep buffer, one per line, at their place."
  (interactive
   (list (when current-prefix-arg
           (split-string (read-string "kinds (ref source source_table doc macro var): ")))))
  (let ((argument (append (list :uri "")
                          (when kinds (list :kinds (vconcat kinds))))))
    (cogitator--execute
     "dbt.unresolved" argument
     (lambda (result)
       (let ((items (append (plist-get result :items) nil))
             (buf (get-buffer-create "*cogitator-unresolved*")))
         (with-current-buffer buf
           (let ((inhibit-read-only t))
             (erase-buffer)
             (insert (format "-*- dbt unresolved: %d -*-\n\n" (plist-get result :count)))
             (insert (mapconcat #'cogitator--unresolved-line items "\n") "\n"))
           (require 'grep)
           (grep-mode))
         (display-buffer buf))))))

;; a model's dependencies (dbt.modelDependencies)
;; ---------------------------------------------------------------------------

(defun cogitator--dependency-line (node)
  "One grep-style line for a dbt.modelDependencies NODE: where it is,
what it is, how it is built and how many hops away."
  (let* ((uri (plist-get node :uri))
         (config (plist-get node :config))
         (built (seq-filter #'stringp (list (plist-get config :materialized)
                                            (plist-get config :incrementalStrategy)))))
    (format "%s:%d:1: [%s] %s%s%s"
            (if (and (stringp uri) (not (string-empty-p uri))) (cogitator--uri-to-file uri) "")
            (1+ (or (plist-get node :line) 0))
            (plist-get node :kind)
            (plist-get node :name)
            (if built (concat " — " (string-join built ", ")) "")
            (let ((depth (or (plist-get node :depth) 1)))
              (if (> depth 1) (format " (%d hops)" depth) "")))))

;;;###autoload
(defun cogitator-model-dependencies ()
  "This model and its neighbours (dbt.modelDependencies): what it uses
directly, what uses it directly, and everything upstream and downstream,
nearest first, in a grep buffer."
  (interactive)
  (cogitator--execute
   "dbt.modelDependencies" (cogitator--command-argument)
   (lambda (result)
     (if (not result)
         (message "cogitator: no model here")
       (let ((buf (get-buffer-create "*cogitator-dependencies*")))
         (with-current-buffer buf
           (let ((inhibit-read-only t))
             (erase-buffer)
             (insert (format "-*- dbt dependencies of %s -*-\n"
                             (plist-get (plist-get result :model) :name)))
             (dolist (section '((:parents . "Uses directly")
                                (:children . "Used directly by")
                                (:upstream . "Everything upstream")
                                (:downstream . "Everything downstream")))
               (let ((nodes (append (plist-get result (car section)) nil)))
                 (insert (format "\n; %s: %d\n" (cdr section) (length nodes)))
                 (dolist (node nodes)
                   (insert (cogitator--dependency-line node) "\n")))))
           (require 'grep)
           (grep-mode))
         (display-buffer buf))))))

;; breaking changes (dbt.breakingChanges)
;; ---------------------------------------------------------------------------

(defun cogitator--breaking-lines (result)
  "The lines of a dbt.breakingChanges RESULT: one per removed or retyped
column, then one per downstream column that reads it."
  (let ((renamed (make-hash-table :test #'equal))
        lines)
    (dolist (removed (append (plist-get (plist-get result :diff) :removed) nil))
      (when (stringp (plist-get removed :possiblyRenamedTo))
        (puthash (plist-get removed :name) (plist-get removed :possiblyRenamedTo) renamed)))
    (dolist (impact (append (plist-get result :impacts) nil))
      (let* ((column (plist-get impact :column))
             (to (gethash column renamed))
             (tests (append (plist-get impact :tests) nil)))
        (push (concat (format "[%s] %s %s" (plist-get impact :severity) column
                              (plist-get impact :change))
                      (if to (format " (possibly renamed to %s)" to) "")
                      (if tests (concat " — tested: " (string-join tests ", ")) ""))
              lines)
        (dolist (use (append (plist-get impact :downstream) nil))
          (push (format "  read by %s.%s%s" (plist-get use :model) (plist-get use :column)
                        (if (eq (plist-get use :contracted) t) " (contracted)" ""))
                lines))))
    (nreverse lines)))

;;;###autoload
(defun cogitator-breaking-changes ()
  "This model against its last built schema (dbt.breakingChanges).
Compares the columns the SQL produces with target/catalog.json (else
the documented columns of target/manifest.json) and lists the removed,
possibly renamed and retyped columns and what reads them downstream."
  (interactive)
  (cogitator--execute
   "dbt.breakingChanges" (cogitator--command-argument)
   (lambda (result)
     (let ((buf (get-buffer-create "*cogitator-breaking-changes*")))
       (with-current-buffer buf
         (let ((inhibit-read-only t))
           (erase-buffer)
           (insert (format "-*- dbt breaking changes: %s -*-\n\n" (plist-get result :summary)))
           (insert (mapconcat #'identity (cogitator--breaking-lines result) "\n") "\n"))
         (special-mode))
       (display-buffer buf)))))

;;;###autoload
(defun cogitator-doctor ()
  "Health report: binary, project root, profiles candidates, client state.
Emacs has no :checkhealth, so this is a plain report buffer."
  (interactive)
  (let* ((binary (car cogitator-server-command))
         (found (executable-find binary))
         (version (and found (ignore-errors
                               (car (process-lines found "--version")))))
         (root (cogitator--project-root))
         (candidates
          (delq nil
                (list (and cogitator-profiles-dir
                           (expand-file-name "profiles.yml" cogitator-profiles-dir))
                      (and (getenv "DBT_PROFILES_DIR")
                           (expand-file-name "profiles.yml" (getenv "DBT_PROFILES_DIR")))
                      (and root (expand-file-name "profiles.yml" root))
                      (and root (expand-file-name ".dbt/profiles.yml" root))
                      (expand-file-name "~/.dbt/profiles.yml"))))
         (buf (get-buffer-create "*cogitator-doctor*")))
    (with-current-buffer buf
      (let ((inhibit-read-only t)
            (seen nil))
        (erase-buffer)
        (insert (format "cogitator binary: %s\n"
                        (if found (format "%s (%s)" found (or version "?"))
                          (format "%s NOT FOUND on PATH" binary))))
        (insert (format "project root: %s\n" (or root "(no dbt_project.yml above here)")))
        (let ((cfg (and root
                        (seq-find #'file-exists-p
                                  (list (expand-file-name "cogitator.yml" root)
                                        (expand-file-name ".cogitator.yml" root))))))
          (insert (format "config file: %s\n"
                          (or cfg "(none — cogitator.yml at the root is hot-loaded)"))))
        (insert (format "managed: eglot=%s lsp-mode=%s\n\n"
                        (and (featurep 'eglot) (eglot-managed-p) t)
                        (bound-and-true-p lsp-mode)))
        (insert "profiles.yml search order:\n")
        (dolist (candidate candidates)
          (let ((exists (file-exists-p candidate)))
            (insert (format " %s %s\n"
                            (if (and exists (not seen)) "✓" " ")
                            candidate))
            (when exists (setq seen t))))
        (unless seen
          (insert "no profiles.yml found — the SQL dialect stays generic\n"))
        (special-mode)))
    (pop-to-buffer buf)))

;;;###autoload
(defun cogitator-cache (&optional clear)
  "Show the server's derived-cache counters; with prefix arg, CLEAR them."
  (interactive "P")
  (cogitator--execute
   "dbt.cache"
   (append (cogitator--command-argument)
           (list :action (if clear "clear" "stats")))
   (lambda (r)
     (let ((fmt (lambda (s) (format "%s/%s/%s"
                                    (plist-get s :entries)
                                    (plist-get s :hits)
                                    (plist-get s :misses)))))
       (message "cogitator caches%s — profiles %s, lineage %s, results %s (entries/hits/misses)"
                (if clear " (cleared)" "")
                (funcall fmt (plist-get r :profiles))
                (funcall fmt (plist-get r :lineage))
                (funcall fmt (plist-get r :results)))))))

;;;###autoload
(defun cogitator-cache-clear ()
  "Clear the server's derived caches and refresh the context."
  (interactive)
  (cogitator-cache t))

;;;###autoload
(defun cogitator-relation ()
  "Resolve the relation under point and put it on the kill ring.
dbt.relation answers database.schema.identifier for the source() table
or ref() at point, otherwise for the file's own model (dbt defaults
only — see the spec)."
  (interactive)
  (cogitator--execute
   "dbt.relation" (cogitator--command-argument t)
   (lambda (r)
     (if (null r)
         (message "cogitator: no relation here — not a dbt model, ref() or source()")
       (let ((relation (plist-get r :relation)))
         (kill-new relation)
         (message "cogitator: %s %s → %s (copied)"
                  (plist-get r :kind) (plist-get r :name) relation))))))

;;;###autoload
(defun cogitator-lineage ()
  "Show column lineage for the current model (arrives as a message)."
  (interactive)
  (cogitator--execute "dbt.showLineage" (cogitator--command-argument) #'ignore))

;;;###autoload
(defun cogitator-schema ()
  "Jump to the current model's properties-YAML entry."
  (interactive)
  (cogitator--execute
   "dbt.goToSchema" (cogitator--command-argument t)
   (lambda (location)
     (if (and location (plist-get location :uri))
         (cogitator--visit-location location)
       (message "No schema entry found for this model.")))))

;;;###autoload
(defun cogitator-status ()
  "Show what the cogitator server knows and has been doing."
  (interactive)
  (cogitator--execute
   "dbt.status" (cogitator--command-argument)
   (lambda (s)
     (let ((server (plist-get s :server))
           (project (plist-get s :project))
           (profiles (plist-get s :profiles))
           (refresh (plist-get s :refresh)))
       (message
        "cogitator %s — backend %s, fusion %s | project %s (%s): %s models, %s sources | profiles %s/%s | refresh #%s in %s ms"
        (plist-get server :version) (plist-get server :backend)
        (plist-get server :fusion)
        (or (plist-get project :name) "?") (plist-get project :dialect)
        (plist-get project :models) (plist-get project :sources)
        (plist-get profiles :activeProfile)
        (let ((target (plist-get profiles :activeTarget)))
          (if (and target (not (equal target ""))) target "(default)"))
        (plist-get refresh :count) (plist-get refresh :lastMs))))))

;;;###autoload
(defun cogitator-extract ()
  "Extract this model into its schema YAML, or add the next version.
The server applies the change via workspace/applyEdit and navigates to it."
  (interactive)
  (cogitator--execute "dbt.extractModelYaml" (cogitator--command-argument) #'ignore))

(defun cogitator--test-label (test)
  (concat (plist-get test :name)
          (when-let* ((column (plist-get test :column))) (format " on %s" column))
          (when-let* ((v (plist-get test :version))) (format " (v%s)" v))))

;;;###autoload
(defun cogitator-tests ()
  "Pick a test defined for this model (or one of its versions) and jump."
  (interactive)
  (cogitator--execute
   "dbt.modelTests" (cogitator--command-argument)
   (lambda (result)
     (let ((tests (and result (append (plist-get result :tests) nil))))
       (if (null tests)
           (message "cogitator: no tests defined for this model (cogitator-extract scaffolds the YAML)")
         (let* ((choices (mapcar (lambda (test) (cons (cogitator--test-label test) test)) tests))
                (pick (completing-read (format "tests for %s: " (plist-get result :model))
                                       (mapcar #'car choices) nil t))
                (test (cdr (assoc pick choices))))
           (when test
             (cogitator--visit-location test))))))))

(defun cogitator--target-choices (report)
  "((LABEL . (:profile P :target T)) …) from a dbt.profiles REPORT plist.
Only allowed profiles are offered; auth method and issue count shown."
  (let (choices)
    (seq-do
     (lambda (profile)
       (when (eq (plist-get profile :allowed) t)
         (seq-do
          (lambda (target)
            (let* ((auth (plist-get target :auth))
                   (issues (length (append (plist-get auth :issues) nil)))
                   (active (and (equal (plist-get profile :name)
                                       (plist-get report :activeProfile))
                                (equal (plist-get target :name)
                                       (plist-get report :activeTarget))))
                   (label (format "%s/%s — %s, %s%s%s"
                                  (plist-get profile :name)
                                  (plist-get target :name)
                                  (or (plist-get target :type) "?")
                                  (or (plist-get auth :method) "?")
                                  (if (> issues 0) (format "  ⚠ %d" issues) "")
                                  (if active "  (active)" ""))))
              (push (cons label (list :profile (plist-get profile :name)
                                      :target (plist-get target :name)))
                    choices)))
          (plist-get profile :targets))))
     (plist-get report :profiles))
    (nreverse choices)))

;;;###autoload
(defun cogitator-target ()
  "Pick the active dbt profile/target from the profiles.yml inventory.
The server re-resolves the SQL dialect and passes --target to dbt runs."
  (interactive)
  (cogitator--execute
   "dbt.profiles" (cogitator--command-argument)
   (lambda (report)
     (let ((choices (cogitator--target-choices report)))
       (if (null choices)
           (message "cogitator: no readable targets (check profiles.yml / the allowlist)")
         (when-let* ((pick (completing-read "dbt target: " (mapcar #'car choices) nil t))
                     (cell (assoc pick choices)))
           (cogitator--execute "dbt.setTarget" (cdr cell) #'ignore)))))))

;; ---------------------------------------------------------------------------
;; eglot (spec §4; built-in since Emacs 29)
;; ---------------------------------------------------------------------------

(defun cogitator--project-find (dir)
  "Give project.el the dbt root for DIR, so eglot picks it as workspace root."
  (when-let* ((root (locate-dominating-file dir "dbt_project.yml")))
    (cons 'transient root)))

;; Evaluated (not byte-compiled) once eglot loads: eieio generates class
;; predicates the compiler cannot know about inside with-eval-after-load.
(with-eval-after-load 'eglot
  (eval
   '(progn
      (defclass eglot-cogitator (eglot-lsp-server) ()
        :documentation "The cogitator dbt language server.")
      (cl-defmethod eglot-initialization-options ((_server eglot-cogitator))
        (or (cogitator--initialization-options) eglot--{}))) ; {} when unset
   t))

;;;###autoload
(defun cogitator-setup-eglot ()
  "Register cogitator with eglot and project.el.
Add `eglot-ensure' to your sql/yaml mode hooks afterwards."
  (require 'eglot)
  (add-hook 'project-find-functions #'cogitator--project-find)
  (add-hook 'eglot-managed-mode-hook #'cogitator--highlight-on-open)
  (add-to-list 'eglot-server-programs
               `(,cogitator-major-modes
                 . ,(lambda (&optional _interactive _project)
                      (cons 'eglot-cogitator (cogitator--server-command))))))

;; ---------------------------------------------------------------------------
;; lsp-mode
;; ---------------------------------------------------------------------------

;;;###autoload
(defun cogitator-setup-lsp-mode ()
  "Register cogitator with lsp-mode."
  (with-eval-after-load 'lsp-mode
    (declare-function make-lsp-client "lsp-mode")
    (declare-function lsp-stdio-connection "lsp-mode")
    (declare-function lsp-register-client "lsp-mode")
    (lsp-register-client
     (make-lsp-client
      :new-connection (lsp-stdio-connection #'cogitator--server-command)
      :activation-fn (lambda (file-name _mode)
                       (and (memq major-mode cogitator-major-modes)
                            (cogitator--project-root file-name)))
      :initialization-options #'cogitator--initialization-options
      :priority 1
      :server-id 'cogitator)))
  (add-hook 'lsp-after-open-hook #'cogitator--highlight-on-open))

;; ---------------------------------------------------------------------------
;; the lineage page (dbt.lineageSite)

(defun cogitator--lineage-site-message (result)
  "The message for a dbt.lineageSite RESULT written to disk, or nil."
  (when-let* ((written (and (listp result) (plist-get result :written)))
              (path (plist-get written :path)))
    (let ((counts (plist-get result :counts)))
      (format "cogitator: lineage page %s — %s of %s columns traced, %s undetermined, %s models declined"
              path
              (or (plist-get counts :columnsTraced) 0)
              (or (plist-get counts :columns) 0)
              (or (plist-get counts :columnsUndetermined) 0)
              (or (plist-get counts :modelsDeclined) 0)))))

;;;###autoload
(defun cogitator-lineage-site ()
  "Write the project's column-lineage page and open it in the browser.
The server writes target/cogitator-lineage/index.html (`dbt.lineageSite'
with write); the page works offline."
  (interactive)
  (cogitator--execute
   "dbt.lineageSite"
   (append (cogitator--command-argument) '(:write t))
   (lambda (result)
     (let ((text (cogitator--lineage-site-message result)))
       (if (not text)
           (message "cogitator: no dbt project is loaded, so there is no lineage page")
         (message "%s" text)
         (browse-url-of-file (plist-get (plist-get result :written) :path)))))))

(provide 'cogitator)
;;; cogitator.el ends here
