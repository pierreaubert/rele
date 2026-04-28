;;; rust-mode.el --- Minimal Rust mode for rele -*- lexical-binding: t; -*-

(defvar rust-mode-map (make-sparse-keymap))
(define-key rust-mode-map (kbd "C-c C-c") 'rust-compile)

(defvar rust-mode-hook nil)

(defun rust-compile (&optional _arg)
  "Placeholder compile command for Rust buffers."
  (interactive)
  (message "rust-compile is not implemented yet"))

(defun rust-mode (&optional _arg)
  "Major mode for Rust source files."
  (interactive)
  (setq major-mode 'rust-mode)
  (setq mode-name "Rust")
  (editor--set-current-buffer-major-mode 'rust-mode)
  (run-mode-hooks 'rust-mode-hook))

(provide 'rust-mode)
;;; rust-mode.el ends here
