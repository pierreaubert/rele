use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

// Submodules grouped by domain
mod error_prims;
pub(crate) mod ert;
mod file_ops;
mod hash;
mod io;
mod json;
pub(crate) mod keymaps;
pub mod list;
mod math;
mod misc;
mod predicates;
pub mod records;
mod sequence;
mod string;
mod stubs;
mod symbol;
mod time;
mod vector;

// Re-export public ERT API used by mod.rs and the eval crate
pub use ert::{make_ert_test_obj, set_current_ert_test};
pub(crate) use vector::{
    bool_vector_length, bool_vector_to_list, char_table_extra_slot, char_table_parent,
    char_table_range, char_table_set_extra_slot, char_table_set_parent, char_table_set_range,
    char_table_subtype, is_bool_vector, is_char_table, make_char_table,
};

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    math::add_primitives(interp);
    symbol::add_primitives(interp);
    io::add_primitives(interp);
    hash::add_primitives(interp);
    time::add_primitives(interp);
    error_prims::add_primitives(interp);
    file_ops::add_primitives(interp);
    ert::add_primitives(interp);

    // Submodules that dispatch via call() but register names inline
    for &name in list::LIST_PRIMITIVE_NAMES {
        interp.define(name, LispObject::primitive(name));
    }
    for name in [
        "eq",
        "eql",
        "equal",
        "not",
        "null",
        "symbolp",
        "numberp",
        "listp",
        "consp",
        "stringp",
        "atom",
        "integerp",
        "floatp",
        "zerop",
        "natnump",
        "functionp",
        "subrp",
        "sequencep",
        "char-or-string-p",
        "booleanp",
        "keywordp",
        "obarrayp",
        "characterp",
        "arrayp",
        "char-table-p",
        "nlistp",
        "byte-code-function-p",
        "closurep",
        "hash-table-p",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
    for name in [
        "princ",
        "prin1",
        "string=",
        "string<",
        "concat",
        "substring",
        "string-to-number",
        "number-to-string",
        "make-string",
        "upcase",
        "downcase",
        "capitalize",
        "safe-length",
        "string",
        "regexp-quote",
        "max-char",
        "string-replace",
        "string-trim",
        "string-prefix-p",
        "string-suffix-p",
        "string-join",
        "string-to-list",
        "char-to-string",
        "string-to-char",
        "string-width",
        "multibyte-string-p",
        "string-search",
        "string-equal",
        "string-lessp",
        "compare-strings",
        "split-string",
        "substring-no-properties",
        "key-description",
        "upcase-initials",
        "unibyte-string",
        "string-lines",
        "format-prompt",
        "string-bytes",
        "decode-char",
        "encode-char",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
    for name in [
        "aref",
        "aset",
        "make-vector",
        "vconcat",
        "vectorp",
        "make-bool-vector",
        "bool-vector",
        "bool-vector-count-population",
        "bool-vector-count-consecutive",
        "bool-vector-not",
        "bool-vector-subsetp",
        "bool-vector-union",
        "bool-vector-intersection",
        "bool-vector-exclusive-or",
        "bool-vector-set-difference",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
    for name in [
        "elt",
        "copy-alist",
        "plist-get",
        "plist-put",
        "plist-member",
        "remove",
        "remq",
        "number-sequence",
        "proper-list-p",
        "delete",
        "rassq",
        "rassoc",
        "take",
        "ntake",
        "length<",
        "length>",
        "length=",
        "fillarray",
        "memql",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
    for name in ["record", "recordp", "record-type"] {
        interp.define(name, LispObject::primitive(name));
    }
    for name in [
        "make-sparse-keymap",
        "make-keymap",
        "keymapp",
        "define-key",
        "keymap-set",
        "define-keymap",
        "key-valid-p",
        "make-composed-keymap",
        "set-keymap-parent",
        "keymap-parent",
        "lookup-key",
        "keymap-lookup",
        "key-binding",
        "copy-keymap",
        "global-set-key",
        "local-set-key",
        "use-global-map",
        "use-local-map",
        "current-global-map",
        "current-local-map",
        "current-active-maps",
        "type-of",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
    for name in [
        "identity",
        "ignore",
        "rele--rx-translate",
        "autoload-do-load",
        "autoload-compute-prefixes",
        "connection-local-value",
        "connection-local-p",
        "file-system-info",
        "propertized-buffer-identification",
        "substitute-command-keys",
        "load-history-filename-element",
        "format-message",
    ] {
        interp.define(name, LispObject::primitive(name));
    }

    // Buffer / marker / narrow / regex / match-data. Implementations
    // live in `primitives_buffer.rs`; dispatched via `call_stateful_primitive`.
    for &n in crate::primitives_buffer::BUFFER_PRIMITIVE_NAMES {
        interp.define(n, LispObject::primitive(n));
    }
    // Window / frame / keymap — headless stubs with real state where
    // it matters (see primitives_window.rs).
    for &n in crate::primitives_window::WINDOW_PRIMITIVE_NAMES {
        interp.define(n, LispObject::primitive(n));
    }
    // File / filename / regex / process-env — see primitives_file.rs.
    for &n in crate::primitives_file::FILE_PRIMITIVE_NAMES {
        interp.define(n, LispObject::primitive(n));
    }
    // EIEIO classes/instances, widgets, advice — see primitives_eieio.rs.
    for &n in crate::primitives_eieio::EIEIO_PRIMITIVE_NAMES {
        interp.define(n, LispObject::primitive(n));
    }
    // cl-lib primitives — see primitives_cl.rs.
    for &n in crate::primitives_cl::CL_PRIMITIVE_NAMES {
        interp.define(n, LispObject::primitive(n));
    }
    for &n in crate::eval::state_cl::STATEFUL_CL_NAMES {
        interp.define(n, LispObject::primitive(n));
    }

    // Primitives that map to existing names (aliases / trivial stubs)
    let alias_to_ignore = [
        "file-name-case-insensitive-p",
        "define-coding-system-internal",
        "define-coding-system-alias",
        "set-coding-system-priority",
        "set-charset-priority",
        "set-safe-terminal-coding-system-internal",
        "obarray-make",
        "obarray-get",
        "obarray-put",
        "optimize-char-table",
        "make-char-table",
        "set-char-table-parent",
        "standard-case-table",
        "standard-syntax-table",
        "syntax-table",
        "set-syntax-table",
        "char-table-extra-slot",
        "char-table-range",
        "unify-charset",
        "find-file-name-handler",
        "unicode-property-table-internal",
        "set-char-table-range",
        "set-char-table-extra-slot",
        "map-char-table",
        "modify-category-entry",
        "modify-syntax-entry",
        "set-category-table",
        "define-category",
        "set-case-syntax",
        "set-case-syntax-pair",
        "set-case-syntax-delims",
        "variable-binding-locus",
        "interactive-form",
        "native-comp-unit-file",
        "native-comp-unit-set-file",
        "subr-native-comp-unit",
        "subr-native-lambda-list",
        "subr-type",
        "base64-encode-string",
        "base64-decode-string",
        "base64-encode-region",
        "base64-decode-region",
        "base64url-encode-string",
        "base64url-encode-region",
        "secure-hash",
        "secure-hash-algorithms",
        "md5",
        "buffer-hash",
        "locale-info",
        "load-average",
        "buffer-line-statistics",
        "clear-string",
        "define-hash-table-test",
        "object-intervals",
        "internal--hash-table-buckets",
        "internal--hash-table-histogram",
        "internal--hash-table-index-size",
        "line-number-at-pos",
        "string-distance",
    ];
    for name in alias_to_ignore {
        interp.define(name, LispObject::primitive("ignore"));
    }

    // Aliases that map to real primitives
    interp.define("position-symbol", LispObject::primitive("position-symbol"));
    interp.define(
        "remove-pos-from-symbol",
        LispObject::primitive("remove-pos-from-symbol"),
    );
    interp.define(
        "indirect-variable",
        LispObject::primitive("indirect-variable"),
    );
    interp.define("equal-including-properties", LispObject::primitive("equal"));
    interp.define(
        "string-collate-equalp",
        LispObject::primitive("string-equal"),
    );
    interp.define(
        "string-version-lessp",
        LispObject::primitive("string-lessp"),
    );
    interp.define("value<", LispObject::primitive("<"));
    interp.define("string-to-multibyte", LispObject::primitive("identity"));
    interp.define("string-to-unibyte", LispObject::primitive("identity"));
    interp.define("string-make-multibyte", LispObject::primitive("identity"));
    interp.define("string-make-unibyte", LispObject::primitive("identity"));
    interp.define("string-as-multibyte", LispObject::primitive("identity"));
    interp.define("string-as-unibyte", LispObject::primitive("identity"));
    interp.define(
        "substring-no-properties",
        LispObject::primitive("substring-no-properties"),
    );

    // Phase 8: data.c type predicates (those not already registered
    // by submodules above)
    let phase8_predicates = [
        "bufferp",
        "markerp",
        "closurep",
        "interpreted-function-p",
        "threadp",
        "mutexp",
        "condition-variable-p",
        "user-ptrp",
        "module-function-p",
        "native-comp-function-p",
        "integer-or-marker-p",
        "number-or-marker-p",
        "vector-or-char-table-p",
        "special-variable-p",
        "bare-symbol-p",
        "symbol-with-pos-p",
        "bool-vector-p",
    ];
    for name in phase8_predicates {
        interp.define(name, LispObject::primitive(name));
    }

    // data.c stubs
    interp.define("cl-type-of", LispObject::primitive("cl-type-of"));

    // fns.c higher-order (dispatched through stateful path)
    interp.define("maphash", LispObject::primitive("maphash"));
    interp.define("mapcan", LispObject::primitive("mapcan"));

    // Additional primitives
    interp.define("make-hash-table", LispObject::primitive("make-hash-table"));
    interp.define("make-closure", LispObject::primitive("make-closure"));
    interp.define("vector", LispObject::primitive("vector"));
    interp.define("format", LispObject::primitive("format"));
    interp.define("format-message", LispObject::primitive("format-message"));
    interp.define("message", LispObject::primitive("message"));
    interp.define("throw", LispObject::primitive("throw"));

    // State-aware primitives
    interp.define("defalias", LispObject::primitive("defalias"));
    interp.define("fset", LispObject::primitive("fset"));
    interp.define("eval", LispObject::primitive("eval"));
    interp.define("funcall", LispObject::primitive("funcall"));
    interp.define("apply", LispObject::primitive("apply"));
    interp.define("put", LispObject::primitive("put"));
    interp.define("get", LispObject::primitive("get"));
    interp.define("provide", LispObject::primitive("provide"));
    interp.define("featurep", LispObject::primitive("featurep"));
    interp.define("require", LispObject::primitive("require"));
    interp.define("function-put", LispObject::primitive("function-put"));
    interp.define("function-get", LispObject::primitive("function-get"));
    interp.define(
        "def-edebug-elem-spec",
        LispObject::primitive("def-edebug-elem-spec"),
    );
    interp.define("defvar-1", LispObject::primitive("defvar-1"));
    interp.define(
        "cl-generic-generalizers",
        LispObject::primitive("cl-generic-generalizers"),
    );
    interp.define(
        "cl-generic-define",
        LispObject::primitive("cl-generic-define"),
    );
    interp.define(
        "cl-generic-define-method",
        LispObject::primitive("cl-generic-define-method"),
    );

    // Batch register: all headless-safe stubs that are only
    // dispatched through the stubs module. This covers hundreds
    // of names that return nil or a constant.
    let stub_names = [
        "obarrayp",
        "make-bool-vector",
        "bool-vector",
        "window-minibuffer-p",
        "frame-internal-border-width",
        "image-type-available-p",
        "gnutls-available-p",
        "libxml-available-p",
        "dbus-available-p",
        "native-comp-available-p",
        "display-graphic-p",
        "display-multi-frame-p",
        "display-color-p",
        "display-mouse-p",
        "get-char-property",
        "get-pos-property",
        "get-text-property",
        "text-properties-at",
        "last-nonminibuffer-frame",
        "tty-top-frame",
        "accessible-keymaps",
        "documentation",
        "documentation-property",
        "backward-prefix-chars",
        "undo-boundary",
        "buffer-text-pixel-size",
        "window-text-pixel-size",
        "make-category-table",
        "category-table",
        "coding-system-plist",
        "coding-system-p",
        "coding-system-aliases",
        "get-load-suffixes",
        "get-truename-buffer",
        "backtrace-frame--internal",
        "make-network-process",
        "make-process",
        "make-serial-process",
        "make-pipe-process",
        "call-process",
        "call-process-region",
        "start-process",
        "open-network-stream",
        "process-list",
        "get-process",
        "process-status",
        "process-attributes",
        "execute-kbd-macro",
        "current-input-mode",
        "make-indirect-buffer",
        "insert-buffer-substring",
        "insert-buffer-substring-no-properties",
        "window-system",
        "redisplay",
        "force-mode-line-update",
        "frame-visible-p",
        "frame-live-p",
        "frame-parameters",
        "window-live-p",
        "minibufferp",
        "minibuffer-window",
        "minibuffer-depth",
        "recursion-depth",
        "current-message",
        "input-pending-p",
        "this-command-keys",
        "this-command-keys-vector",
        "recent-keys",
        "terminal-live-p",
        "terminal-list",
        "mark-marker",
        "mark",
        "marker-insertion-type",
        "marker-position",
        "set-marker",
        "copy-marker",
        "make-marker",
        "delete-and-extract-region",
        "read-key-sequence",
        "read-key-sequence-vector",
        "read-key",
        "read-char",
        "read-char-exclusive",
        "network-lookup-address-info",
        "font-spec",
        "font-family-list",
        "font-info",
        "font-face-attributes",
        "charset-priority-list",
        "charset-list",
        "charset-plist",
        "describe-buffer-bindings",
        "bidi-find-overridden-directionality",
        "bidi-resolved-levels",
        "text-property-not-all",
        "next-property-change",
        "next-single-property-change",
        "previous-property-change",
        "previous-single-property-change",
        "set-text-properties",
        "add-text-properties",
        "remove-text-properties",
        "remove-list-of-text-properties",
        "put-text-property",
        "file-truename",
        "expand-file-name",
        "abbreviate-file-name",
        "mapp",
        "default-value",
        "default-boundp",
        "set-default",
        "local-variable-p",
        "local-variable-if-set-p",
        "make-variable-buffer-local",
        "kill-local-variable",
        "user-uid",
        "user-real-uid",
        "group-gid",
        "group-real-gid",
        "set-default-toplevel-value",
        "detect-coding-string",
        "detect-coding-region",
        "find-coding-systems-string",
        "find-coding-systems-region",
        "find-coding-systems-for-charsets",
        "coding-system-base",
        "coding-system-change-eol-conversion",
        "command-remapping",
        "buffer-swap-text",
        "color-values-from-color-spec",
        "color-values",
        "color-name-to-rgb",
        "coding-system-priority-list",
        "set-coding-system-priority",
        "prefer-coding-system",
        "keyboard-coding-system",
        "terminal-coding-system",
        "set-terminal-coding-system",
        "set-keyboard-coding-system",
        "completing-read",
        "read-from-minibuffer",
        "read-string",
        "read-number",
        "read-buffer",
        "read-file-name",
        "yes-or-no-p",
        "y-or-n-p",
        "message-box",
        "ding",
        "beep",
        "char-displayable-p",
        "char-charset",
        "char-category-set",
        "char-syntax",
        "syntax-class-to-char",
        "syntax-after",
        "composition-get-gstring",
        "find-composition",
        "find-composition-internal",
        "make-overlay",
        "delete-overlay",
        "move-overlay",
        "overlay-start",
        "overlay-end",
        "overlay-buffer",
        "overlay-properties",
        "overlay-get",
        "overlay-put",
        "overlays-at",
        "overlays-in",
        "overlayp",
        "overlay-lists",
        "overlay-recenter",
        "remove-overlays",
        "next-overlay-change",
        "previous-overlay-change",
        "image-size",
        "image-flush",
        "image-mask-p",
        "image-frame-cache-size",
        "image-metadata",
        "create-image",
        "face-list",
        "face-attribute",
        "face-all-attributes",
        "face-bold-p",
        "face-italic-p",
        "face-font",
        "face-background",
        "face-foreground",
        "internal-lisp-face-p",
        "internal-lisp-face-equal-p",
        "set-face-attribute",
        "make-face",
        "copy-face",
        "menu-bar-lines",
        "menu-bar-mode",
        "tool-bar-mode",
        "tab-bar-mode",
        "popup-menu",
        "x-popup-menu",
        "mapatoms",
        "unintern",
        "symbol-plist",
        "add-to-list",
        "add-to-ordered-list",
        "remove-from-invisibility-spec",
        "add-to-invisibility-spec",
        "buffer-list",
        "buffer-base-buffer",
        "buffer-chars-modified-tick",
        "buffer-modified-tick",
        "buffer-modified-p",
        "set-buffer-modified-p",
        "restore-buffer-modified-p",
        "get-buffer-window",
        "window-buffer",
        "window-parent",
        "window-parameter",
        "window-parameters",
        "window-frame",
        "window-dedicated-p",
        "window-normal-size",
        "window-left-child",
        "window-right-child",
        "window-top-child",
        "window-prev-sibling",
        "window-next-sibling",
        "window-prev-buffers",
        "window-next-buffers",
        "window-resizable",
        "window-resizable-p",
        "window-combination-limit",
        "window-new-total",
        "window-new-pixel",
        "window-new-normal",
        "window-old-point",
        "window-old-pixel-height",
        "window-total-width",
        "window-total-height",
        "window-text-width",
        "window-text-height",
        "window-body-width",
        "window-body-height",
        "window-pixel-width",
        "window-pixel-height",
        "window-total-size",
        "window-body-size",
        "window-left-column",
        "window-top-line",
        "window-scroll-bar-height",
        "window-scroll-bar-width",
        "window-fringes",
        "window-margins",
        "window-hscroll",
        "window-vscroll",
        "window-line-height",
        "window-font-height",
        "window-font-width",
        "window-max-chars-per-line",
        "window-screen-lines",
        "window-pixel-edges",
        "window-edges",
        "window-inside-edges",
        "window-inside-pixel-edges",
        "window-absolute-pixel-edges",
        "window-absolute-pixel-position",
        "window-point",
        "window-start",
        "window-end",
        "window-prompt",
        "frame-pixel-width",
        "frame-pixel-height",
        "frame-width",
        "frame-height",
        "frame-total-lines",
        "frame-native-width",
        "frame-native-height",
        "frame-char-width",
        "frame-char-height",
        "frame-text-cols",
        "frame-text-lines",
        "frame-scroll-bar-width",
        "frame-scroll-bar-height",
        "frame-fringe-width",
        "frame-font",
        "frame-position",
        "frame-root-window",
        "frame-selected-window",
        "frame-first-window",
        "frame-focus",
        "frame-edges",
        "frame-border-width",
        "frame-internal-border-height",
        "frame-internal-border",
        "frame-terminal",
        "frame-tool-bar-lines",
        "frame-menu-bar-lines",
        "redirect-frame-focus",
        "select-frame",
        "select-frame-set-input-focus",
        "select-window",
        "make-frame-visible",
        "make-frame-invisible",
        "iconify-frame",
        "delete-frame",
        "delete-window",
        "delete-other-windows",
        "delete-other-windows-vertically",
        "switch-to-buffer-other-window",
        "switch-to-buffer-other-frame",
        "set-frame-size",
        "set-frame-position",
        "set-frame-width",
        "set-frame-height",
        "set-frame-parameter",
        "modify-frame-parameters",
        "set-window-buffer",
        "set-window-parameter",
        "set-window-dedicated-p",
        "set-window-point",
        "set-window-start",
        "set-window-hscroll",
        "set-window-vscroll",
        "set-window-fringes",
        "set-window-margins",
        "set-window-scroll-bars",
        "set-window-display-table",
        "set-face-underline",
        "set-face-strike-through",
        "frame-parameter",
        "x-display-pixel-width",
        "x-display-pixel-height",
        "x-display-mm-width",
        "x-display-mm-height",
        "x-display-color-cells",
        "x-display-planes",
        "x-display-visual-class",
        "x-display-screens",
        "x-display-save-under",
        "x-display-backing-store",
        "x-display-list",
        "x-display-name",
        "multibyte-string-p",
        "unibyte-char-to-multibyte",
        "multibyte-char-to-unibyte",
        "string-as-unibyte",
        "string-as-multibyte",
        "string-to-multibyte",
        "string-to-unibyte",
        "string-make-unibyte",
        "string-make-multibyte",
        "string-width",
        "char-width",
        "truncate-string-to-width",
        "truncate-string-pixelwise",
        "string-pixel-width",
        "text-char-description",
        "single-key-description",
        "run-at-time",
        "run-with-timer",
        "run-with-idle-timer",
        "cancel-timer",
        "cancel-function-timers",
        "timer-list",
        "timer-activate",
        "timer-event-handler",
        "timerp",
        "current-idle-time",
        "timer-set-time",
        "timer-set-function",
        "timer-set-idle-time",
        "timer-inc-time",
        "make-thread",
        "current-thread",
        "thread-name",
        "thread-alive-p",
        "thread-join",
        "thread-signal",
        "thread-yield",
        "thread-last-error",
        "thread-live-p",
        "thread--blocker",
        "all-threads",
        "condition-mutex",
        "condition-name",
        "condition-notify",
        "condition-wait",
        "make-condition-variable",
        "make-mutex",
        "mutex-lock",
        "mutex-unlock",
        "mutex-name",
        "event-end",
        "event-start",
        "event-click-count",
        "event-line-count",
        "event-basic-type",
        "event-modifiers",
        "event-convert-list",
        "posn-at-point",
        "posn-window",
        "posn-area",
        "posn-x-y",
        "posn-col-row",
        "posn-point",
        "posn-string",
        "posn-object",
        "posn-object-x-y",
        "posn-actual-col-row",
        "posn-timestamp",
        "posn-image",
        "posn-object-width-height",
        "frame-or-buffer-changed-p",
        "kbd",
        "global-set-key",
        "local-set-key",
        "global-unset-key",
        "local-unset-key",
        "define-key-after",
        "substitute-key-definition",
        "where-is-internal",
        "set-keymap-parent",
        "copy-keymap",
        "keymap-parent",
        "current-global-map",
        "current-active-maps",
        "use-global-map",
        "lookup-key",
        "map-keymap",
        "map-keymap-internal",
        "keyboard-translate",
        "keyboard-quit",
        "abort-minibuffers",
        "minibuffer-message",
        "set-quit-char",
        "kill-all-local-variables",
        "buffer-local-variables",
        "generate-new-buffer",
        "set-file-modes",
        "symbol-file",
        "current-column",
        "current-indentation",
        "indent-line-to",
        "indent-to",
        "indent-according-to-mode",
        "indent-for-tab-command",
        "indent-rigidly",
        "indent-region",
        "skip-syntax-forward",
        "skip-syntax-backward",
        "skip-chars-forward",
        "skip-chars-backward",
        "parse-partial-sexp",
        "scan-sexps",
        "scan-lists",
        "backward-up-list",
        "forward-sexp",
        "backward-sexp",
        "up-list",
        "down-list",
        "forward-list",
        "backward-list",
        "string-to-syntax",
        "current-input-method",
        "activate-input-method",
        "deactivate-input-method",
        "set-input-method",
        "toggle-input-method",
        "describe-input-method",
        "syntax-table-p",
        "recursive-edit",
        "top-level",
        "exit-recursive-edit",
        "abort-recursive-edit",
        "exit-minibuffer",
        "keyboard-escape-quit",
        "translation-table-id",
        "remove-hook",
        "run-hooks",
        "run-hook-with-args",
        "run-hook-with-args-until-success",
        "run-hook-with-args-until-failure",
        "run-hook-wrapped",
        "make-abbrev-table",
        "clear-abbrev-table",
        "abbrev-table-empty-p",
        "abbrev-expansion",
        "abbrev-symbol",
        "abbrev-get",
        "abbrev-put",
        "abbrev-insert",
        "abbrev-table-get",
        "abbrev-table-put",
        "copy-abbrev-table",
        "define-abbrev",
        "add-minor-mode",
        "easy-menu-do-define",
        "tool-bar-local-item",
        "tool-bar-local-item-from-menu",
        "number-sequence",
        "set-visited-file-name",
        "clear-visited-file-modtime",
        "verify-visited-file-modtime",
        "visited-file-modtime",
        "file-locked-p",
        "lock-buffer",
        "unlock-buffer",
        "set-buffer-multibyte",
        "kill-line",
        "kill-word",
        "backward-kill-word",
        "kill-whole-line",
        "kill-region",
        "kill-ring-save",
        "kill-new",
        "kill-append",
        "yank",
        "yank-pop",
        "current-kill",
        "define-fringe-bitmap",
        "set-fringe-bitmap-face",
        "fringe-bitmaps-at-pos",
        "fringe-bitmap-p",
        "bookmark-set",
        "bookmark-jump",
        "bookmark-delete",
        "bookmark-get-bookmark-record",
        "x-selection-owner-p",
        "x-selection-exists-p",
        "x-get-selection",
        "x-set-selection",
        "x-own-selection",
        "x-disown-selection",
        "x-selection-value",
        "gui-get-selection",
        "gui-set-selection",
        "gui-selection-exists-p",
        "gui-selection-owner-p",
        "clipboard-yank",
        "clipboard-kill-ring-save",
        "clipboard-kill-region",
        "add-function",
        "remove-function",
        "advice-add",
        "advice-remove",
        "advice-function-mapc",
        "advice-function-member-p",
        "advice--p",
        "advice--make",
        "advice--add-function",
        "advice--tweaked",
        "advice--symbol-function",
        "advice-eval-interactive-spec",
        "backtrace-frames",
        "backtrace-eval",
        "backtrace-debug",
        "debug-on-entry",
        "cancel-debug-on-entry",
        "debug",
        "mapbacktrace",
        "signal-process",
        "process-send-string",
        "process-send-region",
        "process-send-eof",
        "process-kill-without-query",
        "process-running-child-p",
        "process-live-p",
        "process-exit-status",
        "process-id",
        "process-name",
        "process-command",
        "process-tty-name",
        "process-coding-system",
        "process-filter",
        "process-sentinel",
        "set-process-filter",
        "set-process-sentinel",
        "set-process-query-on-exit-flag",
        "process-query-on-exit-flag",
        "delete-process",
        "continue-process",
        "stop-process",
        "interrupt-process",
        "quit-process",
        "accept-process-output",
        "process-buffer",
        "set-process-buffer",
        "xml-parse-string",
        "xml-parse-region",
        "xml-parse-file",
        "libxml-parse-xml-region",
        "libxml-parse-html-region",
        "json-parse-string",
        "json-parse-buffer",
        "json-serialize",
        "json-encode",
        "json-decode",
        "json-read",
        "json-read-from-string",
        "sqlite-open",
        "sqlite-close",
        "sqlite-execute",
        "sqlite-select",
        "sqlite-transaction",
        "sqlite-commit",
        "sqlite-rollback",
        "sqlitep",
        "sqlite-pragma",
        "sqlite-load-extension",
        "sqlite-version",
        "treesit-parser-create",
        "treesit-parser-delete",
        "treesit-parser-p",
        "treesit-parser-buffer",
        "treesit-parser-language",
        "treesit-node-p",
        "treesit-node-type",
        "treesit-node-string",
        "treesit-node-start",
        "treesit-node-end",
        "treesit-node-parent",
        "treesit-node-child",
        "treesit-node-children",
        "treesit-node-child-by-field-name",
        "treesit-query-compile",
        "treesit-query-capture",
        "treesit-language-available-p",
        "treesit-library-abi-version",
        "ask-user-about-lock",
        "ask-user-about-supersession-threat",
        "string-lines",
    ];
    for name in stub_names {
        interp.define(name, LispObject::primitive(name));
    }
    for name in [
        "kbd",
        "key-parse",
        "define-coding-system",
        "define-coding-system-internal",
        "define-coding-system-alias",
        "coding-system-p",
        "check-coding-system",
        "coding-system-list",
        "coding-system-plist",
        "coding-system-put",
        "coding-system-get",
        "coding-system-base",
        "coding-system-change-eol-conversion",
        "coding-system-priority-list",
        "set-coding-system-priority",
        "prefer-coding-system",
        "find-coding-systems-string",
        "find-coding-systems-region",
        "find-coding-systems-for-charsets",
        "define-translation-table",
        "make-translation-table-from-alist",
        "translation-table-id",
        "custom-add-option",
        "custom-add-version",
        "custom-declare-variable",
        "custom-declare-face",
        "custom-declare-group",
        "advice-add",
        "advice-remove",
        "advice-function-mapc",
        "advice-function-member-p",
        "advice-member-p",
        "advice--p",
        "advice-p",
        "ad-is-advised",
    ] {
        interp.define(name, LispObject::primitive(name));
    }

    // Phase-1 C-level primitive stubs (dispatched through stubs.rs).
    for name in [
        "prefix-numeric-value",
        "file-name-all-completions",
        "file-name-completion",
        "delete-directory-internal",
        "delete-file-internal",
        "access-file",
        "add-name-to-file",
        "default-file-modes",
        "file-acl",
        "file-selinux-context",
        "set-file-acl",
        "substitute-in-file-name",
        "unhandled-file-name-directory",
        "self-insert-command",
        "downcase-word",
        "upcase-word",
        "scroll-up",
        "insert-and-inherit",
        "insert-byte",
        "transpose-regions",
        "upcase-initials-region",
        "set-marker-insertion-type",
        "internal--labeled-narrow-to-region",
        "field-beginning",
        "field-string-no-properties",
        "minibuffer-prompt-end",
        "gap-position",
        "position-bytes",
        "total-line-spacing",
        "next-single-char-property-change",
        "recent-auto-save-p",
        "decode-coding-region",
        "encode-coding-region",
        "bool-vector-count-population",
        "bool-vector-count-consecutive",
        "bool-vector-not",
        "bool-vector-union",
        "error-type-p",
        "char-equal",
        "byte-to-string",
        "char-resolve-modifiers",
        "char-table-subtype",
        "charset-after",
        "split-char",
        "get-unused-iso-final-char",
        "find-charset-string",
        "find-charset-region",
        "copy-syntax-table",
        "copy-category-table",
        "current-case-table",
        "make-category-set",
        "copy-tramp-file-name",
        "current-time-zone",
        "map-do",
        "funcall-interactively",
        "func-arity",
        "keymap-prompt",
        "read-passwd",
        "read-kbd-macro",
        "unencodable-char-position",
        "check-coding-systems-region",
        "clear-charset-maps",
        "declare-equiv-charset",
        "make-record",
        "make-finalizer",
        "make-ring",
        "ring-convert-sequence-to-ring",
        "hash-table-literal",
        "hash-table-contains-p",
        "obarray-clear",
        "internal--obarray-buckets",
        "set-process-plist",
        "process-contact",
        "kill-process",
        "kill-emacs",
        "lossage-size",
        "num-processors",
        "network-interface-list",
        "group-name",
        "get-display-property",
        "format-mode-line",
        "format-network-address",
        "format-seconds",
        "set-charset-plist",
        "iso-charset",
        "set-auto-mode--find-matching-alist-entry",
        "setopt",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call_primitive(name: &str, args: &LispObject) -> ElispResult<LispObject> {
    // Delegate to submodules in order. Each returns Some(result) on
    // match, None on miss.
    if let Some(result) = math::call(name, args) {
        return result;
    }
    if let Some(result) = predicates::call(name, args) {
        return result;
    }
    if let Some(result) = list::call(name, args) {
        return result;
    }
    if let Some(result) = string::call(name, args) {
        return result;
    }
    if let Some(result) = vector::call(name, args) {
        return result;
    }
    if let Some(result) = sequence::call(name, args) {
        return result;
    }
    if let Some(result) = symbol::call(name, args) {
        return result;
    }
    if let Some(result) = io::call(name, args) {
        return result;
    }
    if let Some(result) = json::call(name, args) {
        return result;
    }
    if let Some(result) = records::call(name, args) {
        return result;
    }
    if let Some(result) = keymaps::call(name, args) {
        return result;
    }
    if let Some(result) = misc::call(name, args) {
        return result;
    }
    if let Some(result) = hash::call(name, args) {
        return result;
    }
    if let Some(result) = time::call(name, args) {
        return result;
    }
    if let Some(result) = error_prims::call(name, args) {
        return result;
    }
    if let Some(result) = file_ops::call(name, args) {
        return result;
    }
    if let Some(result) = ert::call(name, args) {
        return result;
    }
    if let Some(result) = stubs::call(name, args) {
        return result;
    }

    Err(ElispError::VoidFunction(name.to_string()))
}
