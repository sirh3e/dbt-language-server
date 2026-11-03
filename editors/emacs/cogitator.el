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

(with-eval-after-load 'eglot
  (defclass eglot-cogitator (eglot-lsp-server) ()
    :documentation "The cogitator dbt language server.")
  (cl-defmethod eglot-initialization-options ((_server eglot-cogitator))
    (or (cogitator--initialization-options) eglot--{}))) ; {} when unset

;;;###autoload
(defun cogitator-setup-eglot ()
  "Register cogitator with eglot and project.el.
Add `eglot-ensure' to your sql/yaml mode hooks afterwards."
  (require 'eglot)
  (add-hook 'project-find-functions #'cogitator--project-find)
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
      :server-id 'cogitator))))

(provide 'cogitator)
;;; cogitator.el ends here
