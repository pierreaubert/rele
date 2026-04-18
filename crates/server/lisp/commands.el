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

;; ---- History ----

(defun undo (&optional _n)
  "Undo the last edit.  The numeric argument is ignored today; Emacs
uses it to control how far to undo but our history is linear."
  (primitive-undo))

(defun redo (&optional _n)
  "Redo the last undone edit."
  (primitive-redo))

(provide 'rele-tui-commands)
;;; commands.el ends here
