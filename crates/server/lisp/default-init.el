;;; default-init.el --- Default rele Lisp startup -*- lexical-binding: t; -*-

(defvar auto-mode-alist nil)

(autoload 'rust-mode "rust-mode" "Major mode for Rust source files." t)

(setq auto-mode-alist (cons '("\\.rs\\'" . rust-mode) auto-mode-alist))

(defun rele--auto-mode-for-file (file)
  "Return the major mode command selected for FILE by `auto-mode-alist'."
  (let ((alist auto-mode-alist)
        (mode nil))
    (while (and alist (not mode))
      (let ((entry (car alist)))
        (when (and (consp entry)
                   (stringp (car entry))
                   (string-match-p (car entry) file))
          (setq mode (cdr entry))))
      (setq alist (cdr alist)))
    mode))

(provide 'rele-default-init)
;;; default-init.el ends here
