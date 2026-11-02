;;; cogitator-graph.el --- yazi-style model-graph browser  -*- lexical-binding: t; -*-

;;; Commentary:

;; `cogitator-graph' shows dbt.modelGraph as three side-by-side windows:
;; direct upstream models on the left, the current model in the center,
;; direct downstream models on the right.  Keys in every pane:
;;   n/p (or j/k)  select an entry
;;   TAB / h / l   move between the panes
;;   RET           re-center the graph on the selected neighbor
;;   o             open the selected entry's file (center: the model)
;;   q             close and restore the previous window layout

;;; Code:

(require 'cogitator)

(defvar cogitator-graph--state nil
  "Plist: :source-buffer :window-config :data :buffers.")

(defvar-keymap cogitator-graph-mode-map
  "n"   #'next-line
  "j"   #'next-line
  "p"   #'previous-line
  "k"   #'previous-line
  "h"   #'cogitator-graph-left
  "l"   #'cogitator-graph-right
  "TAB" #'cogitator-graph-right
  "RET" #'cogitator-graph-recenter
  "o"   #'cogitator-graph-open-entry
  "q"   #'cogitator-graph-quit)

(define-derived-mode cogitator-graph-mode special-mode "dbt-graph"
  "Major mode for the cogitator model-graph panes."
  (setq-local cursor-in-non-selected-windows nil)
  (hl-line-mode 1))

;; ---------------------------------------------------------------------------
;; rendering
;; ---------------------------------------------------------------------------

(defun cogitator-graph--entry-label (entry)
  (concat (plist-get entry :name)
          (when (equal (plist-get entry :kind) "source") "  [source]")
          (unless (plist-get entry :uri) "  (missing)")))

(defun cogitator-graph--fill (buffer entries &optional center-lines)
  "Render ENTRIES (or CENTER-LINES) into BUFFER, tagging lines with the entry."
  (with-current-buffer buffer
    (let ((inhibit-read-only t))
      (erase-buffer)
      (cond
       (center-lines (insert (string-join center-lines "\n")))
       ((seq-empty-p entries) (insert "· none ·"))
       (t (seq-do (lambda (entry)
                    (insert (propertize (cogitator-graph--entry-label entry)
                                        'cogitator-entry entry)
                            "\n"))
                  entries)
          (delete-char -1)))
      (goto-char (point-min)))))

(defun cogitator-graph--render (data)
  (setq cogitator-graph--state
        (plist-put cogitator-graph--state :data data))
  (let* ((bufs (plist-get cogitator-graph--state :buffers))
         (path (cogitator--uri-to-path (plist-get data :uri)))
         (up (plist-get data :upstream))
         (down (plist-get data :downstream)))
    (cogitator-graph--fill (nth 0 bufs) up)
    (cogitator-graph--fill (nth 1 bufs) nil
                           (list (plist-get data :model)
                                 ""
                                 (abbreviate-file-name path)
                                 ""
                                 (format "%d upstream · %d downstream"
                                         (length up) (length down))))
    (cogitator-graph--fill (nth 2 bufs) down)))

;; ---------------------------------------------------------------------------
;; commands
;; ---------------------------------------------------------------------------

(defun cogitator-graph--fetch (argument)
  "Request dbt.modelGraph with ARGUMENT from the originating buffer."
  (let ((source (plist-get cogitator-graph--state :source-buffer)))
    (if (not (buffer-live-p source))
        (message "cogitator: source buffer is gone; reopen the graph")
      (with-current-buffer source
        (cogitator--execute
         "dbt.modelGraph" argument
         (lambda (data)
           (if data
               (when cogitator-graph--state (cogitator-graph--render data))
             (message "cogitator: not in the project graph"))))))))

(defun cogitator-graph--selected-entry ()
  (get-text-property (line-beginning-position) 'cogitator-entry))

(defun cogitator-graph-recenter ()
  "Re-center the graph on the neighbor under point."
  (interactive)
  (when-let* ((entry (cogitator-graph--selected-entry)))
    (if (equal (plist-get entry :kind) "source")
        (message "%s is a source — no further lineage behind it"
                 (plist-get entry :name))
      (cogitator-graph--fetch (list :uri "" :model (plist-get entry :name))))))

(defun cogitator-graph-open-entry ()
  "Open the file of the entry under point (center pane: the model itself)."
  (interactive)
  (let* ((entry (cogitator-graph--selected-entry))
         (uri (if entry (plist-get entry :uri)
                (plist-get (plist-get cogitator-graph--state :data) :uri))))
    (if (not uri)
        (message "cogitator: no file on disk for this entry")
      (cogitator-graph-quit)
      (find-file (cogitator--uri-to-path uri)))))

(defun cogitator-graph--pane-windows ()
  (seq-filter #'window-live-p
              (mapcar #'get-buffer-window
                      (plist-get cogitator-graph--state :buffers))))

(defun cogitator-graph-left ()
  "Focus the pane to the left."
  (interactive)
  (when-let* ((windows (cogitator-graph--pane-windows))
              (pos (seq-position windows (selected-window))))
    (select-window (nth (max 0 (1- pos)) windows))))

(defun cogitator-graph-right ()
  "Focus the pane to the right."
  (interactive)
  (when-let* ((windows (cogitator-graph--pane-windows))
              (pos (seq-position windows (selected-window))))
    (select-window (nth (min (1- (length windows)) (1+ pos)) windows))))

(defun cogitator-graph-quit ()
  "Close the browser and restore the previous window layout."
  (interactive)
  (when cogitator-graph--state
    (let ((config (plist-get cogitator-graph--state :window-config))
          (buffers (plist-get cogitator-graph--state :buffers)))
      (setq cogitator-graph--state nil)
      (when config (set-window-configuration config))
      (dolist (buf buffers)
        (when (buffer-live-p buf) (kill-buffer buf))))))

;;;###autoload
(defun cogitator-graph ()
  "Browse the model graph of the current buffer's model."
  (interactive)
  (cogitator-graph-quit)
  (let ((source (current-buffer))
        (config (current-window-configuration))
        (buffers (mapcar (lambda (name)
                           (with-current-buffer (get-buffer-create name)
                             (cogitator-graph-mode)
                             (current-buffer)))
                         '("*cogitator upstream*" "*cogitator model*"
                           "*cogitator downstream*"))))
    (setq cogitator-graph--state
          (list :source-buffer source :window-config config :buffers buffers))
    (delete-other-windows)
    (switch-to-buffer (nth 0 buffers))
    (let ((center (split-window nil nil 'right)))
      (set-window-buffer center (nth 1 buffers))
      (with-selected-window center
        (set-window-buffer (split-window nil nil 'right) (nth 2 buffers)))
      (balance-windows)
      (select-window center))
    (cogitator-graph--fetch
     (with-current-buffer source (cogitator--command-argument)))))

(provide 'cogitator-graph)
;;; cogitator-graph.el ends here
