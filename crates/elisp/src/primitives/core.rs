use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

// Submodules grouped by domain
mod abbrevs;
mod base64;
mod faces;
mod hashing;
mod minibuf;
mod misc_system;
mod obarrays;
mod processes;
mod selections;
mod sqlite;
mod terminal;
mod threads;
mod timers;
mod xml;
mod coding_systems;
mod error_prims;
pub(crate) mod ert;
mod key_descriptions;
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
pub mod stub_telemetry;
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
    json::add_primitives(interp);
    base64::add_primitives(interp);
    coding_systems::add_primitives(interp);
    abbrevs::add_primitives(interp);
    obarrays::add_primitives(interp);
    selections::add_primitives(interp);
    faces::add_primitives(interp);
    terminal::add_primitives(interp);
    processes::add_primitives(interp);
    threads::add_primitives(interp);
    timers::add_primitives(interp);
    sqlite::add_primitives(interp);
    minibuf::add_primitives(interp);
    misc_system::add_primitives(interp);
    hashing::add_primitives(interp);
    xml::add_primitives(interp);

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
        "detect-coding-string",
        "unibyte-char-to-multibyte",
        "string-as-unibyte",
        "string-as-multibyte",
        "string-to-multibyte",
        "string-to-unibyte",
        "string-make-unibyte",
        "string-make-multibyte",
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
        "kbd",
        "key-parse",
        "define-key",
        "keymap-set",
        "define-key-after",
        "substitute-key-definition",
        "define-keymap",
        "key-valid-p",
        "make-composed-keymap",
        "set-keymap-parent",
        "keymap-parent",
        "lookup-key",
        "keymap-lookup",
        "key-binding",
        "where-is-internal",
        "copy-keymap",
        "global-set-key",
        "local-set-key",
        "global-unset-key",
        "local-unset-key",
        "use-global-map",
        "use-local-map",
        "current-global-map",
        "current-local-map",
        "current-active-maps",
        "map-keymap",
        "map-keymap-internal",
        "keyboard-translate",
        "set-quit-char",
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
        "documentation",
        "documentation-property",
        "describe-function",
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
    // Category, syntax, case-table, char-table, and charset primitives.
    for &n in crate::primitives_category::CATEGORY_PRIMITIVE_NAMES {
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
        "variable-binding-locus",
        "native-comp-unit-file",
        "native-comp-unit-set-file",
        "subr-native-comp-unit",
        "subr-native-lambda-list",
        "subr-type",
        "locale-info",
        "load-average",
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
    interp.define(
        "interactive-form",
        LispObject::primitive("interactive-form"),
    );
    interp.define("unify-charset", LispObject::primitive("unify-charset"));
    interp.define(
        "find-file-name-handler",
        LispObject::primitive("find-file-name-handler"),
    );
    interp.define(
        "unicode-property-table-internal",
        LispObject::primitive("unicode-property-table-internal"),
    );

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
    interp.define("defconst-1", LispObject::primitive("defconst-1"));
    interp.define(
        "internal--define-uninitialized-variable",
        LispObject::primitive("internal--define-uninitialized-variable"),
    );
    interp.define(
        "internal-delete-indirect-variable",
        LispObject::primitive("internal-delete-indirect-variable"),
    );
    interp.define(
        "make-interpreted-closure",
        LispObject::primitive("make-interpreted-closure"),
    );
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
        "image-type-available-p",
        "gnutls-available-p",
        "dbus-available-p",
        "native-comp-available-p",
        "backward-prefix-chars",
        "undo-boundary",
        "buffer-text-pixel-size",
        "get-load-suffixes",
        "get-truename-buffer",
        "backtrace-frame--internal",
        "make-process",
        "make-pipe-process",
        "start-process",
        "make-indirect-buffer",
        "bidi-resolved-levels",
        "file-truename",
        "expand-file-name",
        "abbreviate-file-name",
        "mapp",
        "user-uid",
        "user-real-uid",
        "group-gid",
        "group-real-gid",
        "detect-coding-region",
        "command-remapping",
        "buffer-swap-text",
        "char-displayable-p",
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
        "multibyte-string-p",
        "multibyte-char-to-unibyte",
        "string-width",
        "char-width",
        "truncate-string-to-width",
        "truncate-string-pixelwise",
        "string-pixel-width",
        "text-char-description",
        "single-key-description",
        "listify-key-sequence",
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
        "keyboard-quit",
        "abort-minibuffers",
        "minibuffer-message",
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
        "current-input-method",
        "activate-input-method",
        "deactivate-input-method",
        "set-input-method",
        "toggle-input-method",
        "describe-input-method",
        "recursive-edit",
        "top-level",
        "exit-recursive-edit",
        "abort-recursive-edit",
        "exit-minibuffer",
        "keyboard-escape-quit",
        "translation-table-id",
        "remove-hook",
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
        "set-buffer-multibyte",
        "kill-line",
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
        "add-function",
        "remove-function",
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
        // The following process primitives are handled either by
        // primitives/core/processes.rs or by stateful_process_* in
        // eval/functions/functions.rs.
        "process-send-string",
        "process-live-p",
        "process-exit-status",
        "process-name",
        "process-filter",
        "process-sentinel",
        "set-process-filter",
        "set-process-sentinel",
        "set-process-query-on-exit-flag",
        "process-query-on-exit-flag",
        "delete-process",
        "accept-process-output",
        "process-buffer",
        "set-process-buffer",
        "xml-parse-string",
        "xml-parse-region",
        "xml-parse-file",
        "libxml-parse-xml-region",
        "libxml-parse-html-region",
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
        "access-file",
        "add-name-to-file",
        "default-file-modes",
        "file-acl",
        "file-selinux-context",
        "set-file-acl",
        "substitute-in-file-name",
        "unhandled-file-name-directory",
        "self-insert-command",
        "scroll-up",
        "total-line-spacing",
        "error-type-p",
        "char-equal",
        "byte-to-string",
        "char-resolve-modifiers",
        "secure-hash-algorithms",
        "split-char",
        "copy-tramp-file-name",
        "current-time-zone",
        "map-do",
        "funcall-interactively",
        "func-arity",
        "keymap-prompt",
        "read-passwd",
        "read-kbd-macro",
        "unencodable-char-position",
        "make-record",
        "make-finalizer",
        "make-ring",
        "ring-convert-sequence-to-ring",
        "hash-table-literal",
        "hash-table-contains-p",
        "lossage-size",
        "group-name",
        "format-mode-line",
        "format-seconds",
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
    if let Some(result) = base64::call(name, args) {
        return result;
    }
    if let Some(result) = coding_systems::call(name, args) {
        return result;
    }
    if let Some(result) = abbrevs::call(name, args) {
        return result;
    }
    if let Some(result) = obarrays::call(name, args) {
        return result;
    }
    if let Some(result) = selections::call(name, args) {
        return result;
    }
    if let Some(result) = faces::call(name, args) {
        return result;
    }
    if let Some(result) = terminal::call(name, args) {
        return result;
    }
    if let Some(result) = processes::call(name, args) {
        return result;
    }
    if let Some(result) = threads::call(name, args) {
        return result;
    }
    if let Some(result) = timers::call(name, args) {
        return result;
    }
    if let Some(result) = sqlite::call(name, args) {
        return result;
    }
    if let Some(result) = minibuf::call(name, args) {
        return result;
    }
    if let Some(result) = misc_system::call(name, args) {
        return result;
    }
    if let Some(result) = hashing::call(name, args) {
        return result;
    }
    if let Some(result) = xml::call(name, args) {
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
    if let Some(result) = crate::primitives_category::call_category_primitive(name, args, None) {
        return result;
    }
    if let Some(result) = crate::primitives_window::call_window_primitive(name, args) {
        return result;
    }
    if let Some(result) = stubs::call(name, args) {
        return result;
    }

    Err(ElispError::VoidFunction(name.to_string()))
}
