;; commands.el --- Navigation and editing commands for the TUI   -*- lexical-binding: t; -*-
;;
;; This file defines the everyday navigation and editing commands as
;; elisp `defun`s, built on the primitive surface Rust exposes
;; (`forward-char`, `forward-line`, `goto-char`, `point`, `point-min`,
;; `point-max`, `move-beginning-of-line`, `move-end-of-line`,
;; `beginning-of-buffer`, `end-of-buffer`, `insert`, `delete-char`,
;; `primitive-undo`, `primitive-redo`, `save-buffer`, `find-file`).
;;
;; The TUI's key handler calls these by name through `run-command`,
;; which evaluates `(name arg-count)` in the interpreter. That means
;; **these defuns are the canonical implementation** — removing the
;; matching Rust closure in `crates/app-tui/src/commands.rs` is the
;; point of this file existing.
;;
;; Each command accepts an optional numeric prefix argument so that
;; `C-u 5 C-f` (forward 5 chars) works the same way it does in Emacs.

;; ---- Navigation ----

(defun backward-char (&optional n)
  "Move point backward N chars (default 1).  Negative N moves forward."
  (forward-char (- (or n 1))))

(defun next-line (&optional n)
  "Move cursor down N lines, preserving column."
  (forward-line (or n 1)))

(defun previous-line (&optional n)
  "Move cursor up N lines, preserving column."
  (forward-line (- (or n 1))))

;; ---- Editing ----

(defun newline (&optional n)
  "Insert N newline characters (default 1)."
  (let ((count (or n 1)))
    (while (> count 0)
      (insert "\n")
      (setq count (- count 1)))))

(defun delete-backward-char (&optional n)
  "Delete N characters before point (default 1)."
  (delete-char (- (or n 1))))

;; ---- Keyboard quit (C-g) ----

(defun keyboard-quit (&optional _ignored)
  "Signal a quit condition.  Cancels any in-progress modal state in
the editor: prefix arg, pending chord (`C-x`, `M-g`), selection,
minibuffer, isearch.  Bound to `C-g` by both the GPUI and TUI key
handlers.  Takes one optional argument because the dispatcher
always passes a prefix-arg integer; we ignore it."
  (interactive)
  (editor--keyboard-quit))

;; ---- History ----

(defun undo (&optional _n)
  "Undo the last edit.  The numeric argument is ignored today; Emacs
uses it to control how far to undo but our history is linear."
  (primitive-undo))

(defun redo (&optional _n)
  "Redo the last undone edit."
  (primitive-redo))

;; ---- Kill ring ----
;;
;; These wrap `editor--*` primitives that delegate to the
;; `EditorCallbacks` trait. Embedders without a kill-ring (e.g.
;; the TUI today) get a silent no-op from the trait default — that
;; mirrors how the underlying methods behave in those clients.

(defun kill-line (&optional _n)
  "Kill text from point to end of line."
  (editor--kill-line))

(defun kill-word (&optional n)
  "Kill N words forward (default 1).  Negative N kills backward."
  (editor--kill-word (or n 1)))

(defun backward-kill-word (&optional n)
  "Kill N words backward (default 1)."
  (editor--kill-word (- (or n 1))))

(defun kill-region (&optional _arg)
  "Kill the active region."
  (editor--kill-region))

(defun copy-region-as-kill (&optional _arg)
  "Save the active region to the kill ring without deleting it."
  (editor--copy-region-as-kill))

(defun kill-ring-save (&optional _arg)
  "Save the active region to the kill ring without deleting it."
  (editor--copy-region-as-kill))

(defun yank (&optional _arg)
  "Reinsert the most recently killed text."
  (editor--yank))

(defun yank-pop (&optional _arg)
  "Replace just-yanked text with the previous kill-ring entry."
  (editor--yank-pop))

;; ---- Rectangles ----

(defun delete-rectangle (&optional _arg)
  "Delete the active rectangle without saving it."
  (editor--delete-rectangle))

(defun kill-rectangle (&optional _arg)
  "Kill the active rectangle into the rectangle buffer."
  (editor--kill-rectangle))

(defun yank-rectangle (&optional _arg)
  "Yank the most recently killed rectangle at point."
  (editor--yank-rectangle))

(defun open-rectangle (&optional _arg)
  "Insert spaces in the active rectangle, shifting text right."
  (editor--open-rectangle))

(defun clear-rectangle (&optional _arg)
  "Replace the active rectangle with spaces."
  (editor--clear-rectangle))

(defun string-rectangle (string &optional _arg)
  "Replace the active rectangle with STRING on each line."
  (interactive "sString rectangle")
  (editor--string-rectangle string))

;; ---- Search / replace ----

(defun search-forward (string &optional _bound _noerror _count)
  "Search forward for STRING."
  (interactive "sSearch: ")
  (editor--search-forward string))

(defun search-backward (string &optional _bound _noerror _count)
  "Search backward for STRING."
  (interactive "sSearch backward: ")
  (editor--search-backward string))

(defun re-search-forward (regexp &optional _bound _noerror _count)
  "Search forward for REGEXP."
  (interactive "sRegexp search: ")
  (editor--re-search-forward regexp))

(defun re-search-backward (regexp &optional _bound _noerror _count)
  "Search backward for REGEXP."
  (interactive "sRegexp search backward: ")
  (editor--re-search-backward regexp))

(defun replace-match (replacement &optional _fixedcase _literal _string _subexp)
  "Replace the most recent search match with REPLACEMENT."
  (editor--replace-match replacement))

(defun replace-string (from to &optional _delimited _start _end _backward)
  "Replace literal FROM with TO from point to the end of the buffer."
  (interactive "sReplace string: \nsReplace string with: ")
  (editor--replace-string from to))

(defun query-replace (from to &optional _delimited _start _end _backward)
  "Query replace literal FROM with TO from point to the end of the buffer."
  (interactive "sQuery replace: \nsQuery replace with: ")
  (editor--query-replace from to))

;; ---- Case ----

(defun upcase-word (&optional _n)
  "Uppercase the next word."
  (editor--upcase-word))

(defun downcase-word (&optional _n)
  "Lowercase the next word."
  (editor--downcase-word))

;; ---- Reorder ----

(defun transpose-chars (&optional _n)
  "Swap the two characters around point."
  (editor--transpose-chars))

(defun transpose-words (&optional _n)
  "Swap the two words around point."
  (editor--transpose-words))

;; ---- Mark ----

(defun set-mark (&optional _arg)
  "Set the mark at point."
  (editor--set-mark))

(defun exchange-point-and-mark (&optional _arg)
  "Exchange the cursor and the mark."
  (editor--exchange-point-and-mark))

;; ---- Buffer list ----

(defun next-buffer (&optional _n)
  "Switch to the next buffer in the list."
  (editor--next-buffer))

(defun previous-buffer (&optional _n)
  "Switch to the previous buffer in the list."
  (editor--previous-buffer))

(defun switch-to-buffer (buffer-or-name &optional _norecord _force-same-window)
  "Switch to BUFFER-OR-NAME.
The TUI supplies BUFFER-OR-NAME from the minibuffer for `C-x b';
the actual switch goes through the editor buffer bridge."
  (interactive "BSwitch to buffer: ")
  (if (or (stringp buffer-or-name) (symbolp buffer-or-name))
      (progn
        (get-buffer-create buffer-or-name)
        (set-buffer buffer-or-name))
    (current-buffer)))

(defun list-buffers (&optional _arg)
  "Show the current editor buffer list in `*Buffer List*'."
  (interactive)
  (let ((current (current-buffer))
        (buffers (buffer-list)))
    (get-buffer-create "*Buffer List*")
    (set-buffer "*Buffer List*")
    (erase-buffer)
    (insert "CRM Buffer\n")
    (insert "  . current buffer\n\n")
    (dolist (name buffers)
      (insert (if (equal name current) ". " "  "))
      (insert name)
      (insert "\n"))))

;; ---- View toggles ----

(defun toggle-preview (&optional _arg)
  "Toggle the preview pane (GPUI client only)."
  (editor--toggle-preview))

(defun toggle-line-numbers (&optional _arg)
  "Toggle editor line numbers (GPUI client only)."
  (editor--toggle-line-numbers))

(defun toggle-preview-line-numbers (&optional _arg)
  "Toggle preview line numbers (GPUI client only)."
  (editor--toggle-preview-line-numbers))

;; ---- Library / theme loading ----
;;
;; Both delegate to the `load` primitive (which honours `load-path`).
;; `load-theme` follows Emacs convention: the theme NAME (a symbol or
;; string) is suffixed with `-theme` to find the actual file.

(defun load-library (library)
  "Load LIBRARY by searching `load-path'."
  (interactive "sLoad library: ")
  (load library))

(defun load-theme (theme)
  "Load THEME by name.  Themes live in `etc/themes/' as
`THEME-theme.el' files.  We `require' upstream `custom.el' so the
theme's `(deftheme ...)` / `(custom-theme-set-faces ...)` macros
expand correctly, then `load' the theme file.  Wrapped in
`condition-case' so any per-form failure inside the theme file
surfaces as a user-visible message instead of vanishing into
stderr."
  (interactive "sLoad theme: ")
  (unless (fboundp 'custom-declare-theme)
    (require 'custom))
  (let ((name (cond
               ((symbolp theme) (symbol-name theme))
               ((stringp theme) theme)
               (t (format "%s" theme)))))
    (condition-case err
        (progn
          (load (concat name "-theme"))
          (message "Loaded theme: %s" name))
      (error
       (message "Failed to load theme `%s': %s" name err)))))

;; ---- Dired ----
;;
;; Minimal dired entry point for frontends.  It uses the editor buffer
;; bridge (`get-buffer-create`, `set-buffer`, `erase-buffer`) and the
;; elisp-side `insert-directory` primitive rather than hard-coding a
;; directory listing in the TUI.

(defun dired (path)
  "Open PATH in a dired buffer.
This is the frontend-facing dired entry point used by `C-x d'.  It
creates a generated editor buffer and populates it through the
`insert-directory' primitive, keeping the listing construction in the
elisp layer while the host owns buffer storage and rendering."
  (interactive "DDired (directory): ")
  (let* ((raw (if (and (stringp path) (not (equal path ""))) path "."))
         (dir (file-name-as-directory (expand-file-name raw)))
         (buffer-name (concat "*Dired: " dir "*")))
    (get-buffer-create buffer-name)
    (set-buffer buffer-name)
    (erase-buffer)
    (insert "  " dir ":\n")
    (insert-directory dir "-al" nil t)
    (goto-char (point-min))
    buffer-name))

(provide 'rele-tui-commands)
;;; commands.el ends here
