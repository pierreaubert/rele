//! Headless-safe stubs for Emacs C DEFUNs that require GUI, processes,
//! syntax tables, or other subsystems the headless interpreter doesn't model.
//! Each returns a sensible default (usually nil or a constant) so that
//! stdlib and test files can load without `void-function` errors.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

#[allow(clippy::too_many_lines)]
pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let r = match name {
        // Headless-safe availability stubs
        "image-type-available-p"
        | "gnutls-available-p"
        | "libxml-available-p"
        | "dbus-available-p"
        | "native-comp-available-p"
        | "display-graphic-p"
        | "display-multi-frame-p"
        | "display-color-p"
        | "display-mouse-p" => Ok(LispObject::nil()),

        // Text-property stubs
        "get-char-property" | "get-pos-property" | "get-text-property" | "text-properties-at" => {
            Ok(LispObject::nil())
        }
        "text-property-not-all" => Ok(LispObject::nil()),
        "next-property-change"
        | "next-single-property-change"
        | "previous-property-change"
        | "previous-single-property-change" => Ok(LispObject::nil()),
        "set-text-properties"
        | "add-text-properties"
        | "remove-text-properties"
        | "remove-list-of-text-properties"
        | "put-text-property" => Ok(LispObject::nil()),

        // Documentation stubs
        "documentation" | "documentation-property" => Ok(LispObject::nil()),

        // Buffer/editing stubs
        "backward-prefix-chars" | "undo-boundary" => Ok(LispObject::nil()),
        "buffer-text-pixel-size" | "window-text-pixel-size" => Ok(LispObject::cons(
            LispObject::integer(0),
            LispObject::integer(0),
        )),
        "coding-system-p" | "coding-system-aliases" | "coding-system-plist" => {
            Ok(LispObject::nil())
        }
        "get-load-suffixes" => Ok(LispObject::nil()),
        "get-truename-buffer" => Ok(LispObject::nil()),
        "backtrace-frame--internal" => Ok(LispObject::nil()),
        "add-minor-mode" => Ok(LispObject::nil()),

        "make-category-table" | "category-table" => Ok(LispObject::nil()),
        "make-network-process" => Err(ElispError::EvalError(
            "make-network-process: no network in test env".to_string(),
        )),
        "make-process"
        | "make-serial-process"
        | "make-pipe-process"
        | "call-process"
        | "call-process-region"
        | "start-process"
        | "open-network-stream"
        | "process-list"
        | "get-process"
        | "process-status"
        | "process-attributes" => Ok(LispObject::nil()),
        "execute-kbd-macro" | "current-input-mode" => Ok(LispObject::nil()),
        "make-indirect-buffer"
        | "insert-buffer-substring"
        | "insert-buffer-substring-no-properties" => Ok(LispObject::nil()),
        "window-system" | "redisplay" | "force-mode-line-update" => Ok(LispObject::nil()),
        "easy-menu-do-define" | "tool-bar-local-item" | "tool-bar-local-item-from-menu" => {
            Ok(LispObject::nil())
        }
        "frame-visible-p" | "frame-live-p" | "frame-parameters" => Ok(LispObject::nil()),
        "window-live-p" | "minibufferp" | "minibuffer-window" | "minibuffer-depth"
        | "recursion-depth" => Ok(LispObject::nil()),
        "current-message" | "input-pending-p" => Ok(LispObject::nil()),
        "this-command-keys" | "this-command-keys-vector" | "recent-keys" => Ok(LispObject::nil()),
        "terminal-live-p" | "terminal-list" => Ok(LispObject::nil()),
        "last-nonminibuffer-frame" | "tty-top-frame" | "accessible-keymaps" => {
            Ok(LispObject::nil())
        }

        // Marker stubs
        "mark-marker"
        | "mark"
        | "marker-insertion-type"
        | "marker-position"
        | "set-marker"
        | "copy-marker"
        | "make-marker" => Ok(LispObject::nil()),
        "delete-and-extract-region" => Ok(LispObject::string("")),

        // Reading stubs
        "read-key-sequence" | "read-key-sequence-vector" | "read-key" => Ok(LispObject::nil()),
        "read-char" | "read-char-exclusive" => Ok(LispObject::nil()),

        // Font/charset stubs
        "font-spec" | "font-family-list" | "font-info" | "font-face-attributes" => {
            Ok(LispObject::nil())
        }
        "charset-priority-list" | "charset-list" | "charset-plist" => Ok(LispObject::nil()),
        "describe-buffer-bindings" => Ok(LispObject::nil()),
        "bidi-find-overridden-directionality" | "bidi-resolved-levels" => Ok(LispObject::nil()),
        "network-lookup-address-info" => Ok(LispObject::nil()),

        // File/path pass-through stubs
        "file-truename" | "expand-file-name" | "abbreviate-file-name" => {
            Ok(args.first().unwrap_or(LispObject::nil()))
        }
        "set-file-modes" => Ok(LispObject::nil()),

        // mapp
        "mapp" => Ok(LispObject::from(matches!(
            args.first().unwrap_or_else(LispObject::nil),
            LispObject::Cons(_)
                | LispObject::HashTable(_)
                | LispObject::Vector(_)
                | LispObject::Nil
        ))),

        // Variable/binding stubs
        "default-value" | "default-boundp" => Ok(LispObject::nil()),
        "set-default" => Ok(args.nth(1).unwrap_or(LispObject::nil())),
        "local-variable-p" | "local-variable-if-set-p" => Ok(LispObject::nil()),
        "make-variable-buffer-local" | "kill-local-variable" => {
            Ok(args.first().unwrap_or(LispObject::nil()))
        }

        // User ID stubs
        "user-uid" | "user-real-uid" => Ok(LispObject::integer(1000)),
        "group-gid" | "group-real-gid" => Ok(LispObject::integer(1000)),

        "set-default-toplevel-value" => Ok(args.nth(1).unwrap_or(LispObject::nil())),

        // Coding system stubs
        "detect-coding-string" => Ok(LispObject::cons(
            LispObject::symbol("utf-8"),
            LispObject::nil(),
        )),
        "detect-coding-region" => Ok(LispObject::symbol("utf-8")),
        "find-coding-systems-string"
        | "find-coding-systems-region"
        | "find-coding-systems-for-charsets" => Ok(LispObject::cons(
            LispObject::symbol("utf-8"),
            LispObject::nil(),
        )),
        "coding-system-base" | "coding-system-change-eol-conversion" => {
            Ok(args.first().unwrap_or(LispObject::nil()))
        }
        "command-remapping" | "buffer-swap-text" => Ok(LispObject::nil()),
        "color-values-from-color-spec" | "color-values" | "color-name-to-rgb" => {
            Ok(LispObject::nil())
        }
        "coding-system-priority-list" => Ok(LispObject::cons(
            LispObject::symbol("utf-8"),
            LispObject::nil(),
        )),
        "set-coding-system-priority" | "prefer-coding-system" => Ok(LispObject::nil()),
        "keyboard-coding-system" | "terminal-coding-system" => Ok(LispObject::symbol("utf-8")),
        "set-terminal-coding-system" | "set-keyboard-coding-system" => Ok(LispObject::nil()),

        // Completion / minibuffer stubs
        "completing-read"
        | "read-from-minibuffer"
        | "read-string"
        | "read-number"
        | "read-buffer"
        | "read-file-name" => Ok(args.nth(2).unwrap_or(LispObject::nil())),
        "yes-or-no-p" | "y-or-n-p" => Ok(LispObject::nil()),
        "message-box" | "ding" | "beep" => Ok(LispObject::nil()),

        // Char/text utilities
        "char-displayable-p" => Ok(LispObject::t()),
        "char-charset" => Ok(LispObject::symbol("unicode")),
        "char-category-set" => Ok(LispObject::nil()),
        "char-syntax" | "syntax-class-to-char" => Ok(LispObject::integer(b'.' as i64)),
        "syntax-after" => Ok(LispObject::nil()),
        "composition-get-gstring" | "find-composition" | "find-composition-internal" => {
            Ok(LispObject::nil())
        }

        // Overlay stubs
        "next-overlay-change" | "previous-overlay-change" => Ok(LispObject::nil()),

        // Image / display / face stubs
        "image-size" => Err(ElispError::EvalError(
            "image-size: not a graphic display".to_string(),
        )),
        "image-mask-p" => Err(ElispError::EvalError(
            "image-mask-p: not a graphic display".to_string(),
        )),
        "image-metadata" => Err(ElispError::EvalError(
            "image-metadata: not a graphic display".to_string(),
        )),
        "image-flush" | "image-frame-cache-size" | "create-image" => Ok(LispObject::nil()),
        "face-list"
        | "face-attribute"
        | "face-all-attributes"
        | "face-bold-p"
        | "face-italic-p"
        | "face-font"
        | "face-background"
        | "face-foreground"
        | "internal-lisp-face-p"
        | "internal-lisp-face-equal-p" => Ok(LispObject::nil()),
        "set-face-attribute" | "make-face" | "copy-face" => Ok(LispObject::nil()),

        // Menu / tool-bar stubs
        "menu-bar-lines" | "menu-bar-mode" | "tool-bar-mode" | "tab-bar-mode" | "popup-menu"
        | "x-popup-menu" => Ok(LispObject::nil()),

        // Obarray / symbol stubs
        "mapatoms" | "unintern" => Ok(LispObject::nil()),
        "symbol-plist" => Ok(LispObject::nil()),
        "add-to-list" | "add-to-ordered-list" => Ok(LispObject::nil()),
        "remove-from-invisibility-spec" | "add-to-invisibility-spec" => Ok(LispObject::nil()),

        // Buffer stubs
        "buffer-list" | "buffer-base-buffer" => Ok(LispObject::nil()),
        "buffer-chars-modified-tick" | "buffer-modified-tick" => Ok(LispObject::integer(0)),
        "buffer-modified-p" | "set-buffer-modified-p" | "restore-buffer-modified-p" => {
            Ok(LispObject::nil())
        }
        "get-buffer-window" => Ok(LispObject::nil()),

        // Window stubs
        "window-buffer"
        | "window-parent"
        | "window-parameter"
        | "window-parameters"
        | "window-frame"
        | "window-dedicated-p"
        | "window-normal-size"
        | "window-left-child"
        | "window-right-child"
        | "window-top-child"
        | "window-prev-sibling"
        | "window-next-sibling"
        | "window-prev-buffers"
        | "window-next-buffers"
        | "window-resizable"
        | "window-resizable-p"
        | "window-combination-limit"
        | "window-new-total"
        | "window-new-pixel"
        | "window-new-normal"
        | "window-old-point"
        | "window-old-pixel-height" => Ok(LispObject::nil()),
        "window-total-width"
        | "window-total-height"
        | "window-text-width"
        | "window-text-height"
        | "window-body-width"
        | "window-body-height"
        | "window-pixel-width"
        | "window-pixel-height"
        | "window-total-size"
        | "window-body-size"
        | "window-left-column"
        | "window-top-line"
        | "window-scroll-bar-height"
        | "window-scroll-bar-width"
        | "window-fringes"
        | "window-margins"
        | "window-hscroll"
        | "window-vscroll"
        | "window-line-height"
        | "window-font-height"
        | "window-font-width"
        | "window-max-chars-per-line"
        | "window-screen-lines" => Ok(LispObject::integer(80)),
        "window-pixel-edges"
        | "window-edges"
        | "window-inside-edges"
        | "window-inside-pixel-edges"
        | "window-absolute-pixel-edges"
        | "window-absolute-pixel-position"
        | "window-point"
        | "window-start"
        | "window-end"
        | "window-prompt" => Ok(LispObject::nil()),
        "window-minibuffer-p" => Ok(LispObject::nil()),

        // Frame stubs
        "frame-pixel-width" | "frame-pixel-height" => Ok(LispObject::integer(800)),
        "frame-width"
        | "frame-height"
        | "frame-total-lines"
        | "frame-native-width"
        | "frame-native-height" => Ok(LispObject::integer(80)),
        "frame-char-width"
        | "frame-char-height"
        | "frame-text-cols"
        | "frame-text-lines"
        | "frame-scroll-bar-width"
        | "frame-scroll-bar-height"
        | "frame-fringe-width"
        | "frame-font"
        | "frame-position"
        | "frame-root-window"
        | "frame-selected-window"
        | "frame-first-window"
        | "frame-focus"
        | "frame-edges"
        | "frame-border-width"
        | "frame-internal-border-height"
        | "frame-internal-border"
        | "frame-terminal"
        | "frame-tool-bar-lines"
        | "frame-menu-bar-lines"
        | "frame-internal-border-width" => Ok(LispObject::nil()),
        "redirect-frame-focus"
        | "select-frame"
        | "select-frame-set-input-focus"
        | "select-window"
        | "make-frame-visible"
        | "make-frame-invisible"
        | "iconify-frame"
        | "delete-frame"
        | "delete-window"
        | "delete-other-windows"
        | "delete-other-windows-vertically"
        | "switch-to-buffer-other-window"
        | "switch-to-buffer-other-frame"
        | "set-frame-size"
        | "set-frame-position"
        | "set-frame-width"
        | "set-frame-height"
        | "set-frame-parameter"
        | "modify-frame-parameters"
        | "set-window-buffer"
        | "set-window-parameter"
        | "set-window-dedicated-p"
        | "set-window-point"
        | "set-window-start"
        | "set-window-hscroll"
        | "set-window-vscroll"
        | "set-window-fringes"
        | "set-window-margins"
        | "set-window-scroll-bars"
        | "set-window-display-table"
        | "set-face-underline"
        | "set-face-strike-through" => Ok(LispObject::nil()),
        "frame-parameter" => Ok(LispObject::nil()),

        // X display stubs
        "x-display-pixel-width"
        | "x-display-pixel-height"
        | "x-display-mm-width"
        | "x-display-mm-height"
        | "x-display-color-cells"
        | "x-display-planes"
        | "x-display-visual-class"
        | "x-display-screens"
        | "x-display-save-under"
        | "x-display-backing-store"
        | "x-display-list"
        | "x-display-name" => Ok(LispObject::integer(0)),

        // String encoding pass-through stubs
        "unibyte-char-to-multibyte" | "multibyte-char-to-unibyte" => {
            Ok(args.first().unwrap_or(LispObject::nil()))
        }
        "string-as-unibyte"
        | "string-as-multibyte"
        | "string-to-multibyte"
        | "string-to-unibyte"
        | "string-make-unibyte"
        | "string-make-multibyte" => Ok(args.first().unwrap_or(LispObject::nil())),
        "char-width" => {
            // Delegate to string-width
            super::string::prim_string_width(args)
                .ok()
                .map_or(Ok(LispObject::integer(1)), Ok)
        }
        "truncate-string-to-width" | "truncate-string-pixelwise" => {
            Ok(args.first().unwrap_or(LispObject::nil()))
        }
        "string-pixel-width" => Ok(LispObject::integer(0)),
        "text-char-description" | "single-key-description" => super::io::prim_prin1_to_string(args)
            .ok()
            .map_or(Ok(LispObject::string("")), Ok),

        // Timer / thread — no-ops
        "run-at-time"
        | "run-with-timer"
        | "run-with-idle-timer"
        | "cancel-timer"
        | "cancel-function-timers"
        | "timer-list"
        | "timer-activate"
        | "timer-event-handler"
        | "timerp"
        | "current-idle-time" => Ok(LispObject::nil()),
        "timer-set-time" | "timer-set-function" | "timer-set-idle-time" | "timer-inc-time" => {
            Ok(LispObject::nil())
        }
        "make-thread"
        | "current-thread"
        | "thread-name"
        | "thread-alive-p"
        | "thread-join"
        | "thread-signal"
        | "thread-yield"
        | "thread-last-error"
        | "thread-live-p"
        | "thread--blocker"
        | "all-threads"
        | "condition-mutex"
        | "condition-name"
        | "condition-notify"
        | "condition-wait"
        | "make-condition-variable"
        | "make-mutex"
        | "mutex-lock"
        | "mutex-unlock"
        | "mutex-name" => Ok(LispObject::nil()),

        // Event machinery stubs
        "event-end"
        | "event-start"
        | "event-click-count"
        | "event-line-count"
        | "event-basic-type"
        | "event-modifiers"
        | "event-convert-list"
        | "posn-at-point"
        | "posn-window"
        | "posn-area"
        | "posn-x-y"
        | "posn-col-row"
        | "posn-point"
        | "posn-string"
        | "posn-object"
        | "posn-object-x-y"
        | "posn-actual-col-row"
        | "posn-timestamp"
        | "posn-image"
        | "posn-object-width-height"
        | "frame-or-buffer-changed-p" => Ok(LispObject::nil()),

        // Keymap stubs
        "kbd" => Ok(args.first().unwrap_or(LispObject::nil())),
        "global-set-key"
        | "local-set-key"
        | "global-unset-key"
        | "local-unset-key"
        | "define-key-after"
        | "substitute-key-definition"
        | "where-is-internal"
        | "set-keymap-parent"
        | "copy-keymap"
        | "keymap-parent"
        | "current-global-map"
        | "current-active-maps"
        | "use-global-map"
        | "lookup-key"
        | "map-keymap"
        | "map-keymap-internal"
        | "keyboard-translate"
        | "keyboard-quit"
        | "abort-minibuffers"
        | "minibuffer-message"
        | "set-quit-char" => Ok(LispObject::nil()),

        // Buffer-local stubs
        "kill-all-local-variables" | "buffer-local-variables" => Ok(LispObject::nil()),
        "generate-new-buffer" => Ok(args.first().unwrap_or(LispObject::nil())),
        "symbol-file" => Ok(LispObject::nil()),

        // Indentation stubs
        "current-column" | "current-indentation" => Ok(LispObject::integer(0)),
        "indent-line-to"
        | "indent-to"
        | "indent-according-to-mode"
        | "indent-for-tab-command"
        | "indent-rigidly"
        | "indent-region" => Ok(LispObject::nil()),

        // Syntax / sexp stubs
        "skip-syntax-forward"
        | "skip-syntax-backward"
        | "skip-chars-forward"
        | "skip-chars-backward" => Ok(LispObject::integer(0)),
        "parse-partial-sexp" | "scan-sexps" | "scan-lists" | "backward-up-list"
        | "forward-sexp" | "backward-sexp" | "up-list" | "down-list" | "forward-list"
        | "backward-list" | "string-to-syntax" => Ok(LispObject::nil()),
        "current-input-method" => Ok(LispObject::nil()),
        "activate-input-method"
        | "deactivate-input-method"
        | "set-input-method"
        | "toggle-input-method"
        | "describe-input-method" => Ok(LispObject::nil()),
        "syntax-table-p" => Ok(LispObject::nil()),
        "recursive-edit"
        | "top-level"
        | "exit-recursive-edit"
        | "abort-recursive-edit"
        | "exit-minibuffer"
        | "keyboard-escape-quit" => Ok(LispObject::nil()),
        "translation-table-id" => Ok(LispObject::nil()),

        // Hook stubs
        "remove-hook"
        | "run-hooks"
        | "run-hook-with-args"
        | "run-hook-with-args-until-success"
        | "run-hook-with-args-until-failure"
        | "run-hook-wrapped" => Ok(LispObject::nil()),

        // Abbrev stubs
        "make-abbrev-table"
        | "clear-abbrev-table"
        | "abbrev-table-empty-p"
        | "abbrev-expansion"
        | "abbrev-symbol"
        | "abbrev-get"
        | "abbrev-put"
        | "abbrev-insert"
        | "abbrev-table-get"
        | "abbrev-table-put"
        | "copy-abbrev-table"
        | "define-abbrev" => Ok(LispObject::nil()),

        // File visit stubs
        "set-visited-file-name"
        | "clear-visited-file-modtime"
        | "verify-visited-file-modtime"
        | "visited-file-modtime"
        | "file-locked-p"
        | "lock-buffer"
        | "unlock-buffer"
        | "ask-user-about-lock"
        | "ask-user-about-supersession-threat" => Ok(LispObject::nil()),
        "set-buffer-multibyte" => Ok(args.first().unwrap_or(LispObject::nil())),

        // Kill ring stubs
        "kill-line" | "kill-word" | "backward-kill-word" | "kill-whole-line" | "kill-region"
        | "kill-ring-save" | "kill-new" | "kill-append" | "yank" | "yank-pop" | "current-kill" => {
            Ok(LispObject::nil())
        }

        // Fringe stubs
        "define-fringe-bitmap"
        | "set-fringe-bitmap-face"
        | "fringe-bitmaps-at-pos"
        | "fringe-bitmap-p" => Ok(LispObject::nil()),

        // Bookmark stubs
        "bookmark-set" | "bookmark-jump" | "bookmark-delete" | "bookmark-get-bookmark-record" => {
            Ok(LispObject::nil())
        }

        // Selection stubs
        "x-selection-owner-p"
        | "x-selection-exists-p"
        | "x-get-selection"
        | "x-set-selection"
        | "x-own-selection"
        | "x-disown-selection"
        | "x-selection-value"
        | "gui-get-selection"
        | "gui-set-selection"
        | "gui-selection-exists-p"
        | "gui-selection-owner-p" => Ok(LispObject::nil()),
        "clipboard-yank" | "clipboard-kill-ring-save" | "clipboard-kill-region" => {
            Ok(LispObject::nil())
        }

        // Advice stubs
        "add-function"
        | "remove-function"
        | "advice-add"
        | "advice-remove"
        | "advice-function-mapc"
        | "advice-function-member-p"
        | "advice--p"
        | "advice--make"
        | "advice--add-function"
        | "advice--tweaked"
        | "advice--symbol-function"
        | "advice-eval-interactive-spec" => Ok(LispObject::nil()),

        // Debugging stubs
        "backtrace-frames"
        | "backtrace-eval"
        | "backtrace-debug"
        | "debug-on-entry"
        | "cancel-debug-on-entry"
        | "debug"
        | "mapbacktrace" => Ok(LispObject::nil()),

        // Process stubs
        "signal-process"
        | "process-send-string"
        | "process-send-region"
        | "process-send-eof"
        | "process-kill-without-query"
        | "process-running-child-p"
        | "process-live-p"
        | "process-exit-status"
        | "process-id"
        | "process-name"
        | "process-command"
        | "process-tty-name"
        | "process-coding-system"
        | "process-filter"
        | "process-sentinel"
        | "set-process-filter"
        | "set-process-sentinel"
        | "set-process-query-on-exit-flag"
        | "process-query-on-exit-flag"
        | "delete-process"
        | "continue-process"
        | "stop-process"
        | "interrupt-process"
        | "quit-process"
        | "accept-process-output"
        | "process-buffer"
        | "set-process-buffer" => Ok(LispObject::nil()),

        // XML/JSON stubs
        "xml-parse-string"
        | "xml-parse-region"
        | "xml-parse-file"
        | "libxml-parse-xml-region"
        | "libxml-parse-html-region" => Ok(LispObject::nil()),
        "json-parse-string"
        | "json-parse-buffer"
        | "json-serialize"
        | "json-encode"
        | "json-decode"
        | "json-read"
        | "json-read-from-string" => Ok(LispObject::nil()),

        // SQLite stubs
        "sqlite-open"
        | "sqlite-close"
        | "sqlite-execute"
        | "sqlite-select"
        | "sqlite-transaction"
        | "sqlite-commit"
        | "sqlite-rollback"
        | "sqlitep"
        | "sqlite-pragma"
        | "sqlite-load-extension"
        | "sqlite-version" => Ok(LispObject::nil()),

        // Tree-sitter stubs
        "treesit-parser-create"
        | "treesit-parser-delete"
        | "treesit-parser-p"
        | "treesit-parser-buffer"
        | "treesit-parser-language"
        | "treesit-node-p"
        | "treesit-node-type"
        | "treesit-node-string"
        | "treesit-node-start"
        | "treesit-node-end"
        | "treesit-node-parent"
        | "treesit-node-child"
        | "treesit-node-children"
        | "treesit-node-child-by-field-name"
        | "treesit-query-compile"
        | "treesit-query-capture"
        | "treesit-language-available-p"
        | "treesit-library-abi-version" => Ok(LispObject::nil()),

        // Apply (must go through eval dispatch)
        "apply" => Err(ElispError::EvalError(
            "apply must be called through eval dispatch".to_string(),
        )),

        // Byteorder
        "byteorder" => Ok(LispObject::integer(if cfg!(target_endian = "little") {
            108
        } else {
            66
        })),

        // --- Phase-1 C-level primitive stubs ---

        // Numeric argument helpers
        "prefix-numeric-value" => Ok(LispObject::integer(
            args.first().and_then(|a| a.as_integer()).unwrap_or(1),
        )),

        // File system stubs
        "file-name-all-completions" | "file-name-completion" => Ok(LispObject::nil()),
        "delete-directory-internal"
        | "delete-file-internal"
        | "access-file"
        | "add-name-to-file" => Ok(LispObject::nil()),
        "default-file-modes" => Ok(LispObject::integer(0o644)),
        "file-acl" | "file-selinux-context" | "set-file-acl" => Ok(LispObject::nil()),
        "substitute-in-file-name" => Ok(args.first().unwrap_or(LispObject::nil())),
        "unhandled-file-name-directory" => Ok(LispObject::nil()),

        // Buffer/editing stubs
        "self-insert-command" | "downcase-word" | "upcase-word" | "scroll-up" => {
            Ok(LispObject::nil())
        }
        "insert-and-inherit" | "insert-byte" => Ok(LispObject::nil()),
        "transpose-regions" | "upcase-initials-region" => Ok(LispObject::nil()),
        "set-marker-insertion-type" => Ok(LispObject::nil()),
        "internal--labeled-narrow-to-region" => Ok(LispObject::nil()),
        "field-beginning" => Ok(LispObject::nil()),
        "field-string-no-properties" => Ok(LispObject::string("")),
        "minibuffer-prompt-end" => Ok(LispObject::integer(1)),
        "gap-position" => Ok(LispObject::integer(1)),
        "position-bytes" => Ok(args.first().unwrap_or(LispObject::nil())),
        "total-line-spacing" => Ok(LispObject::integer(0)),
        "next-single-char-property-change" => Ok(LispObject::nil()),
        "recent-auto-save-p" => Ok(LispObject::nil()),
        "decode-coding-region" | "encode-coding-region" => Ok(LispObject::nil()),

        // Bool-vector operations
        "bool-vector-count-population" | "bool-vector-count-consecutive" => {
            Ok(LispObject::integer(0))
        }
        "bool-vector-not" | "bool-vector-union" => Ok(LispObject::nil()),

        // Type predicates (Emacs 30+)
        "bare-symbol-p" => {
            let a = args.first().unwrap_or_else(LispObject::nil);
            Ok(if a.as_symbol().is_some() {
                LispObject::t()
            } else {
                LispObject::nil()
            })
        }
        "closurep" | "error-type-p" | "module-function-p" => Ok(LispObject::nil()),

        // Character utilities
        "char-equal" => {
            let a = args.first().and_then(|v| v.as_integer()).unwrap_or(0);
            let b = args.nth(1).and_then(|v| v.as_integer()).unwrap_or(-1);
            Ok(if a == b {
                LispObject::t()
            } else {
                LispObject::nil()
            })
        }
        "byte-to-string" => {
            let byte = args.first().and_then(|v| v.as_integer()).unwrap_or(0);
            #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]
            let ch = char::from(byte as u8);
            Ok(LispObject::string(&ch.to_string()))
        }
        "char-resolve-modifiers" => Ok(args.first().unwrap_or(LispObject::nil())),
        "char-table-subtype" => Ok(LispObject::nil()),
        "charset-after" => Ok(LispObject::symbol("unicode")),
        "split-char" => Ok(LispObject::nil()),
        "get-unused-iso-final-char" => Ok(LispObject::nil()),
        "find-charset-string" | "find-charset-region" => Ok(LispObject::nil()),

        // Table/syntax copies
        "copy-syntax-table" | "copy-category-table" | "current-case-table" => Ok(LispObject::nil()),
        "make-category-set" => Ok(LispObject::nil()),
        "copy-tramp-file-name" => Ok(args.first().unwrap_or(LispObject::nil())),

        // Time
        "current-time-zone" => Ok(LispObject::cons(
            LispObject::integer(0),
            LispObject::cons(LispObject::string("UTC"), LispObject::nil()),
        )),

        // Higher-order / functional
        "map-do" => Ok(LispObject::nil()),
        "funcall-interactively" => Ok(LispObject::nil()),

        // Introspection
        "func-arity" => Ok(LispObject::cons(
            LispObject::integer(0),
            LispObject::symbol("many"),
        )),
        "keymap-prompt" => Ok(LispObject::nil()),

        // Reading stubs
        "read-passwd" => Ok(LispObject::string("")),
        "read-kbd-macro" => Ok(LispObject::nil()),

        // Encoding / charset
        "unencodable-char-position" => Ok(LispObject::nil()),
        "check-coding-systems-region" => Ok(LispObject::nil()),
        "clear-charset-maps" | "declare-equiv-charset" => Ok(LispObject::nil()),

        // Records / finalizers
        "make-record" | "make-finalizer" => Ok(LispObject::nil()),
        "make-ring" => Ok(LispObject::nil()),
        "ring-convert-sequence-to-ring" => Ok(LispObject::nil()),

        // Hash table literal (Emacs 30)
        "hash-table-literal" => Ok(LispObject::nil()),
        "hash-table-contains-p" => Ok(LispObject::nil()),

        // Obarray
        "obarray-clear" => Ok(LispObject::nil()),
        "internal--obarray-buckets" => Ok(LispObject::nil()),

        // Process-related
        "set-process-plist" | "process-contact" | "kill-process" | "kill-emacs" => {
            Ok(LispObject::nil())
        }

        // System info
        "lossage-size" => Ok(LispObject::integer(300)),
        "num-processors" => Ok(LispObject::integer(1)),
        "network-interface-list" => Ok(LispObject::nil()),
        "group-name" => Ok(LispObject::nil()),
        "get-display-property" => Ok(LispObject::nil()),

        // Display/formatting stubs
        "format-mode-line" => Ok(LispObject::string("")),
        "format-network-address" => Ok(LispObject::string("")),
        "format-seconds" => Ok(LispObject::string("")),

        // Misc C primitives
        "set-charset-plist" => Ok(LispObject::nil()),
        "iso-charset" => Ok(LispObject::nil()),
        "set-auto-mode--find-matching-alist-entry" => Ok(LispObject::nil()),
        "setopt" => Ok(LispObject::nil()),

        _ => return None,
    };
    Some(r)
}
