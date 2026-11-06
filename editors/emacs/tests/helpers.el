;;; helpers.el --- deterministic test helpers  -*- lexical-binding: t; -*-

;; Shared by tests/run.el: labeled waits that FAIL at their deadline
;; (never a stale assertion later), a sandboxed HOME set up BEFORE the
;; server spawns, and a per-run fixture copy so testdata/ stays pristine.

;;; Code:

(require 'cl-lib)

(defvar cogitator-test-failures 0)
(defvar cogitator-test--cleanups nil)
(defvar cogitator-test-tmp nil
  "The sandbox directory of this run (also holds the fixture copy).")

(defun cogitator-test-check (ok label &optional detail)
  "Record OK as a pass/fail line for LABEL, with DETAIL on failure."
  (if ok
      (princ (format "ok   %s\n" label))
    (setq cogitator-test-failures (1+ cogitator-test-failures))
    (princ (format "FAIL %s%s\n" label (if detail (format " — %S" detail) ""))))
  ok)

(defun cogitator-test-wait (label seconds pred &optional detail-fn)
  "Poll PRED until non-nil or SECONDS elapse; a timeout FAILS LABEL.
Returns PRED's final value. DETAIL-FN supplies failure context."
  (let ((deadline (+ (float-time) seconds))
        (value nil))
    (while (and (not (setq value (funcall pred)))
                (< (float-time) deadline))
      (accept-process-output nil 0.1))
    (unless value
      (setq cogitator-test-failures (1+ cogitator-test-failures))
      (princ (format "FAIL %s (timed out after %ss)%s\n" label seconds
                     (if detail-fn (format " — %S" (funcall detail-fn)) ""))))
    value))

(defun cogitator-test-defer (fn)
  "Run FN during `cogitator-test-finish' (also after failures)."
  (push fn cogitator-test--cleanups))

(defun cogitator-test-sandbox ()
  "Point HOME/XDG at a throwaway dir BEFORE the server starts."
  (let* ((dir (expand-file-name (format "cogitator-el-%d" (emacs-pid))
                                temporary-file-directory))
         (home (expand-file-name "home" dir)))
    (when (file-directory-p dir) (delete-directory dir t))
    (make-directory home t)
    (setenv "HOME" home)
    (setenv "USERPROFILE" home)
    (setenv "XDG_CONFIG_HOME" (expand-file-name ".config" home))
    (setenv "XDG_DATA_HOME" (expand-file-name ".local/share" home))
    (setenv "XDG_STATE_HOME" (expand-file-name ".local/state" home))
    (setenv "XDG_CACHE_HOME" (expand-file-name ".cache" home))
    (setenv "DBT_PROFILES_DIR" nil)
    (cogitator-test-defer (lambda () (ignore-errors (delete-directory dir t))))
    (setq cogitator-test-tmp dir)))

(defun cogitator-test-copy-fixture (repo)
  "Copy REPO's jaffle fixture (minus target/) into the sandbox."
  (let ((src (expand-file-name "testdata/jaffle_shop_duckdb" repo))
        (dst (expand-file-name "jaffle_shop_duckdb" cogitator-test-tmp)))
    (copy-directory src dst nil t t)
    (let ((target (expand-file-name "target" dst)))
      (when (file-directory-p target) (delete-directory target t)))
    dst))

(defun cogitator-test-server-bin (repo)
  "The server binary: COGITATOR_BIN, else the cargo target dir."
  (or (getenv "COGITATOR_BIN")
      (let ((target (or (getenv "CARGO_TARGET_DIR")
                        (expand-file-name "target" repo)))
            (exe (if (eq system-type 'windows-nt) ".exe" "")))
        (expand-file-name (concat "debug/cogitator" exe) target))))

(defun cogitator-test-eglot-start ()
  "Start eglot on the current buffer.
The ONE touchpoint of eglot's internal API (`eglot--guess-contact');
kept isolated here so an eglot change breaks exactly one function."
  (apply #'eglot (append (eglot--guess-contact) '(nil))))

(defun cogitator-test-exec (command argument)
  "One workspace/executeCommand round trip through eglot."
  (jsonrpc-request (eglot--current-server-or-lose)
                   :workspace/executeCommand
                   (list :command command :arguments (vector argument))))

(defun cogitator-test-wait-refresh (n seconds)
  "The readiness barrier: dbt.status refresh.count >= N (monotonic)."
  (let ((last nil))
    (cogitator-test-wait
     (format "refresh barrier (count >= %d)" n) seconds
     (lambda ()
       (let ((s (ignore-errors
                  (cogitator-test-exec "dbt.status"
                                       (cogitator--command-argument)))))
         (setq last s)
         (and s (>= (or (plist-get (plist-get s :refresh) :count) 0) n))))
     (lambda () last))))

(defun cogitator-test-finish ()
  "Cleanups, summary, exit code."
  (dolist (fn cogitator-test--cleanups) (ignore-errors (funcall fn)))
  (princ (if (zerop cogitator-test-failures) "ALL PASSED\n"
           (format "%d failure(s)\n" cogitator-test-failures)))
  (kill-emacs (if (zerop cogitator-test-failures) 0 1)))

(provide 'helpers)
;;; helpers.el ends here
