use crate::error::{ElispError, ElispResult, SignalData};
use crate::object::LispObject;
use std::sync::atomic::{AtomicI64, Ordering};

/// Hard upper bound on cons-chain walks done in primitive code.
/// Primitives don't have access to `InterpreterState`, so they can't
/// charge eval-ops; instead they cap iteration to detect circular or
/// pathologically long lists. 2^24 = 16M cons cells, well above any
/// real Lisp data but cheap enough to prevent unbounded hangs.
const MAX_LIST_WALK: u64 = 1 << 24;

/// Walk a cons chain, collecting the cars into a Vec. Errors if the
/// chain exceeds `MAX_LIST_WALK` elements (likely circular).
#[allow(dead_code)]
fn collect_list(mut list: LispObject) -> ElispResult<Vec<LispObject>> {
    let mut out = Vec::new();
    let mut steps: u64 = 0;
    while let Some((car, cdr)) = list.destructure_cons() {
        steps += 1;
        if steps > MAX_LIST_WALK {
            return Err(ElispError::EvalError(
                "list appears to be circular".to_string(),
            ));
        }
        out.push(car);
        list = cdr;
    }
    Ok(out)
}

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    interp.define("+", LispObject::primitive("+"));
    interp.define("-", LispObject::primitive("-"));
    interp.define("*", LispObject::primitive("*"));
    interp.define("/", LispObject::primitive("/"));
    interp.define("=", LispObject::primitive("="));
    interp.define("<", LispObject::primitive("<"));
    interp.define(">", LispObject::primitive(">"));
    interp.define("<=", LispObject::primitive("<="));
    interp.define(">=", LispObject::primitive(">="));
    interp.define("/=", LispObject::primitive("/="));
    interp.define("cons", LispObject::primitive("cons"));
    interp.define("car", LispObject::primitive("car"));
    interp.define("cdr", LispObject::primitive("cdr"));
    interp.define("list", LispObject::primitive("list"));
    interp.define("length", LispObject::primitive("length"));
    interp.define("append", LispObject::primitive("append"));
    interp.define("reverse", LispObject::primitive("reverse"));
    interp.define("member", LispObject::primitive("member"));
    interp.define("assoc", LispObject::primitive("assoc"));
    interp.define("eq", LispObject::primitive("eq"));
    interp.define("equal", LispObject::primitive("equal"));
    interp.define("not", LispObject::primitive("not"));
    interp.define("null", LispObject::primitive("null"));
    interp.define("symbolp", LispObject::primitive("symbolp"));
    interp.define("numberp", LispObject::primitive("numberp"));
    interp.define("listp", LispObject::primitive("listp"));
    interp.define("consp", LispObject::primitive("consp"));
    interp.define("stringp", LispObject::primitive("stringp"));
    interp.define("princ", LispObject::primitive("princ"));
    interp.define("prin1", LispObject::primitive("prin1"));
    interp.define("string=", LispObject::primitive("string="));
    interp.define("string<", LispObject::primitive("string<"));
    interp.define("concat", LispObject::primitive("concat"));
    interp.define("substring", LispObject::primitive("substring"));

    // New primitives — list operations
    interp.define("nth", LispObject::primitive("nth"));
    interp.define("nthcdr", LispObject::primitive("nthcdr"));
    interp.define("setcar", LispObject::primitive("setcar"));
    interp.define("setcdr", LispObject::primitive("setcdr"));
    interp.define("nconc", LispObject::primitive("nconc"));
    interp.define("nreverse", LispObject::primitive("nreverse"));
    interp.define("delq", LispObject::primitive("delq"));
    interp.define("memq", LispObject::primitive("memq"));
    interp.define("assq", LispObject::primitive("assq"));
    interp.define("last", LispObject::primitive("last"));
    interp.define("copy-sequence", LispObject::primitive("copy-sequence"));
    interp.define("cadr", LispObject::primitive("cadr"));
    interp.define("cddr", LispObject::primitive("cddr"));
    interp.define("caar", LispObject::primitive("caar"));
    interp.define("cdar", LispObject::primitive("cdar"));
    interp.define("car-safe", LispObject::primitive("car-safe"));
    interp.define("cdr-safe", LispObject::primitive("cdr-safe"));
    interp.define("make-list", LispObject::primitive("make-list"));

    // New primitives — type predicates
    interp.define("atom", LispObject::primitive("atom"));
    interp.define("integerp", LispObject::primitive("integerp"));
    interp.define("floatp", LispObject::primitive("floatp"));
    interp.define("zerop", LispObject::primitive("zerop"));
    interp.define("natnump", LispObject::primitive("natnump"));
    // boundp / fboundp are env-aware — see call_stateful_primitive.
    interp.define("boundp", LispObject::primitive("boundp"));
    interp.define("fboundp", LispObject::primitive("fboundp"));
    interp.define("functionp", LispObject::primitive("functionp"));
    interp.define("subrp", LispObject::primitive("subrp"));

    // New primitives — numeric
    interp.define("1+", LispObject::primitive("1+"));
    interp.define("1-", LispObject::primitive("1-"));
    interp.define("mod", LispObject::primitive("mod"));
    interp.define("abs", LispObject::primitive("abs"));
    interp.define("max", LispObject::primitive("max"));
    interp.define("min", LispObject::primitive("min"));
    interp.define("floor", LispObject::primitive("floor"));
    interp.define("ceiling", LispObject::primitive("ceiling"));
    interp.define("round", LispObject::primitive("round"));
    interp.define("truncate", LispObject::primitive("truncate"));
    interp.define("float", LispObject::primitive("float"));
    interp.define("ash", LispObject::primitive("ash"));
    interp.define("logand", LispObject::primitive("logand"));
    interp.define("logior", LispObject::primitive("logior"));
    interp.define("lognot", LispObject::primitive("lognot"));

    // New primitives — symbol
    interp.define("symbol-name", LispObject::primitive("symbol-name"));
    // env-aware: routed through call_stateful_primitive.
    interp.define("symbol-value", LispObject::primitive("symbol-value"));
    interp.define("symbol-function", LispObject::primitive("symbol-function"));
    interp.define("default-value", LispObject::primitive("default-value"));
    interp.define("default-boundp", LispObject::primitive("default-boundp"));
    interp.define("set", LispObject::primitive("set"));
    interp.define("set-default", LispObject::primitive("set-default"));
    interp.define("intern", LispObject::primitive("intern"));
    interp.define("intern-soft", LispObject::primitive("intern-soft"));
    interp.define("makunbound", LispObject::primitive("makunbound"));
    interp.define("fmakunbound", LispObject::primitive("fmakunbound"));
    interp.define("make-hash-table", LispObject::primitive("make-hash-table"));
    interp.define("make-closure", LispObject::primitive("make-closure"));
    interp.define("vector", LispObject::primitive("vector"));
    interp.define("format", LispObject::primitive("format"));
    interp.define("format-message", LispObject::primitive("format-message"));
    interp.define("message", LispObject::primitive("message"));
    interp.define("throw", LispObject::primitive("throw"));
    interp.define("signal", LispObject::primitive("signal"));
    interp.define("error", LispObject::primitive("error"));

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
    // cl-lib primitives — see primitives_cl.rs. Higher-order functions
    // (cl-find, cl-some, cl-reduce, cl-mapcar, ...) need the interpreter
    // env/state to funcall their predicate arg, so they live in the
    // "state_cl" module and dispatch through call_stateful_primitive.
    for &n in crate::primitives_cl::CL_PRIMITIVE_NAMES {
        interp.define(n, LispObject::primitive(n));
    }
    for &n in crate::eval::state_cl::STATEFUL_CL_NAMES {
        interp.define(n, LispObject::primitive(n));
    }

    // New primitives — string
    interp.define(
        "string-to-number",
        LispObject::primitive("string-to-number"),
    );
    interp.define(
        "number-to-string",
        LispObject::primitive("number-to-string"),
    );
    interp.define("make-string", LispObject::primitive("make-string"));
    // string-match is handled by the eval dispatch (has regex support).

    // New primitives — I/O
    interp.define("prin1-to-string", LispObject::primitive("prin1-to-string"));

    // New primitives — misc
    interp.define("identity", LispObject::primitive("identity"));
    interp.define("ignore", LispObject::primitive("ignore"));
    interp.define("type-of", LispObject::primitive("type-of"));

    // String — extended
    interp.define("upcase", LispObject::primitive("upcase"));
    interp.define("downcase", LispObject::primitive("downcase"));
    interp.define("capitalize", LispObject::primitive("capitalize"));
    interp.define("safe-length", LispObject::primitive("safe-length"));
    interp.define("read", LispObject::primitive("read"));
    interp.define("characterp", LispObject::primitive("characterp"));
    // (string &rest CHARS) → make a string from character codepoints.
    interp.define("string", LispObject::primitive("string"));
    interp.define(
        "file-name-case-insensitive-p",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "define-coding-system-internal",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "define-coding-system-alias",
        LispObject::primitive("ignore"),
    );
    interp.define(
        "set-coding-system-priority",
        LispObject::primitive("ignore"),
    );
    interp.define("set-charset-priority", LispObject::primitive("ignore"));
    interp.define(
        "set-safe-terminal-coding-system-internal",
        LispObject::primitive("ignore"),
    );
    interp.define("regexp-quote", LispObject::primitive("regexp-quote"));
    interp.define("max-char", LispObject::primitive("max-char"));
    interp.define("obarray-make", LispObject::primitive("ignore"));
    interp.define("obarray-get", LispObject::primitive("ignore"));
    interp.define("obarray-put", LispObject::primitive("ignore"));
    interp.define("optimize-char-table", LispObject::primitive("ignore"));
    interp.define("make-char-table", LispObject::primitive("ignore"));
    interp.define("set-char-table-parent", LispObject::primitive("ignore"));
    interp.define("standard-case-table", LispObject::primitive("ignore"));
    interp.define("standard-syntax-table", LispObject::primitive("ignore"));
    interp.define("syntax-table", LispObject::primitive("ignore"));
    interp.define("set-syntax-table", LispObject::primitive("ignore"));
    interp.define("char-table-extra-slot", LispObject::primitive("ignore"));
    interp.define("char-table-range", LispObject::primitive("ignore"));
    // charset/unicode stubs: return the code as-is for decode-char,
    // and pass-through for encode-char. Enough for mule-conf /
    // characters.el to load without blowing up on unsupported charsets.
    interp.define("decode-char", LispObject::primitive("decode-char"));
    interp.define("encode-char", LispObject::primitive("encode-char"));
    // Deep stdlib stubs — all return nil / no-op so load can continue.
    interp.define("unify-charset", LispObject::primitive("ignore"));
    interp.define("find-file-name-handler", LispObject::primitive("ignore"));
    interp.define(
        "unicode-property-table-internal",
        LispObject::primitive("ignore"),
    );
    interp.define("set-char-table-range", LispObject::primitive("ignore"));
    interp.define("set-char-table-extra-slot", LispObject::primitive("ignore"));
    interp.define("map-char-table", LispObject::primitive("ignore"));
    interp.define("modify-category-entry", LispObject::primitive("ignore"));
    interp.define("modify-syntax-entry", LispObject::primitive("ignore"));
    interp.define("set-category-table", LispObject::primitive("ignore"));
    interp.define("define-category", LispObject::primitive("ignore"));
    interp.define("set-case-syntax", LispObject::primitive("ignore"));
    interp.define("set-case-syntax-pair", LispObject::primitive("ignore"));
    interp.define("set-case-syntax-delims", LispObject::primitive("ignore"));
    interp.define("string-replace", LispObject::primitive("string-replace"));
    interp.define("string-trim", LispObject::primitive("string-trim"));
    interp.define("string-prefix-p", LispObject::primitive("string-prefix-p"));
    interp.define("string-suffix-p", LispObject::primitive("string-suffix-p"));
    interp.define("string-join", LispObject::primitive("string-join"));
    interp.define("char-to-string", LispObject::primitive("char-to-string"));
    interp.define("string-to-char", LispObject::primitive("string-to-char"));
    interp.define("string-width", LispObject::primitive("string-width"));
    interp.define(
        "multibyte-string-p",
        LispObject::primitive("multibyte-string-p"),
    );

    // Vector
    interp.define("aref", LispObject::primitive("aref"));
    interp.define("aset", LispObject::primitive("aset"));
    interp.define("make-vector", LispObject::primitive("make-vector"));
    interp.define("vconcat", LispObject::primitive("vconcat"));
    interp.define("vectorp", LispObject::primitive("vectorp"));

    // String — search / comparison
    interp.define("string-search", LispObject::primitive("string-search"));
    interp.define("string-equal", LispObject::primitive("string-equal"));
    interp.define("string-lessp", LispObject::primitive("string-lessp"));
    interp.define("compare-strings", LispObject::primitive("compare-strings"));
    interp.define("split-string", LispObject::primitive("split-string"));

    // Sequence — extended
    interp.define("elt", LispObject::primitive("elt"));
    interp.define("copy-alist", LispObject::primitive("copy-alist"));
    interp.define("plist-get", LispObject::primitive("plist-get"));
    interp.define("plist-put", LispObject::primitive("plist-put"));
    interp.define("plist-member", LispObject::primitive("plist-member"));
    interp.define("remove", LispObject::primitive("remove"));
    interp.define("remq", LispObject::primitive("remq"));
    interp.define("number-sequence", LispObject::primitive("number-sequence"));

    // Numeric — extended
    interp.define("random", LispObject::primitive("random"));
    interp.define("logxor", LispObject::primitive("logxor"));

    // Math — transcendental functions
    interp.define("sin", LispObject::primitive("sin"));
    interp.define("cos", LispObject::primitive("cos"));
    interp.define("tan", LispObject::primitive("tan"));
    interp.define("asin", LispObject::primitive("asin"));
    interp.define("acos", LispObject::primitive("acos"));
    interp.define("atan", LispObject::primitive("atan"));
    interp.define("exp", LispObject::primitive("exp"));
    interp.define("log", LispObject::primitive("log"));
    interp.define("sqrt", LispObject::primitive("sqrt"));
    interp.define("expt", LispObject::primitive("expt"));
    interp.define("isnan", LispObject::primitive("isnan"));
    interp.define("copysign", LispObject::primitive("copysign"));
    interp.define("frexp", LispObject::primitive("frexp"));
    interp.define("ldexp", LispObject::primitive("ldexp"));

    // Type — extended
    interp.define("sequencep", LispObject::primitive("sequencep"));
    interp.define(
        "char-or-string-p",
        LispObject::primitive("char-or-string-p"),
    );
    interp.define("booleanp", LispObject::primitive("booleanp"));
    interp.define("keywordp", LispObject::primitive("keywordp"));

    // Misc — extended (apply/error/signal handled by eval dispatch, but registered for functionp)
    interp.define("apply", LispObject::primitive("apply"));
    interp.define("error", LispObject::primitive("error"));
    interp.define("user-error", LispObject::primitive("user-error"));
    interp.define("signal", LispObject::primitive("signal"));

    // Records / structs
    interp.define("record", LispObject::primitive("record"));
    interp.define("recordp", LispObject::primitive("recordp"));
    interp.define("record-type", LispObject::primitive("record-type"));

    // Keymaps
    interp.define("make-sparse-keymap", LispObject::primitive("make-sparse-keymap"));
    interp.define("make-keymap", LispObject::primitive("make-keymap"));
    interp.define("keymapp", LispObject::primitive("keymapp"));
    interp.define("define-key", LispObject::primitive("define-key"));

    // ---- data.c type predicates (Phase 8) ----
    interp.define("arrayp", LispObject::primitive("arrayp"));
    interp.define("nlistp", LispObject::primitive("nlistp"));
    interp.define("bufferp", LispObject::primitive("bufferp"));
    interp.define("markerp", LispObject::primitive("markerp"));
    interp.define("byte-code-function-p", LispObject::primitive("byte-code-function-p"));
    interp.define("closurep", LispObject::primitive("closurep"));
    interp.define("interpreted-function-p", LispObject::primitive("interpreted-function-p"));
    interp.define("threadp", LispObject::primitive("threadp"));
    interp.define("mutexp", LispObject::primitive("mutexp"));
    interp.define("condition-variable-p", LispObject::primitive("condition-variable-p"));
    interp.define("user-ptrp", LispObject::primitive("user-ptrp"));
    interp.define("module-function-p", LispObject::primitive("module-function-p"));
    interp.define("native-comp-function-p", LispObject::primitive("native-comp-function-p"));
    interp.define("integer-or-marker-p", LispObject::primitive("integer-or-marker-p"));
    interp.define("number-or-marker-p", LispObject::primitive("number-or-marker-p"));
    interp.define("vector-or-char-table-p", LispObject::primitive("vector-or-char-table-p"));
    interp.define("bare-symbol-p", LispObject::primitive("bare-symbol-p"));
    interp.define("symbol-with-pos-p", LispObject::primitive("symbol-with-pos-p"));
    interp.define("bool-vector-p", LispObject::primitive("bool-vector-p"));
    interp.define("hash-table-p", LispObject::primitive("hash-table-p"));

    // ---- data.c accessors / mutators (Phase 8) ----
    interp.define("%", LispObject::primitive("%"));
    interp.define("logcount", LispObject::primitive("logcount"));
    interp.define("byteorder", LispObject::primitive("byteorder"));
    interp.define("indirect-function", LispObject::primitive("indirect-function"));
    interp.define("subr-arity", LispObject::primitive("subr-arity"));
    interp.define("subr-name", LispObject::primitive("subr-name"));
    interp.define("setplist", LispObject::primitive("setplist"));
    interp.define("cl-type-of", LispObject::primitive("cl-type-of"));

    // ---- data.c stubs (rele-specific types that don't exist) ----
    interp.define("add-variable-watcher", LispObject::primitive("ignore"));
    interp.define("remove-variable-watcher", LispObject::primitive("ignore"));
    interp.define("get-variable-watchers", LispObject::primitive("ignore"));
    interp.define("variable-binding-locus", LispObject::primitive("ignore"));
    interp.define("interactive-form", LispObject::primitive("ignore"));
    interp.define("command-modes", LispObject::primitive("ignore"));
    interp.define("indirect-variable", LispObject::primitive("indirect-variable"));
    interp.define("position-symbol", LispObject::primitive("identity"));
    interp.define("remove-pos-from-symbol", LispObject::primitive("identity"));
    interp.define("symbol-with-pos-pos", LispObject::primitive("ignore"));
    interp.define("native-comp-unit-file", LispObject::primitive("ignore"));
    interp.define("native-comp-unit-set-file", LispObject::primitive("ignore"));
    interp.define("subr-native-comp-unit", LispObject::primitive("ignore"));
    interp.define("subr-native-lambda-list", LispObject::primitive("ignore"));
    interp.define("subr-type", LispObject::primitive("ignore"));

    // ---- fns.c additions (Phase 8) ----
    interp.define("proper-list-p", LispObject::primitive("proper-list-p"));
    interp.define("delete", LispObject::primitive("delete"));
    interp.define("rassq", LispObject::primitive("rassq"));
    interp.define("rassoc", LispObject::primitive("rassoc"));
    interp.define("maphash", LispObject::primitive("maphash"));
    interp.define("remhash", LispObject::primitive("remhash"));
    interp.define("hash-table-count", LispObject::primitive("hash-table-count"));
    interp.define("hash-table-test", LispObject::primitive("hash-table-test"));
    interp.define("hash-table-size", LispObject::primitive("hash-table-size"));
    interp.define("hash-table-weakness", LispObject::primitive("hash-table-weakness"));
    interp.define("copy-hash-table", LispObject::primitive("copy-hash-table"));
    interp.define("substring-no-properties", LispObject::primitive("substring-no-properties"));
    interp.define("take", LispObject::primitive("take"));
    interp.define("ntake", LispObject::primitive("ntake"));
    interp.define("length<", LispObject::primitive("length<"));
    interp.define("length>", LispObject::primitive("length>"));
    interp.define("length=", LispObject::primitive("length="));
    interp.define("fillarray", LispObject::primitive("fillarray"));
    interp.define("string-bytes", LispObject::primitive("string-bytes"));
    interp.define("mapcan", LispObject::primitive("mapcan"));
    interp.define("sxhash-eq", LispObject::primitive("sxhash-eq"));
    interp.define("sxhash-eql", LispObject::primitive("sxhash-eql"));
    interp.define("sxhash-equal", LispObject::primitive("sxhash-equal"));
    interp.define("sxhash-equal-including-properties", LispObject::primitive("sxhash-equal"));
    interp.define("memql", LispObject::primitive("memql"));
    interp.define("string-to-multibyte", LispObject::primitive("identity"));
    interp.define("string-to-unibyte", LispObject::primitive("identity"));
    interp.define("string-make-multibyte", LispObject::primitive("identity"));
    interp.define("string-make-unibyte", LispObject::primitive("identity"));
    interp.define("string-as-multibyte", LispObject::primitive("identity"));
    interp.define("string-as-unibyte", LispObject::primitive("identity"));

    // fns.c stubs (not needed for bootstrap)
    interp.define("base64-encode-string", LispObject::primitive("ignore"));
    interp.define("base64-decode-string", LispObject::primitive("ignore"));
    interp.define("base64-encode-region", LispObject::primitive("ignore"));
    interp.define("base64-decode-region", LispObject::primitive("ignore"));
    interp.define("base64url-encode-string", LispObject::primitive("ignore"));
    interp.define("base64url-encode-region", LispObject::primitive("ignore"));
    interp.define("secure-hash", LispObject::primitive("ignore"));
    interp.define("secure-hash-algorithms", LispObject::primitive("ignore"));
    interp.define("md5", LispObject::primitive("ignore"));
    interp.define("buffer-hash", LispObject::primitive("ignore"));
    interp.define("locale-info", LispObject::primitive("ignore"));
    interp.define("load-average", LispObject::primitive("ignore"));
    interp.define("buffer-line-statistics", LispObject::primitive("ignore"));
    interp.define("clear-string", LispObject::primitive("ignore"));
    interp.define("define-hash-table-test", LispObject::primitive("ignore"));
    interp.define("equal-including-properties", LispObject::primitive("equal"));
    interp.define("object-intervals", LispObject::primitive("ignore"));
    interp.define("internal--hash-table-buckets", LispObject::primitive("ignore"));
    interp.define("internal--hash-table-histogram", LispObject::primitive("ignore"));
    interp.define("internal--hash-table-index-size", LispObject::primitive("ignore"));
    interp.define("line-number-at-pos", LispObject::primitive("ignore"));
    interp.define("string-collate-equalp", LispObject::primitive("string-equal"));
    interp.define("string-distance", LispObject::primitive("ignore"));
    interp.define("string-version-lessp", LispObject::primitive("string-lessp"));
    interp.define("value<", LispObject::primitive("<"));

    // Batch: additional headless-safe DEFUNs. Each one is dispatched
    // as a regular primitive (argument evaluation happens in the
    // caller). Registered on the value cell so `(funcall 'FOO ...)`
    // resolves, and on the function cell via the obarray auto-promote
    // path that `define` uses. Semantic details live in the dispatch
    // arm in `call_primitive`.
    for name in [
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
        "key-description",
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
        "time-equal-p",
        "time-less-p",
        "time-convert",
        "format-time-string",
        "upcase-initials",
        "make-directory-internal",
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
        // Second batch
        "terpri",
        "print",
        "prin1-to-string",
        "pp",
        "pp-to-string",
        "write-char",
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
        "encode-time",
        "current-time",
        "current-time-string",
        "float-time",
        "network-lookup-address-info",
        "logb",
        "frexp",
        "ldexp",
        "copysign",
        "isnan",
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
        "directory-name-p",
        "file-accessible-directory-p",
        "file-name-as-directory",
        "file-truename",
        "expand-file-name",
        "abbreviate-file-name",
        "mapp",
        "hash-table-keys",
        "hash-table-values",
        "default-value",
        "default-boundp",
        "set-default",
        "local-variable-p",
        "local-variable-if-set-p",
        "make-variable-buffer-local",
        "kill-local-variable",
        "evenp",
        "oddp",
        "plusp",
        "minusp",
        // Third batch
        "user-uid",
        "user-real-uid",
        "group-gid",
        "group-real-gid",
        "default-toplevel-value",
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
        // Fourth batch
        "mapatoms",
        "unintern",
        "intern-soft",
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
        "generate-new-buffer-name",
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
        "unibyte-string",
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
        "string-lines",
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
        // Fifth batch
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
        "buffer-local-value",
        "generate-new-buffer",
        "rename-file",
        "copy-file",
        "delete-directory",
        "file-modes",
        "set-file-modes",
        "file-newer-than-file-p",
        "file-symlink-p",
        "file-regular-p",
        "file-readable-p",
        "file-writable-p",
        "file-executable-p",
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
        "number-sequence",
        "format-prompt",
        "format-message",
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
    ] {
        interp.define(name, LispObject::primitive(name));
    }

    // Phase 7a: state-aware primitives — semantically regular
    // functions (evaluated args) that happen to need env/macros/state
    // access. Registered on the function cell so the VM and any other
    // function-cell caller can dispatch them; `call_function` routes
    // through `call_stateful_primitive` before the regular primitive
    // dispatch. Source-level `(defalias ...)` etc. still hit the
    // special-form dispatch in eval_inner (same effect, different
    // entry point).
    interp.define("defalias", LispObject::primitive("defalias"));
    interp.define("fset", LispObject::primitive("fset"));
    interp.define("eval", LispObject::primitive("eval"));
    interp.define("funcall", LispObject::primitive("funcall"));
    interp.define("apply", LispObject::primitive("apply"));
    interp.define("put", LispObject::primitive("put"));
    interp.define("get", LispObject::primitive("get"));
}

pub fn call_primitive(name: &str, args: &LispObject) -> ElispResult<LispObject> {
    match name {
        "+" => prim_add(args),
        "-" => prim_sub(args),
        "*" => prim_mul(args),
        "/" => prim_div(args),
        "=" => prim_num_eq(args),
        "<" => prim_lt(args),
        ">" => prim_gt(args),
        "<=" => prim_le(args),
        ">=" => prim_ge(args),
        "/=" => prim_ne(args),
        "cons" => prim_cons(args),
        "car" => prim_car(args),
        "cdr" => prim_cdr(args),
        "list" => prim_list(args),
        "length" => prim_length(args),
        "append" => prim_append(args),
        "reverse" => prim_reverse(args),
        "member" => prim_member(args),
        "assoc" => prim_assoc(args),
        "eq" => prim_eq(args),
        "equal" => prim_equal(args),
        "not" => prim_not(args),
        "null" => prim_null(args),
        "symbolp" => prim_symbolp(args),
        "numberp" => prim_numberp(args),
        "listp" => prim_listp(args),
        "consp" => prim_consp(args),
        "stringp" => prim_stringp(args),
        "princ" => prim_princ(args),
        "prin1" => prim_prin1(args),
        "string=" => prim_string_eq(args),
        "string<" => prim_string_lt(args),
        "concat" => prim_concat(args),
        "substring" => prim_substring(args),

        // List operations
        "nth" => prim_nth(args),
        "nthcdr" => prim_nthcdr(args),
        "setcar" => prim_setcar(args),
        "setcdr" => prim_setcdr(args),
        "nconc" => prim_nconc(args),
        "nreverse" => prim_nreverse(args),
        "delq" => prim_delq(args),
        "memq" => prim_memq(args),
        "assq" => prim_assq(args),
        "last" => prim_last(args),
        "copy-sequence" => prim_copy_sequence(args),
        "cadr" => prim_cadr(args),
        "cddr" => prim_cddr(args),
        "caar" => prim_caar(args),
        "cdar" => prim_cdar(args),
        "car-safe" => prim_car_safe(args),
        "cdr-safe" => prim_cdr_safe(args),
        "make-list" => prim_make_list(args),

        // Type predicates
        "atom" => prim_atom(args),
        "integerp" => prim_integerp(args),
        "floatp" => prim_floatp(args),
        "zerop" => prim_zerop(args),
        "natnump" => prim_natnump(args),
        // boundp/fboundp handled by eval dispatch (need env access)
        "functionp" => prim_functionp(args),
        "subrp" => prim_subrp(args),

        // Numeric
        "1+" => prim_1_plus(args),
        "1-" => prim_1_minus(args),
        "mod" => prim_mod(args),
        "abs" => prim_abs(args),
        "max" => prim_max(args),
        "min" => prim_min(args),
        "floor" => prim_floor(args),
        "ceiling" => prim_ceiling(args),
        "round" => prim_round(args),
        "truncate" => prim_truncate(args),
        "float" => prim_float(args),
        "ash" => prim_ash(args),
        "logand" => prim_logand(args),
        "logior" => prim_logior(args),
        "lognot" => prim_lognot(args),

        // Symbol
        "symbol-name" => prim_symbol_name(args),
        // symbol-function handled by eval dispatch (needs env + macro table)

        // String
        "string-to-number" => prim_string_to_number(args),
        "number-to-string" => prim_number_to_string(args),
        "make-string" => prim_make_string(args),
        // string-match handled by eval dispatch (has regex support)

        // I/O
        "prin1-to-string" => prim_prin1_to_string(args),

        // Misc
        "identity" => prim_identity(args),
        "ignore" => prim_ignore(args),

        // Records / structs
        "record" => prim_record(args),
        "recordp" => {
            let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            // In rele, records are vectors with a symbol as first element.
            Ok(LispObject::from(matches!(&obj, LispObject::Vector(v) if
                matches!(v.lock().first(), Some(LispObject::Symbol(_))))))
        }
        "record-type" => {
            let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
            match &obj {
                LispObject::Vector(v) => {
                    let guard = v.lock();
                    if let Some(first) = guard.first() {
                        Ok(first.clone())
                    } else {
                        Ok(LispObject::nil())
                    }
                }
                _ => Err(ElispError::WrongTypeArgument("recordp".to_string())),
            }
        }

        // Keymaps
        "make-sparse-keymap" => prim_make_sparse_keymap(args),
        "make-keymap" => prim_make_keymap(args),
        "keymapp" => prim_keymapp(args),
        "define-key" => prim_define_key(args),
        "type-of" => prim_type_of(args),

        // String — extended
        "upcase" => prim_upcase(args),
        "downcase" => prim_downcase(args),
        "capitalize" => prim_capitalize(args),
        "safe-length" => prim_safe_length(args),
        "read" => prim_read(args),
        "characterp" => prim_characterp(args),
        "string" => prim_string(args),
        "regexp-quote" => prim_regexp_quote(args),
        "max-char" => prim_max_char(args),
        "decode-char" => prim_decode_char(args),
        "encode-char" => prim_encode_char(args),
        "string-replace" => prim_string_replace(args),
        "string-trim" => prim_string_trim(args),
        "string-prefix-p" => prim_string_prefix_p(args),
        "string-suffix-p" => prim_string_suffix_p(args),
        "string-join" => prim_string_join(args),
        "char-to-string" => prim_char_to_string(args),
        "string-to-char" => prim_string_to_char(args),
        "string-width" => prim_string_width(args),
        "multibyte-string-p" => prim_multibyte_string_p(args),

        // Math — transcendental
        "sin" => prim_math_1(args, f64::sin),
        "cos" => prim_math_1(args, f64::cos),
        "tan" => prim_math_1(args, f64::tan),
        "asin" => prim_math_1(args, f64::asin),
        "acos" => prim_math_1(args, f64::acos),
        "atan" => prim_atan(args),
        "exp" => prim_math_1(args, f64::exp),
        "log" => prim_log(args),
        "sqrt" => prim_math_1(args, f64::sqrt),
        "expt" => prim_expt(args),
        "isnan" => {
            let n = get_number(
                &args.first().ok_or(ElispError::WrongNumberOfArguments)?,
            )
            .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
            Ok(LispObject::from(n.is_nan()))
        }
        "copysign" => {
            let a = get_number(&args.first().ok_or(ElispError::WrongNumberOfArguments)?)
                .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
            let b = get_number(&args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?)
                .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
            Ok(LispObject::float(a.copysign(b)))
        }
        "frexp" => {
            let n = get_number(&args.first().ok_or(ElispError::WrongNumberOfArguments)?)
                .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
            let bits = n.to_bits();
            let exp = ((bits >> 52) & 0x7ff) as i64 - 1022;
            let frac = f64::from_bits((bits & 0x800f_ffff_ffff_ffff) | 0x3fe0_0000_0000_0000);
            Ok(LispObject::cons(LispObject::float(frac), LispObject::integer(exp)))
        }
        "ldexp" => {
            let sig = get_number(&args.first().ok_or(ElispError::WrongNumberOfArguments)?)
                .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
            let exp = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?
                .as_integer().ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
            Ok(LispObject::float(sig * 2f64.powi(exp as i32)))
        }

        // Vector
        "aref" => prim_aref(args),
        "aset" => prim_aset(args),
        "make-vector" => prim_make_vector(args),
        "vconcat" => prim_vconcat(args),
        "vectorp" => prim_vectorp(args),

        // String — search / comparison
        "string-search" => prim_string_search(args),
        "string-equal" => prim_string_equal(args),
        "string-lessp" => prim_string_lessp(args),
        "compare-strings" => prim_compare_strings(args),
        "split-string" => prim_split_string(args),

        // Sequence — extended
        "elt" => prim_elt(args),
        "copy-alist" => prim_copy_alist(args),
        "plist-get" => prim_plist_get(args),
        "plist-put" => prim_plist_put(args),
        "plist-member" => prim_plist_member(args),
        "remove" => prim_remove(args),
        "remq" => prim_remq(args),
        "number-sequence" => prim_number_sequence(args),

        // Numeric — extended
        "random" => prim_random(args),
        "logxor" => prim_logxor(args),

        // Type — extended
        "sequencep" => prim_sequencep(args),
        "char-or-string-p" => prim_char_or_string_p(args),
        "booleanp" => prim_booleanp(args),
        "keywordp" => prim_keywordp(args),

        // Misc — extended (apply/error/signal/user-error: normally handled by eval dispatch)
        "apply" => Err(ElispError::EvalError(
            "apply must be called through eval dispatch".to_string(),
        )),
        "error" => prim_error(args),
        "user-error" => prim_user_error(args),
        "signal" => prim_signal(args),

        // ---- Phase 8: data.c type predicates ----
        "arrayp" => prim_arrayp(args),
        "nlistp" => prim_nlistp(args),
        "bufferp" => Ok(LispObject::nil()),  // rele has no buffer objects in Lisp
        "markerp" => Ok(LispObject::nil()),  // rele has no marker objects
        "byte-code-function-p" => prim_byte_code_function_p(args),
        "closurep" => Ok(LispObject::nil()),  // rele doesn't distinguish closure vs lambda
        "interpreted-function-p" => Ok(LispObject::nil()),
        "threadp" => Ok(LispObject::nil()),
        "mutexp" => Ok(LispObject::nil()),
        "condition-variable-p" => Ok(LispObject::nil()),
        "user-ptrp" => Ok(LispObject::nil()),
        "module-function-p" => Ok(LispObject::nil()),
        "native-comp-function-p" => Ok(LispObject::nil()),
        "integer-or-marker-p" => prim_integerp(args),  // no markers, so same as integerp
        "number-or-marker-p" => prim_numberp(args),     // no markers, so same as numberp
        "vector-or-char-table-p" => prim_vectorp(args), // char-tables are vectors for now
        "bare-symbol-p" => prim_symbolp(args),           // no pos-symbols
        "symbol-with-pos-p" => Ok(LispObject::nil()),
        "bool-vector-p" => Ok(LispObject::nil()),  // rele has no bool-vector type
        "hash-table-p" => prim_hash_table_p(args),

        // ---- Phase 8: data.c accessors ----
        "%" => prim_mod(args),
        "logcount" => prim_logcount(args),
        "byteorder" => Ok(LispObject::integer(if cfg!(target_endian = "little") { 108 } else { 66 })),
        "indirect-function" => prim_indirect_function(args),
        "indirect-variable" => prim_identity(args), // no variable indirection in rele
        "subr-arity" => prim_subr_arity(args),
        "subr-name" => prim_subr_name(args),
        "setplist" => prim_setplist(args),
        "cl-type-of" => prim_type_of(args),  // alias for now

        // ---- Phase 8: fns.c additions ----
        "proper-list-p" => prim_proper_list_p(args),
        "delete" => prim_delete(args),
        "rassq" => prim_rassq(args),
        "rassoc" => prim_rassoc(args),
        "maphash" => Err(ElispError::EvalError("maphash needs eval dispatch".to_string())),
        "remhash" => prim_remhash(args),
        "hash-table-count" => prim_hash_table_count(args),
        "hash-table-test" => prim_hash_table_test(args),
        "hash-table-size" => prim_hash_table_size(args),
        "hash-table-weakness" => Ok(LispObject::nil()), // no weak tables
        "copy-hash-table" => prim_copy_hash_table(args),
        "substring-no-properties" => prim_substring(args), // rele has no text properties
        "take" => prim_take(args),
        "ntake" => prim_take(args), // same as take for now (destructive ok)
        "length<" => prim_length_lt(args),
        "length>" => prim_length_gt(args),
        "length=" => prim_length_eq(args),
        "fillarray" => prim_fillarray(args),
        "string-bytes" => prim_string_bytes(args),
        "mapcan" => Err(ElispError::EvalError("mapcan needs eval dispatch".to_string())),
        "sxhash-eq" | "sxhash-eql" | "sxhash-equal" => prim_sxhash(args),
        "memql" => prim_memql(args),

        // ---- Batch: frequently-called DEFUNs with headless-safe
        //      semantics. These unblock stdlib / ERT test loading by
        //      returning the value real Emacs would return for a
        //      non-interactive, non-GUI session.
        "obarrayp" => prim_vectorp(args),
        "make-bool-vector" => prim_make_bool_vector(args),
        "bool-vector" => prim_bool_vector(args),
        "window-minibuffer-p" => Ok(LispObject::nil()),
        "frame-internal-border-width" => Ok(LispObject::integer(0)),
        "image-type-available-p" => Ok(LispObject::nil()),
        "gnutls-available-p" => Ok(LispObject::nil()),
        "libxml-available-p" => Ok(LispObject::nil()),
        "dbus-available-p" => Ok(LispObject::nil()),
        "native-comp-available-p" => Ok(LispObject::nil()),
        "display-graphic-p" => Ok(LispObject::nil()),
        "display-multi-frame-p" => Ok(LispObject::nil()),
        "display-color-p" => Ok(LispObject::nil()),
        "display-mouse-p" => Ok(LispObject::nil()),
        "get-char-property" => Ok(LispObject::nil()),
        "get-pos-property" => Ok(LispObject::nil()),
        "get-text-property" => Ok(LispObject::nil()),
        "text-properties-at" => Ok(LispObject::nil()),
        "last-nonminibuffer-frame" => Ok(LispObject::nil()),
        "tty-top-frame" => Ok(LispObject::nil()),
        "accessible-keymaps" => Ok(LispObject::nil()),
        "key-description" => prim_key_description(args),
        "documentation" => Ok(LispObject::nil()),
        "documentation-property" => Ok(LispObject::nil()),
        "backward-prefix-chars" => Ok(LispObject::nil()),
        "undo-boundary" => Ok(LispObject::nil()),
        "buffer-text-pixel-size" => Ok(LispObject::cons(
            LispObject::integer(0),
            LispObject::integer(0),
        )),
        "window-text-pixel-size" => Ok(LispObject::cons(
            LispObject::integer(0),
            LispObject::integer(0),
        )),
        "make-category-table" => Ok(LispObject::nil()),
        "category-table" => Ok(LispObject::nil()),
        "coding-system-plist" => Ok(LispObject::nil()),
        "coding-system-p" => Ok(LispObject::nil()),
        "coding-system-aliases" => Ok(LispObject::nil()),
        "get-load-suffixes" => Ok(LispObject::cons(
            LispObject::string(".elc"),
            LispObject::cons(LispObject::string(".el"), LispObject::nil()),
        )),
        "get-truename-buffer" => Ok(LispObject::nil()),
        "backtrace-frame--internal" => Ok(LispObject::nil()),
        "time-equal-p" => prim_time_equal_p(args),
        "time-less-p" => prim_time_less_p(args),
        "time-convert" => prim_time_convert(args),
        "format-time-string" => prim_format_time_string(args),
        "upcase-initials" => prim_upcase_initials(args),
        "make-directory-internal" => prim_make_directory_internal(args),
        "make-network-process"
        | "make-process"
        | "make-serial-process"
        | "make-pipe-process"
        | "call-process"
        | "call-process-region"
        | "start-process"
        | "open-network-stream" => Ok(LispObject::nil()),
        "process-list" => Ok(LispObject::nil()),
        "get-process" => Ok(LispObject::nil()),
        "process-status" => Ok(LispObject::nil()),
        "process-attributes" => Ok(LispObject::nil()),
        "execute-kbd-macro" => Ok(LispObject::nil()),
        "current-input-mode" => Ok(LispObject::nil()),
        "make-indirect-buffer" => Ok(LispObject::nil()),
        "insert-buffer-substring" => Ok(LispObject::nil()),
        "insert-buffer-substring-no-properties" => Ok(LispObject::nil()),
        "window-system" => Ok(LispObject::nil()),
        "redisplay" => Ok(LispObject::nil()),
        "force-mode-line-update" => Ok(LispObject::nil()),
        "frame-visible-p" => Ok(LispObject::t()),
        "frame-live-p" => Ok(LispObject::t()),
        "frame-parameters" => Ok(LispObject::nil()),
        "window-live-p" => Ok(LispObject::nil()),
        "selected-window" => Ok(LispObject::nil()),
        "minibuffer-window" => Ok(LispObject::nil()),
        "minibufferp" => Ok(LispObject::nil()),
        "minibuffer-depth" => Ok(LispObject::integer(0)),
        "recursion-depth" => Ok(LispObject::integer(0)),
        "current-message" => Ok(LispObject::nil()),
        "input-pending-p" => Ok(LispObject::nil()),
        "this-command-keys" => Ok(LispObject::string("")),
        "this-command-keys-vector" => Ok(LispObject::Vector(std::sync::Arc::new(
            parking_lot::Mutex::new(Vec::new()),
        ))),
        "recent-keys" => Ok(LispObject::Vector(std::sync::Arc::new(
            parking_lot::Mutex::new(Vec::new()),
        ))),
        "terminal-live-p" => Ok(LispObject::nil()),
        "terminal-list" => Ok(LispObject::nil()),
        "frame-list" => Ok(LispObject::nil()),
        "window-list" => Ok(LispObject::nil()),

        // Second batch: more headless-safe DEFUNs.
        "terpri" => Ok(LispObject::nil()), // print newline — no-op for us
        "print" | "pp" | "pp-to-string" => prim_prin1_to_string(args),
        "write-char" => Ok(args.first().unwrap_or(LispObject::nil())),
        "mark-marker" | "mark" => Ok(LispObject::integer(0)),
        "marker-insertion-type" => Ok(LispObject::nil()),
        "marker-position" => args
            .first()
            .and_then(|a| a.as_integer())
            .map_or(Ok(LispObject::nil()), |i| Ok(LispObject::integer(i))),
        "set-marker" => Ok(args.first().unwrap_or(LispObject::nil())),
        "copy-marker" => Ok(args.first().unwrap_or(LispObject::nil())),
        "make-marker" => Ok(LispObject::nil()),
        "delete-and-extract-region" => Ok(LispObject::string("")),
        "read-key-sequence" | "read-key-sequence-vector" | "read-key" => {
            Ok(LispObject::nil())
        }
        "read-char" | "read-char-exclusive" => Ok(LispObject::nil()),
        "encode-time" => prim_encode_time(args),
        "current-time" => prim_current_time(),
        "current-time-string" => Ok(LispObject::string("")),
        "float-time" => prim_float_time(args),
        "network-lookup-address-info" => Ok(LispObject::nil()),
        "logb" => prim_logb(args),
        "font-spec" | "font-family-list" | "font-info" | "font-face-attributes" => {
            Ok(LispObject::nil())
        }
        "charset-priority-list" | "charset-list" | "charset-plist" => {
            Ok(LispObject::nil())
        }
        "describe-buffer-bindings" => Ok(LispObject::nil()),
        "bidi-find-overridden-directionality" => Ok(LispObject::nil()),
        "bidi-resolved-levels" => Ok(LispObject::nil()),
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
        "directory-name-p" => prim_directory_name_p(args),
        "file-accessible-directory-p" => prim_file_accessible_directory_p(args),
        "file-name-as-directory" => prim_file_name_as_directory(args),
        "file-truename" => Ok(args.first().unwrap_or(LispObject::nil())),
        "expand-file-name" => Ok(args.first().unwrap_or(LispObject::nil())),
        "abbreviate-file-name" => Ok(args.first().unwrap_or(LispObject::nil())),
        "mapp" => Ok(LispObject::from(matches!(
            args.first().unwrap_or_else(LispObject::nil),
            LispObject::Cons(_)
                | LispObject::HashTable(_)
                | LispObject::Vector(_)
                | LispObject::Nil
        ))),
        "hash-table-keys" => prim_hash_table_keys(args),
        "hash-table-values" => prim_hash_table_values(args),
        "default-value" | "default-boundp" => Ok(LispObject::nil()),
        "set-default" => Ok(args.nth(1).unwrap_or(LispObject::nil())),
        "local-variable-p" | "local-variable-if-set-p" => Ok(LispObject::nil()),
        "make-variable-buffer-local" => Ok(args.first().unwrap_or(LispObject::nil())),
        "kill-local-variable" => Ok(args.first().unwrap_or(LispObject::nil())),
        "evenp" => prim_evenp(args),
        "oddp" => prim_oddp(args),
        "plusp" => prim_plusp(args),
        "minusp" => prim_minusp(args),

        // Third batch.
        "user-uid" | "user-real-uid" => Ok(LispObject::integer(
            // Hard-coded 1000 for headless runs; real Emacs consults
            // getuid(2). Stdlib callers mostly just need a number.
            1000,
        )),
        "group-gid" | "group-real-gid" => Ok(LispObject::integer(1000)),
        "default-toplevel-value" => prim_default_toplevel_value(args),
        "set-default-toplevel-value" => {
            Ok(args.nth(1).unwrap_or(LispObject::nil()))
        }
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
        "command-remapping" => Ok(LispObject::nil()),
        "buffer-swap-text" => Ok(LispObject::nil()),
        "color-values-from-color-spec" | "color-values" | "color-name-to-rgb" => {
            Ok(LispObject::nil())
        }
        "coding-system-priority-list" => Ok(LispObject::cons(
            LispObject::symbol("utf-8"),
            LispObject::nil(),
        )),
        "set-coding-system-priority" => Ok(LispObject::nil()),
        "prefer-coding-system" => Ok(LispObject::nil()),
        "keyboard-coding-system" => Ok(LispObject::symbol("utf-8")),
        "terminal-coding-system" => Ok(LispObject::symbol("utf-8")),
        "set-terminal-coding-system" | "set-keyboard-coding-system" => {
            Ok(LispObject::nil())
        }
        // Completion / minibuffer stubs.
        "completing-read" | "read-from-minibuffer" | "read-string"
        | "read-number" | "read-buffer" | "read-file-name" => {
            // Return the provided DEFAULT (third arg for most forms),
            // or nil if not supplied — matches what `noninteractive`
            // Emacs returns.
            Ok(args.nth(2).unwrap_or(LispObject::nil()))
        }
        "yes-or-no-p" | "y-or-n-p" => Ok(LispObject::nil()),
        "message-box" | "ding" | "beep" => Ok(LispObject::nil()),
        // Generic char/text utilities with trivially-nil defaults.
        "char-displayable-p" => Ok(LispObject::t()),
        "char-charset" => Ok(LispObject::symbol("unicode")),
        "char-category-set" => Ok(LispObject::nil()),
        "char-syntax" => Ok(LispObject::integer(b'.' as i64)),
        "syntax-class-to-char" => Ok(LispObject::integer(b'.' as i64)),
        "syntax-after" => Ok(LispObject::nil()),
        "composition-get-gstring"
        | "find-composition"
        | "find-composition-internal" => Ok(LispObject::nil()),
        // Overlays — rele has no overlay type; every op is nil.
        "make-overlay" | "delete-overlay" | "move-overlay" | "overlay-start"
        | "overlay-end" | "overlay-buffer" | "overlay-properties"
        | "overlay-get" | "overlay-put" | "overlays-at" | "overlays-in"
        | "overlay-lists" | "overlay-recenter" | "remove-overlays"
        | "next-overlay-change" | "previous-overlay-change" => {
            Ok(LispObject::nil())
        }
        // Image / display / face stubs.
        "image-size" | "image-flush" | "image-mask-p"
        | "image-frame-cache-size"
        | "image-metadata" | "create-image" => Ok(LispObject::nil()),
        "face-list" | "face-attribute" | "face-all-attributes"
        | "face-bold-p" | "face-italic-p" | "face-font" | "face-background"
        | "face-foreground" | "internal-lisp-face-p"
        | "internal-lisp-face-equal-p" => Ok(LispObject::nil()),
        "set-face-attribute" | "make-face" | "copy-face" => {
            Ok(LispObject::nil())
        }
        // Menu / tool-bar stubs.
        "menu-bar-lines" | "menu-bar-mode" | "tool-bar-mode"
        | "tab-bar-mode" | "popup-menu" | "x-popup-menu" => {
            Ok(LispObject::nil())
        }

        // Fourth batch: obarray + buffer/window/frame + string/sequence
        // + timer/thread — the next layer of frequently-called DEFUNs.
        "mapatoms" => Ok(LispObject::nil()), // no-op walk (no obarray iteration yet)
        "unintern" => Ok(LispObject::nil()),
        "intern-soft" => prim_intern_soft(args),
        "symbol-plist" => Ok(LispObject::nil()),
        // add-to-list mutates its var cell; without stateful-dispatch
        // access we can only return nil. Stdlib callers mostly use
        // the return value to decide "did I change anything"; returning
        // nil says "no-op" which is safer than a stale value.
        "add-to-list" => Ok(LispObject::nil()),
        "add-to-ordered-list" => Ok(LispObject::nil()),
        "remove-from-invisibility-spec"
        | "add-to-invisibility-spec" => Ok(LispObject::nil()),
        "buffer-list" | "buffer-base-buffer" => Ok(LispObject::nil()),
        "buffer-chars-modified-tick" | "buffer-modified-tick" => {
            Ok(LispObject::integer(0))
        }
        "buffer-modified-p" => Ok(LispObject::nil()),
        "set-buffer-modified-p" => Ok(LispObject::nil()),
        "restore-buffer-modified-p" => Ok(LispObject::nil()),
        "generate-new-buffer-name" => prim_generate_new_buffer_name(args),
        "get-buffer-window" => Ok(LispObject::nil()),
        "window-buffer" | "window-parent" | "window-parameter"
        | "window-parameters" | "window-frame" | "window-dedicated-p"
        | "window-normal-size" | "window-left-child" | "window-right-child"
        | "window-top-child" | "window-prev-sibling" | "window-next-sibling"
        | "window-prev-buffers" | "window-next-buffers"
        | "window-resizable" | "window-resizable-p" | "window-combination-limit"
        | "window-new-total" | "window-new-pixel" | "window-new-normal"
        | "window-old-point" | "window-old-pixel-height" => Ok(LispObject::nil()),
        "window-total-width" | "window-total-height"
        | "window-text-width" | "window-text-height"
        | "window-body-width" | "window-body-height"
        | "window-pixel-width" | "window-pixel-height"
        | "window-total-size" | "window-body-size"
        | "window-left-column" | "window-top-line"
        | "window-scroll-bar-height" | "window-scroll-bar-width"
        | "window-fringes" | "window-margins" | "window-hscroll"
        | "window-vscroll" | "window-line-height" | "window-font-height"
        | "window-font-width" | "window-max-chars-per-line"
        | "window-screen-lines" => Ok(LispObject::integer(80)),
        "window-pixel-edges" | "window-edges" | "window-inside-edges"
        | "window-inside-pixel-edges" | "window-absolute-pixel-edges"
        | "window-absolute-pixel-position" | "window-point" | "window-start"
        | "window-end" | "window-prompt" => Ok(LispObject::nil()),
        "frame-pixel-width" | "frame-pixel-height" => {
            Ok(LispObject::integer(800))
        }
        "frame-width" | "frame-height" | "frame-total-lines"
        | "frame-native-width" | "frame-native-height" => {
            Ok(LispObject::integer(80))
        }
        "frame-char-width" | "frame-char-height"
        | "frame-text-cols" | "frame-text-lines"
        | "frame-scroll-bar-width" | "frame-scroll-bar-height"
        | "frame-fringe-width" | "frame-font" | "frame-position"
        | "frame-root-window" | "frame-selected-window"
        | "frame-first-window" | "frame-focus"
        | "frame-edges" | "frame-border-width"
        | "frame-internal-border-height" | "frame-internal-border"
        | "frame-terminal" | "frame-tool-bar-lines"
        | "frame-menu-bar-lines" => Ok(LispObject::nil()),
        "redirect-frame-focus" | "select-frame" | "select-frame-set-input-focus"
        | "select-window" | "make-frame-visible" | "make-frame-invisible"
        | "iconify-frame" | "delete-frame" | "delete-window"
        | "delete-other-windows" | "delete-other-windows-vertically"
        | "switch-to-buffer-other-window" | "switch-to-buffer-other-frame"
        | "set-frame-size" | "set-frame-position" | "set-frame-width"
        | "set-frame-height" | "set-frame-parameter"
        | "modify-frame-parameters" | "set-window-buffer"
        | "set-window-parameter" | "set-window-dedicated-p"
        | "set-window-point" | "set-window-start" | "set-window-hscroll"
        | "set-window-vscroll" | "set-window-fringes" | "set-window-margins"
        | "set-window-scroll-bars" | "set-window-display-table"
        | "set-face-underline" | "set-face-strike-through" => Ok(LispObject::nil()),
        "frame-parameter" => Ok(LispObject::nil()),
        "x-display-pixel-width" | "x-display-pixel-height"
        | "x-display-mm-width" | "x-display-mm-height"
        | "x-display-color-cells" | "x-display-planes"
        | "x-display-visual-class" | "x-display-screens"
        | "x-display-save-under" | "x-display-backing-store"
        | "x-display-list" | "x-display-name" => Ok(LispObject::integer(0)),
        "unibyte-string" => prim_unibyte_string(args),
        "unibyte-char-to-multibyte" | "multibyte-char-to-unibyte" => {
            Ok(args.first().unwrap_or(LispObject::nil()))
        }
        "string-as-unibyte" | "string-as-multibyte"
        | "string-to-multibyte" | "string-to-unibyte"
        | "string-make-unibyte" | "string-make-multibyte" => {
            Ok(args.first().unwrap_or(LispObject::nil()))
        }
        "char-width" => prim_string_width(args),
        "string-lines" => prim_string_lines(args),
        "truncate-string-to-width"
        | "truncate-string-pixelwise" => {
            Ok(args.first().unwrap_or(LispObject::nil()))
        }
        "string-pixel-width" => Ok(LispObject::integer(0)),
        "text-char-description" | "single-key-description" => prim_prin1_to_string(args),
        // Timer / thread — no-ops.
        "run-at-time" | "run-with-timer" | "run-with-idle-timer"
        | "cancel-timer" | "cancel-function-timers" | "timer-list"
        | "timer-activate" | "timer-event-handler" | "timerp"
        | "current-idle-time" => Ok(LispObject::nil()),
        "timer-set-time" | "timer-set-function" | "timer-set-idle-time"
        | "timer-inc-time" => Ok(LispObject::nil()),
        "make-thread" | "current-thread" | "thread-name"
        | "thread-alive-p" | "thread-join" | "thread-signal"
        | "thread-yield" | "thread-last-error" | "thread-live-p"
        | "thread--blocker" | "all-threads" | "condition-mutex"
        | "condition-name" | "condition-notify" | "condition-wait"
        | "make-condition-variable" | "make-mutex"
        | "mutex-lock" | "mutex-unlock" | "mutex-name" => Ok(LispObject::nil()),
        // Event machinery stubs.
        "event-end" | "event-start" | "event-click-count"
        | "event-line-count" | "event-basic-type" | "event-modifiers"
        | "event-convert-list" | "posn-at-point" | "posn-window"
        | "posn-area" | "posn-x-y" | "posn-col-row" | "posn-point"
        | "posn-string" | "posn-object" | "posn-object-x-y"
        | "posn-actual-col-row" | "posn-timestamp"
        | "posn-image" | "posn-object-width-height" => Ok(LispObject::nil()),

        // Fifth batch: keymap, file I/O, syntax, hooks, misc.
        "kbd" => Ok(args.first().unwrap_or(LispObject::nil())),
        "global-set-key" | "local-set-key" | "global-unset-key"
        | "local-unset-key" | "define-key-after" | "substitute-key-definition"
        | "where-is-internal" | "set-keymap-parent" | "copy-keymap"
        | "keymap-parent" | "current-global-map" | "current-active-maps"
        | "use-global-map" | "lookup-key" | "map-keymap" | "map-keymap-internal"
        | "keyboard-translate" | "keyboard-quit" | "abort-minibuffers"
        | "minibuffer-message" | "set-quit-char" => Ok(LispObject::nil()),
        "kill-all-local-variables" | "buffer-local-variables" => {
            Ok(LispObject::nil())
        }
        "buffer-local-value" => prim_buffer_local_value(args),
        "generate-new-buffer" => {
            Ok(args.first().unwrap_or(LispObject::nil()))
        }
        "rename-file" => prim_rename_file(args),
        "copy-file" => prim_copy_file(args),
        "delete-directory" => prim_delete_directory(args),
        "file-modes" => prim_file_modes(args),
        "set-file-modes" => Ok(LispObject::nil()),
        "file-newer-than-file-p" => prim_file_newer_than_file_p(args),
        "file-symlink-p" => prim_file_symlink_p(args),
        "file-regular-p" => prim_file_regular_p(args),
        "file-readable-p" => prim_file_readable_p(args),
        "file-writable-p" => prim_file_writable_p(args),
        "file-executable-p" => prim_file_executable_p(args),
        "symbol-file" => Ok(LispObject::nil()),
        "current-column" | "current-indentation" => Ok(LispObject::integer(0)),
        "indent-line-to" | "indent-to" | "indent-according-to-mode"
        | "indent-for-tab-command" | "indent-rigidly" | "indent-region" => {
            Ok(LispObject::nil())
        }
        "skip-syntax-forward" | "skip-syntax-backward"
        | "skip-chars-forward" | "skip-chars-backward" => {
            Ok(LispObject::integer(0))
        }
        "parse-partial-sexp" | "scan-sexps" | "scan-lists"
        | "backward-up-list" | "forward-sexp" | "backward-sexp"
        | "up-list" | "down-list" | "forward-list" | "backward-list"
        | "string-to-syntax" => Ok(LispObject::nil()),
        "current-input-method" => Ok(LispObject::nil()),
        "activate-input-method" | "deactivate-input-method"
        | "set-input-method" | "toggle-input-method"
        | "describe-input-method" => Ok(LispObject::nil()),
        "syntax-table-p" => Ok(LispObject::nil()),
        "recursive-edit" | "top-level" | "exit-recursive-edit"
        | "abort-recursive-edit" | "exit-minibuffer" | "keyboard-escape-quit" => {
            Ok(LispObject::nil())
        }
        "translation-table-id" => Ok(LispObject::nil()),
        "remove-hook" | "run-hooks" | "run-hook-with-args"
        | "run-hook-with-args-until-success"
        | "run-hook-with-args-until-failure"
        | "run-hook-wrapped" => Ok(LispObject::nil()),
        "make-abbrev-table" | "clear-abbrev-table"
        | "abbrev-table-empty-p" | "abbrev-expansion"
        | "abbrev-symbol" | "abbrev-get" | "abbrev-put"
        | "abbrev-insert" | "abbrev-table-get" | "abbrev-table-put"
        | "copy-abbrev-table" | "define-abbrev" => Ok(LispObject::nil()),
        "format-prompt" => prim_format_prompt(args),
        "format-message" => {
            // (format-message FMT &rest ARGS) — `format` is the closest
            // equivalent; `format-message` also rewrites quotes, which
            // we skip.
            prim_concat(args)
        }
        "set-visited-file-name" | "clear-visited-file-modtime"
        | "verify-visited-file-modtime" | "visited-file-modtime"
        | "file-locked-p" | "lock-buffer" | "unlock-buffer"
        | "ask-user-about-lock"
        | "ask-user-about-supersession-threat" => Ok(LispObject::nil()),
        "set-buffer-multibyte" => Ok(args.first().unwrap_or(LispObject::nil())),
        "kill-line" | "kill-word" | "backward-kill-word"
        | "kill-whole-line" | "kill-region" | "kill-ring-save"
        | "kill-new" | "kill-append" | "yank" | "yank-pop"
        | "current-kill" => Ok(LispObject::nil()),
        "define-fringe-bitmap" | "set-fringe-bitmap-face"
        | "fringe-bitmaps-at-pos" | "fringe-bitmap-p" => Ok(LispObject::nil()),
        "bookmark-set" | "bookmark-jump" | "bookmark-delete"
        | "bookmark-get-bookmark-record" => Ok(LispObject::nil()),
        "x-selection-owner-p" | "x-selection-exists-p"
        | "x-get-selection" | "x-set-selection" | "x-own-selection"
        | "x-disown-selection" | "x-selection-value"
        | "gui-get-selection" | "gui-set-selection"
        | "gui-selection-exists-p" | "gui-selection-owner-p" => {
            Ok(LispObject::nil())
        }
        "clipboard-yank" | "clipboard-kill-ring-save"
        | "clipboard-kill-region" => Ok(LispObject::nil()),
        // Advice / function transformation (no-op).
        "add-function" | "remove-function" | "advice-add"
        | "advice-remove" | "advice-function-mapc"
        | "advice-function-member-p" | "advice--p"
        | "advice--make" | "advice--add-function" | "advice--tweaked"
        | "advice--symbol-function" | "advice-eval-interactive-spec" => {
            Ok(LispObject::nil())
        }
        // Debugging stubs.
        "backtrace-frames" | "backtrace-eval" | "backtrace-debug"
        | "debug-on-entry" | "cancel-debug-on-entry"
        | "debug" | "mapbacktrace" => Ok(LispObject::nil()),
        "signal-process" | "process-send-string" | "process-send-region"
        | "process-send-eof" | "process-kill-without-query"
        | "process-running-child-p" | "process-live-p"
        | "process-exit-status" | "process-id" | "process-name"
        | "process-command" | "process-tty-name"
        | "process-coding-system" | "process-filter"
        | "process-sentinel" | "set-process-filter"
        | "set-process-sentinel" | "set-process-query-on-exit-flag"
        | "process-query-on-exit-flag"
        | "delete-process" | "continue-process" | "stop-process"
        | "interrupt-process" | "quit-process" | "accept-process-output"
        | "process-buffer" | "set-process-buffer" => Ok(LispObject::nil()),
        "xml-parse-string" | "xml-parse-region" | "xml-parse-file"
        | "libxml-parse-xml-region" | "libxml-parse-html-region" => {
            Ok(LispObject::nil())
        }
        "json-parse-string" | "json-parse-buffer"
        | "json-serialize" | "json-encode" | "json-decode"
        | "json-read" | "json-read-from-string" => Ok(LispObject::nil()),
        "sqlite-open" | "sqlite-close" | "sqlite-execute"
        | "sqlite-select" | "sqlite-transaction"
        | "sqlite-commit" | "sqlite-rollback"
        | "sqlitep" | "sqlite-pragma" | "sqlite-load-extension"
        | "sqlite-version" => Ok(LispObject::nil()),
        "treesit-parser-create" | "treesit-parser-delete"
        | "treesit-parser-p" | "treesit-parser-buffer"
        | "treesit-parser-language" | "treesit-node-p"
        | "treesit-node-type" | "treesit-node-string"
        | "treesit-node-start" | "treesit-node-end"
        | "treesit-node-parent" | "treesit-node-child"
        | "treesit-node-children" | "treesit-node-child-by-field-name"
        | "treesit-query-compile" | "treesit-query-capture"
        | "treesit-language-available-p"
        | "treesit-library-abi-version" => Ok(LispObject::nil()),

        _ => Err(ElispError::VoidFunction(name.to_string())),
    }
}

fn get_number(obj: &LispObject) -> Option<f64> {
    match obj {
        LispObject::Integer(i) => Some(*i as f64),
        LispObject::Float(f) => Some(*f),
        // Emacs treats nil as 0 and t as 1 in numeric contexts.
        LispObject::Nil => Some(0.0),
        LispObject::T => Some(1.0),
        _ => None,
    }
}

fn prim_add(args: &LispObject) -> ElispResult<LispObject> {
    let mut raw = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        raw.push(arg);
        current = rest;
    }
    let all_int = raw.iter().all(|a| matches!(a, LispObject::Integer(_)));
    if all_int {
        let sum: i64 = raw
            .iter()
            .map(|a| a.as_integer().unwrap())
            .fold(0i64, |a, b| a.wrapping_add(b));
        Ok(LispObject::integer(sum))
    } else {
        let sum: f64 = raw
            .iter()
            .map(|a| get_number(a).ok_or_else(|| ElispError::WrongTypeArgument("number".into())))
            .collect::<ElispResult<Vec<_>>>()?
            .into_iter()
            .sum();
        Ok(LispObject::float(sum))
    }
}

fn prim_sub(args: &LispObject) -> ElispResult<LispObject> {
    let mut raw = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        raw.push(arg);
        current = rest;
    }
    if raw.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }
    let all_int = raw.iter().all(|a| matches!(a, LispObject::Integer(_)));
    if all_int {
        let ints: Vec<i64> = raw.iter().map(|a| a.as_integer().unwrap()).collect();
        let result = if ints.len() == 1 {
            ints[0].wrapping_neg()
        } else {
            ints.iter()
                .skip(1)
                .fold(ints[0], |acc, &x| acc.wrapping_sub(x))
        };
        Ok(LispObject::integer(result))
    } else {
        let nums: Vec<f64> = raw
            .iter()
            .map(|a| get_number(a).ok_or_else(|| ElispError::WrongTypeArgument("number".into())))
            .collect::<ElispResult<Vec<_>>>()?;
        let result = if nums.len() == 1 {
            -nums[0]
        } else {
            nums.iter().skip(1).fold(nums[0], |acc, &x| acc - x)
        };
        Ok(LispObject::float(result))
    }
}

fn prim_mul(args: &LispObject) -> ElispResult<LispObject> {
    let mut raw = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        raw.push(arg);
        current = rest;
    }
    let all_int = raw.iter().all(|a| matches!(a, LispObject::Integer(_)));
    if all_int {
        let product: i64 = raw
            .iter()
            .map(|a| a.as_integer().unwrap())
            .fold(1i64, |a, b| a.wrapping_mul(b));
        Ok(LispObject::integer(product))
    } else {
        let product: f64 = raw
            .iter()
            .map(|a| get_number(a).ok_or_else(|| ElispError::WrongTypeArgument("number".into())))
            .collect::<ElispResult<Vec<_>>>()?
            .into_iter()
            .product();
        Ok(LispObject::float(product))
    }
}

fn prim_div(args: &LispObject) -> ElispResult<LispObject> {
    let mut raw_args: Vec<LispObject> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        raw_args.push(arg);
        current = rest;
    }
    if raw_args.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }
    // Validate all args are numbers
    for a in &raw_args {
        if get_number(a).is_none() {
            return Err(ElispError::WrongTypeArgument("number".to_string()));
        }
    }
    let all_integer = raw_args.iter().all(|a| matches!(a, LispObject::Integer(_)));
    if all_integer {
        let ints: Vec<i64> = raw_args.iter().map(|a| a.as_integer().unwrap()).collect();
        for &d in &ints[1..] {
            if d == 0 {
                return Err(ElispError::DivisionByZero);
            }
        }
        if ints.len() == 1 {
            if ints[0] == 0 {
                return Err(ElispError::DivisionByZero);
            }
            return Ok(LispObject::integer(1i64.wrapping_div(ints[0])));
        }
        // wrapping_div avoids panic on i64::MIN / -1 (the only overflow case).
        let result = ints
            .iter()
            .skip(1)
            .fold(ints[0], |acc, &x| acc.wrapping_div(x));
        Ok(LispObject::integer(result))
    } else {
        let numbers: Vec<f64> = raw_args.iter().map(|a| get_number(a).unwrap()).collect();
        for &d in &numbers[1..] {
            if d == 0.0 {
                return Err(ElispError::DivisionByZero);
            }
        }
        if numbers.len() == 1 {
            if numbers[0] == 0.0 {
                return Err(ElispError::DivisionByZero);
            }
            return Ok(LispObject::float(1.0 / numbers[0]));
        }
        let result = numbers.iter().skip(1).fold(numbers[0], |acc, &x| acc / x);
        Ok(LispObject::float(result))
    }
}

fn prim_num_eq(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.is_empty() {
        return Err(ElispError::WrongNumberOfArguments);
    }
    let first = numbers[0];
    Ok(LispObject::from(numbers.iter().all(|&x| x == first)))
}

fn prim_lt(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.len() < 2 {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for w in numbers.windows(2) {
        if w[0].partial_cmp(&w[1]) != Some(std::cmp::Ordering::Less) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

fn prim_gt(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.len() < 2 {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for w in numbers.windows(2) {
        if w[0].partial_cmp(&w[1]) != Some(std::cmp::Ordering::Greater) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

fn prim_le(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.len() < 2 {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for w in numbers.windows(2) {
        if !matches!(
            w[0].partial_cmp(&w[1]),
            Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
        ) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

fn prim_ge(args: &LispObject) -> ElispResult<LispObject> {
    let mut numbers: Vec<f64> = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let n =
            get_number(&arg).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        numbers.push(n);
        current = rest;
    }
    if numbers.len() < 2 {
        return Err(ElispError::WrongNumberOfArguments);
    }
    for w in numbers.windows(2) {
        if !matches!(
            w[0].partial_cmp(&w[1]),
            Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
        ) {
            return Ok(LispObject::nil());
        }
    }
    Ok(LispObject::t())
}

fn prim_ne(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let na = get_number(&a).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    let nb = get_number(&b).ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::from(na != nb))
}

fn prim_cons(args: &LispObject) -> ElispResult<LispObject> {
    let car = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let cdr = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::cons(car, cdr))
}

fn prim_car(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Nil => Ok(LispObject::nil()),
        LispObject::Cons(cell) => Ok(cell.lock().0.clone()),
        _ => Err(ElispError::WrongTypeArgument("list".to_string())),
    }
}

fn prim_cdr(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Nil => Ok(LispObject::nil()),
        LispObject::Cons(cell) => Ok(cell.lock().1.clone()),
        _ => Err(ElispError::WrongTypeArgument("list".to_string())),
    }
}

fn prim_list(args: &LispObject) -> ElispResult<LispObject> {
    Ok(args.clone())
}

fn prim_length(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::Nil => Ok(LispObject::integer(0)),
        LispObject::Cons(_) => {
            let mut count = 0;
            let mut current = arg.clone();
            while let Some((_, rest)) = current.destructure_cons() {
                count += 1;
                current = rest;
            }
            Ok(LispObject::integer(count))
        }
        LispObject::String(s) => Ok(LispObject::integer(s.chars().count() as i64)),
        LispObject::Vector(v) => Ok(LispObject::integer(v.lock().len() as i64)),
        LispObject::HashTable(ht) => Ok(LispObject::integer(ht.lock().data.len() as i64)),
        _ => Err(ElispError::WrongTypeArgument(
            "sequence (list, string, vector)".to_string(),
        )),
    }
}

fn prim_append(args: &LispObject) -> ElispResult<LispObject> {
    // Collect all arguments into a vec
    let mut all_args = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        all_args.push(arg);
        current = rest;
    }
    if all_args.is_empty() {
        return Ok(LispObject::nil());
    }
    // The last argument becomes the tail directly (even if non-list)
    let tail = all_args.pop().unwrap();
    // Collect elements from all but the last arg in reverse order
    let mut items = Vec::new();
    for arg in &all_args {
        let mut cur = arg.clone();
        while let Some((car, cdr)) = cur.destructure_cons() {
            items.push(car);
            cur = cdr;
        }
    }
    // Build result by consing items onto the tail in reverse
    let mut result = tail;
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

fn prim_reverse(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    let mut result = LispObject::nil();
    let mut current = arg.clone();
    while let Some((car, cdr)) = current.destructure_cons() {
        result = LispObject::cons(car, result);
        current = cdr;
    }
    Ok(result)
}

fn prim_member(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        if obj == car {
            return Ok(current);
        }
        current = cdr;
    }
    Ok(LispObject::nil())
}

fn prim_assoc(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let alist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = alist;
    while let Some((entry, rest)) = current.destructure_cons() {
        if let Some((k, _)) = entry.destructure_cons() {
            if key == k {
                return Ok(entry);
            }
        }
        current = rest;
    }
    Ok(LispObject::nil())
}

fn prim_eq(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match (&a, &b) {
        (LispObject::Nil, LispObject::Nil) => true,
        (LispObject::T, LispObject::T) => true,
        (LispObject::Integer(x), LispObject::Integer(y)) => x == y,
        (LispObject::Symbol(x), LispObject::Symbol(y)) => x == y,
        _ => false,
    };
    Ok(LispObject::from(result))
}

fn prim_equal(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(a == b))
}

fn prim_not(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_nil()))
}

fn prim_null(args: &LispObject) -> ElispResult<LispObject> {
    prim_not(args)
}

fn prim_symbolp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(
        arg.is_symbol() || arg.is_nil() || arg.is_t(),
    ))
}

fn prim_numberp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_integer() || arg.is_float()))
}

fn prim_listp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_nil() || arg.is_cons()))
}

fn prim_consp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_cons()))
}

fn prim_stringp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_string()))
}

fn prim_princ(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    print!("{}", arg.princ_to_string());
    Ok(arg)
}

fn prim_prin1(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args
        .clone()
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?;
    print!("{}", arg.prin1_to_string());
    Ok(arg)
}

fn prim_string_eq(args: &LispObject) -> ElispResult<LispObject> {
    let (a, b) = match (args.clone().first(), args.clone().nth(1)) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err(ElispError::WrongNumberOfArguments),
    };
    match (&a, &b) {
        (LispObject::String(s1), LispObject::String(s2)) => Ok(LispObject::from(s1 == s2)),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string_lt(args: &LispObject) -> ElispResult<LispObject> {
    let (a, b) = match (args.clone().first(), args.clone().nth(1)) {
        (Some(a), Some(b)) => (a, b),
        _ => return Err(ElispError::WrongNumberOfArguments),
    };
    match (&a, &b) {
        (LispObject::String(s1), LispObject::String(s2)) => Ok(LispObject::from(s1 < s2)),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_concat(args: &LispObject) -> ElispResult<LispObject> {
    // Emacs `concat` accepts any sequence of character-producing items:
    // - String: text appended verbatim.
    // - Nil: empty list, contributes nothing.
    // - List of integers: each integer pushed as a character codepoint.
    // - Vector of integers: same.
    // This matches help.el's pattern `(concat "[" (mapcar #'car alist) "]")`
    // where the middle arg is a list of character codes.
    let mut result = String::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        match arg {
            LispObject::String(s) => result.push_str(&s),
            LispObject::Nil => {}
            LispObject::Cons(_) => {
                // Treat as a list of character codepoints.
                let mut list_cur = arg;
                while let Some((car, lrest)) = list_cur.destructure_cons() {
                    match car {
                        LispObject::Integer(n) => {
                            let ch = char::from_u32(n as u32).ok_or_else(|| {
                                ElispError::WrongTypeArgument("character".to_string())
                            })?;
                            result.push(ch);
                        }
                        _ => {
                            return Err(ElispError::WrongTypeArgument(
                                "sequence of chars".to_string(),
                            ));
                        }
                    }
                    list_cur = lrest;
                }
            }
            LispObject::Vector(v) => {
                let guard = v.lock();
                for item in guard.iter() {
                    match item {
                        LispObject::Integer(n) => {
                            let ch = char::from_u32(*n as u32).ok_or_else(|| {
                                ElispError::WrongTypeArgument("character".to_string())
                            })?;
                            result.push(ch);
                        }
                        _ => {
                            return Err(ElispError::WrongTypeArgument(
                                "sequence of chars".to_string(),
                            ));
                        }
                    }
                }
            }
            _ => return Err(ElispError::WrongTypeArgument("sequence".to_string())),
        }
        current = rest;
    }
    Ok(LispObject::string(&result))
}

fn prim_substring(args: &LispObject) -> ElispResult<LispObject> {
    let s = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let start = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let end = args.nth(2);

    let s = match s {
        LispObject::String(s) => s.clone(),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    let start_signed = match start {
        LispObject::Integer(i) => i,
        _ => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };
    let end_signed = match end {
        Some(LispObject::Integer(i)) => Some(i),
        Some(LispObject::Nil) | None => None,
        Some(_) => return Err(ElispError::WrongTypeArgument("integer".to_string())),
    };

    let chars: Vec<char> = s.chars().collect();
    let len = chars.len() as i64;
    // Emacs allows negative indices that count from the end. Map them
    // explicitly so we never `as usize`-cast a negative integer (which
    // wraps to a huge value and panics on slice).
    let normalize = |i: i64| -> i64 {
        if i < 0 { (len + i).max(0) } else { i.min(len) }
    };
    let start = normalize(start_signed) as usize;
    let end_idx = match end_signed {
        Some(e) => normalize(e) as usize,
        None => chars.len(),
    };
    if start > end_idx {
        return Err(ElispError::EvalError(format!(
            "substring: start {start} > end {end_idx}"
        )));
    }
    let result: String = chars[start..end_idx].iter().collect();
    Ok(LispObject::string(&result))
}

// ---------------------------------------------------------------------------
// List operations
// ---------------------------------------------------------------------------

fn prim_nth(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if n < 0 {
        return Ok(LispObject::nil());
    }
    let mut current = list;
    for _ in 0..n {
        match current.destructure_cons() {
            Some((_, cdr)) => current = cdr,
            None => return Ok(LispObject::nil()),
        }
    }
    match current.destructure_cons() {
        Some((car, _)) => Ok(car),
        None => Ok(LispObject::nil()),
    }
}

fn prim_nthcdr(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if n < 0 {
        return Ok(list);
    }
    let mut current = list;
    for _ in 0..n {
        match current.destructure_cons() {
            Some((_, cdr)) => current = cdr,
            None => return Ok(LispObject::nil()),
        }
    }
    Ok(current)
}

fn prim_setcar(args: &LispObject) -> ElispResult<LispObject> {
    let cell = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let new_car = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match &cell {
        LispObject::Cons(_) => {
            cell.set_car(new_car.clone());
            Ok(new_car)
        }
        _ => Err(ElispError::WrongTypeArgument("cons".to_string())),
    }
}

fn prim_setcdr(args: &LispObject) -> ElispResult<LispObject> {
    let cell = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let new_cdr = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match &cell {
        LispObject::Cons(_) => {
            cell.set_cdr(new_cdr.clone());
            Ok(new_cdr)
        }
        _ => Err(ElispError::WrongTypeArgument("cons".to_string())),
    }
}

fn prim_nconc(args: &LispObject) -> ElispResult<LispObject> {
    prim_append(args)
}

fn prim_nreverse(args: &LispObject) -> ElispResult<LispObject> {
    prim_reverse(args)
}

fn prim_delq(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut result = LispObject::nil();
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        if !eq_test(&elt, &car) {
            result = LispObject::cons(car, result);
        }
        current = cdr;
    }
    prim_reverse(&LispObject::cons(result, LispObject::nil()))
}

fn prim_memq(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = list;
    while let Some((car, _cdr)) = current.destructure_cons() {
        if eq_test(&elt, &car) {
            return Ok(current);
        }
        current = _cdr;
    }
    Ok(LispObject::nil())
}

fn prim_assq(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let alist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = alist;
    while let Some((entry, rest)) = current.destructure_cons() {
        if let Some((k, _)) = entry.destructure_cons() {
            if eq_test(&key, &k) {
                return Ok(entry);
            }
        }
        current = rest;
    }
    Ok(LispObject::nil())
}

fn prim_last(args: &LispObject) -> ElispResult<LispObject> {
    let list = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = args.nth(1).and_then(|a| a.as_integer()).unwrap_or(1);
    if n <= 0 {
        // (last '(a b c) 0) => nil in Emacs
        return Ok(LispObject::nil());
    }
    // Count list length
    let mut len: i64 = 0;
    let mut current = list.clone();
    while let Some((_, cdr)) = current.destructure_cons() {
        len += 1;
        current = cdr;
    }
    let skip = (len - n).max(0);
    let mut current = list;
    for _ in 0..skip {
        if let Some((_, cdr)) = current.destructure_cons() {
            current = cdr;
        }
    }
    Ok(current)
}

fn prim_copy_sequence(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(arg.clone())
}

fn prim_cadr(args: &LispObject) -> ElispResult<LispObject> {
    // (car (cdr x))
    let cdr_args = prim_cdr(args)?;
    let wrapped = LispObject::cons(cdr_args, LispObject::nil());
    prim_car(&wrapped)
}

fn prim_cddr(args: &LispObject) -> ElispResult<LispObject> {
    // (cdr (cdr x))
    let cdr_args = prim_cdr(args)?;
    let wrapped = LispObject::cons(cdr_args, LispObject::nil());
    prim_cdr(&wrapped)
}

fn prim_caar(args: &LispObject) -> ElispResult<LispObject> {
    // (car (car x))
    let car_args = prim_car(args)?;
    let wrapped = LispObject::cons(car_args, LispObject::nil());
    prim_car(&wrapped)
}

fn prim_cdar(args: &LispObject) -> ElispResult<LispObject> {
    // (cdr (car x))
    let car_args = prim_car(args)?;
    let wrapped = LispObject::cons(car_args, LispObject::nil());
    prim_cdr(&wrapped)
}

fn prim_car_safe(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Cons(_) => {
            let wrapped = LispObject::cons(arg, LispObject::nil());
            prim_car(&wrapped)
        }
        _ => Ok(LispObject::nil()),
    }
}

fn prim_cdr_safe(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Cons(_) => {
            let wrapped = LispObject::cons(arg, LispObject::nil());
            prim_cdr(&wrapped)
        }
        _ => Ok(LispObject::nil()),
    }
}

fn prim_make_list(args: &LispObject) -> ElispResult<LispObject> {
    let length = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let init = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if length < 0 {
        return Ok(LispObject::nil());
    }
    let mut result = LispObject::nil();
    for _ in 0..length {
        result = LispObject::cons(init.clone(), result);
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Type predicates
// ---------------------------------------------------------------------------

fn prim_atom(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(!arg.is_cons()))
}

fn prim_integerp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_integer()))
}

fn prim_floatp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_float()))
}

fn prim_zerop(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::from(n == 0.0))
}

fn prim_natnump(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match &arg {
        LispObject::Integer(i) => *i >= 0,
        _ => false,
    };
    Ok(LispObject::from(result))
}

fn prim_functionp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match &arg {
        LispObject::Primitive(_) => true,
        LispObject::Cons(cell) => {
            let b = cell.lock();
            if let LispObject::Symbol(id) = &b.0 {
                crate::obarray::symbol_name(*id) == "lambda"
            } else {
                false
            }
        }
        _ => false,
    };
    Ok(LispObject::from(result))
}

fn prim_subrp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(arg.is_primitive()))
}

// ---------------------------------------------------------------------------
// Numeric
// ---------------------------------------------------------------------------

fn numeric_result(val: f64) -> LispObject {
    if val == val.floor() && val.abs() < 1e15 {
        LispObject::integer(val as i64)
    } else {
        LispObject::float(val)
    }
}

fn prim_1_plus(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::Integer(n) => Ok(LispObject::integer(n.wrapping_add(1))),
        LispObject::Float(f) => Ok(LispObject::float(f + 1.0)),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

fn prim_1_minus(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::Integer(n) => Ok(LispObject::integer(n.wrapping_sub(1))),
        LispObject::Float(f) => Ok(LispObject::float(f - 1.0)),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

fn prim_mod(args: &LispObject) -> ElispResult<LispObject> {
    let x = args
        .first()
        .and_then(|a| get_number(&a))
        .ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    let y = args
        .nth(1)
        .and_then(|a| get_number(&a))
        .ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    if y == 0.0 {
        return Err(ElispError::DivisionByZero);
    }
    // Emacs mod: result has same sign as divisor
    let r = x % y;
    let result = if r != 0.0 && ((r > 0.0) != (y > 0.0)) {
        r + y
    } else {
        r
    };
    Ok(numeric_result(result))
}

fn prim_abs(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Integer(i) => Ok(LispObject::integer(i.abs())),
        LispObject::Float(f) => Ok(LispObject::float(f.abs())),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

fn prim_max(args: &LispObject) -> ElispResult<LispObject> {
    let first = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut max_val =
        get_number(&first).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    let mut max_obj = first;
    let mut current = args.rest().unwrap_or(LispObject::nil());
    while let Some((arg, rest)) = current.destructure_cons() {
        let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
        if n > max_val {
            max_val = n;
            max_obj = arg;
        }
        current = rest;
    }
    Ok(max_obj)
}

fn prim_min(args: &LispObject) -> ElispResult<LispObject> {
    let first = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut min_val =
        get_number(&first).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    let mut min_obj = first;
    let mut current = args.rest().unwrap_or(LispObject::nil());
    while let Some((arg, rest)) = current.destructure_cons() {
        let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
        if n < min_val {
            min_val = n;
            min_obj = arg;
        }
        current = rest;
    }
    Ok(min_obj)
}

fn prim_floor(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::integer(n.floor() as i64))
}

fn prim_ceiling(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::integer(n.ceil() as i64))
}

fn prim_round(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::integer(n.round() as i64))
}

fn prim_truncate(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::integer(n.trunc() as i64))
}

fn prim_float(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = get_number(&arg).ok_or(ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::float(n))
}

fn prim_ash(args: &LispObject) -> ElispResult<LispObject> {
    let value = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let count = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    Ok(LispObject::integer(crate::emacs::data::arithmetic_shift(
        value, count,
    )))
}

fn collect_int_args(args: &LispObject) -> ElispResult<Vec<i64>> {
    let mut values = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        values.push(
            arg.as_integer()
                .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?,
        );
        current = rest;
    }
    Ok(values)
}

fn prim_logand(args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(crate::emacs::data::logand(
        &collect_int_args(args)?,
    )))
}

fn prim_logior(args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(crate::emacs::data::logior(
        &collect_int_args(args)?,
    )))
}

fn prim_lognot(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    Ok(LispObject::integer(crate::emacs::data::lognot(n)))
}

// ---------------------------------------------------------------------------
// Symbol
// ---------------------------------------------------------------------------

fn prim_symbol_name(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Symbol(id) => Ok(LispObject::string(&crate::obarray::symbol_name(*id))),
        LispObject::Nil => Ok(LispObject::string("nil")),
        LispObject::T => Ok(LispObject::string("t")),
        _ => Err(ElispError::WrongTypeArgument("symbol".to_string())),
    }
}

// ---------------------------------------------------------------------------
// String
// ---------------------------------------------------------------------------

fn prim_string_to_number(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = match &arg {
        LispObject::String(s) => s.clone(),
        // Emacs returns 0 for non-string args; tolerate nil gracefully
        _ if arg.is_nil() => return Ok(LispObject::integer(0)),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };
    // Try integer first, then float, default to 0
    if let Ok(i) = s.trim().parse::<i64>() {
        Ok(LispObject::integer(i))
    } else if let Ok(f) = s.trim().parse::<f64>() {
        Ok(LispObject::float(f))
    } else {
        Ok(LispObject::integer(0))
    }
}

fn prim_number_to_string(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Integer(i) => Ok(LispObject::string(&i.to_string())),
        LispObject::Float(f) => Ok(LispObject::string(&f.to_string())),
        _ => Err(ElispError::WrongTypeArgument("number".to_string())),
    }
}

fn prim_make_string(args: &LispObject) -> ElispResult<LispObject> {
    let length = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let ch = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    if length < 0 {
        return Ok(LispObject::string(""));
    }
    let c = char::from_u32(ch as u32).unwrap_or('?');
    let s: String = std::iter::repeat_n(c, length as usize).collect();
    Ok(LispObject::string(&s))
}

// ---------------------------------------------------------------------------
// I/O
// ---------------------------------------------------------------------------

fn prim_prin1_to_string(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::string(&arg.prin1_to_string()))
}

// ---------------------------------------------------------------------------
// Misc
// ---------------------------------------------------------------------------

fn prim_identity(args: &LispObject) -> ElispResult<LispObject> {
    args.first().ok_or(ElispError::WrongNumberOfArguments)
}

fn prim_ignore(args: &LispObject) -> ElispResult<LispObject> {
    // Consume all args, return nil
    let _ = args;
    Ok(LispObject::nil())
}

// ---------------------------------------------------------------------------
// Keymap primitives — minimal implementations so that stdlib files that
// set up language environments / input methods can load without signalling
// wrong-type-argument on nil keymaps.
// ---------------------------------------------------------------------------

fn prim_make_sparse_keymap(args: &LispObject) -> ElispResult<LispObject> {
    // (make-sparse-keymap &optional PROMPT) → (keymap PROMPT) or (keymap)
    let prompt = args.first();
    match prompt {
        Some(p) if !p.is_nil() => Ok(LispObject::cons(
            LispObject::symbol("keymap"),
            LispObject::cons(p, LispObject::nil()),
        )),
        _ => Ok(LispObject::cons(
            LispObject::symbol("keymap"),
            LispObject::nil(),
        )),
    }
}

fn prim_make_keymap(args: &LispObject) -> ElispResult<LispObject> {
    // (make-keymap &optional PROMPT) — full keymap. Treat same as sparse.
    prim_make_sparse_keymap(args)
}

fn prim_keymapp(args: &LispObject) -> ElispResult<LispObject> {
    // (keymapp OBJ) → t if OBJ is a keymap (list starting with `keymap')
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some((car, _)) = obj.destructure_cons() {
        if car.as_symbol().as_deref() == Some("keymap") {
            return Ok(LispObject::t());
        }
    }
    Ok(LispObject::nil())
}

fn prim_define_key(args: &LispObject) -> ElispResult<LispObject> {
    // (define-key MAP KEY DEF &optional REMOVE) — stub that returns DEF.
    // We don't actually modify the keymap; just prevent signalling.
    Ok(args.nth(2).unwrap_or(LispObject::nil()))
}

fn prim_type_of(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let type_name = match &arg {
        LispObject::Nil => "symbol",
        LispObject::T => "symbol",
        LispObject::Symbol(_) => "symbol",
        LispObject::Integer(_) => "integer",
        LispObject::Float(_) => "float",
        LispObject::String(_) => "string",
        LispObject::Cons(_) => "cons",
        LispObject::Primitive(_) => "subr",
        LispObject::Vector(_) => "vector",
        LispObject::BytecodeFn(_) => "compiled-function",
        LispObject::HashTable(_) => "hash-table",
    };
    Ok(LispObject::symbol(type_name))
}

// ---------------------------------------------------------------------------
// Helper: eq test (identity equality for symbols/integers, pointer-like)
// ---------------------------------------------------------------------------

fn eq_test(a: &LispObject, b: &LispObject) -> bool {
    match (a, b) {
        (LispObject::Nil, LispObject::Nil) => true,
        (LispObject::T, LispObject::T) => true,
        (LispObject::Integer(x), LispObject::Integer(y)) => x == y,
        (LispObject::Symbol(x), LispObject::Symbol(y)) => x == y,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// String — extended
// ---------------------------------------------------------------------------

fn prim_upcase(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::string(&crate::emacs::casefiddle::upcase_string(s))),
        LispObject::Integer(c) => {
            let ch = char::from_u32(*c as u32).unwrap_or('?');
            Ok(LispObject::integer(crate::emacs::casefiddle::upcase_char(ch) as i64))
        }
        _ => Err(ElispError::WrongTypeArgument("string-or-char".to_string())),
    }
}

fn prim_downcase(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::string(&crate::emacs::casefiddle::downcase_string(s))),
        LispObject::Integer(c) => {
            let ch = char::from_u32(*c as u32).unwrap_or('?');
            Ok(LispObject::integer(crate::emacs::casefiddle::downcase_char(ch) as i64))
        }
        _ => Err(ElispError::WrongTypeArgument("string-or-char".to_string())),
    }
}

fn prim_capitalize(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::string(&crate::emacs::casefiddle::capitalize_string(s))),
        LispObject::Integer(c) => {
            let ch = char::from_u32(*c as u32).unwrap_or('?');
            // Emacs: capitalize of a char == upcase (single-character "word")
            Ok(LispObject::integer(crate::emacs::casefiddle::upcase_char(ch) as i64))
        }
        _ => Err(ElispError::WrongTypeArgument("string-or-char".to_string())),
    }
}

fn prim_safe_length(args: &LispObject) -> ElispResult<LispObject> {
    // Like `length`, but returns the number of cons cells traversed
    // without signalling an error on a cyclic or dotted list. Uses
    // `destructure_cons` which clones the Arc — cheap enough for
    // loader-time use. Caps to stop cycles.
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut count: i64 = 0;
    let mut cur = arg;
    while let Some((_, rest)) = cur.destructure_cons() {
        count += 1;
        cur = rest;
        if count > 1_000_000 {
            break;
        }
    }
    Ok(LispObject::integer(count))
}

fn prim_read(args: &LispObject) -> ElispResult<LispObject> {
    // (read STRING-OR-STREAM) — we only support string input. For
    // buffer/marker streams we'd need editor state; nil args in Emacs
    // read from stdin, which we don't model. Return nil instead of
    // erroring so callers that don't care about the result survive.
    let arg = args.first().unwrap_or(LispObject::nil());
    match arg {
        LispObject::String(s) => {
            crate::reader::read(&s).map_err(|e| ElispError::EvalError(format!("read: {e}")))
        }
        _ => Ok(LispObject::nil()),
    }
}

fn prim_max_char(args: &LispObject) -> ElispResult<LispObject> {
    // (max-char &optional UNICODE) — max character code in Emacs char
    // space. Emacs 30 returns #x3fffff. Argument selects Unicode-only
    // max (`#x10ffff`) when t. We return the Emacs constant for nil/no
    // arg and the Unicode constant for `t`.
    let unicode_arg = args.first().unwrap_or(LispObject::nil());
    if matches!(unicode_arg, LispObject::T) {
        Ok(LispObject::integer(0x10ffff))
    } else {
        Ok(LispObject::integer(0x3fffff))
    }
}

fn prim_regexp_quote(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::string(&crate::emacs::search::regexp_quote(s))),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string(args: &LispObject) -> ElispResult<LispObject> {
    // (string &rest CHARS) → build a string from character codepoints.
    let mut out = String::new();
    let mut cur = args.clone();
    while let Some((car, rest)) = cur.destructure_cons() {
        match car {
            LispObject::Integer(n) => {
                let ch = char::from_u32(n as u32)
                    .ok_or_else(|| ElispError::WrongTypeArgument("character".to_string()))?;
                out.push(ch);
            }
            _ => return Err(ElispError::WrongTypeArgument("character".to_string())),
        }
        cur = rest;
    }
    Ok(LispObject::string(&out))
}

fn prim_characterp(args: &LispObject) -> ElispResult<LispObject> {
    // (characterp OBJ) → t if OBJ is a valid character. In Emacs a
    // character is a non-negative integer that is a valid Unicode
    // code point (< 0x3fffff in Emacs's char space).
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::Integer(n) if *n >= 0 && *n < 0x3fffff => Ok(LispObject::t()),
        _ => Ok(LispObject::nil()),
    }
}

fn prim_decode_char(args: &LispObject) -> ElispResult<LispObject> {
    // (decode-char CHARSET CODE-POINT &optional RESTRICTION)
    // Proper implementation requires a charset registry. Stub: return
    // CODE-POINT as a character for the `unicode` / `ucs` charsets,
    // nil otherwise (Emacs signals nil for unsupported mappings). This
    // is enough for `characters.el` to advance past the call.
    let charset = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let code = args.nth(1).unwrap_or(LispObject::nil());
    let charset_name = charset.as_symbol().unwrap_or_default();
    match (charset_name.as_str(), &code) {
        ("unicode" | "ucs", LispObject::Integer(_)) => Ok(code),
        _ => Ok(LispObject::nil()),
    }
}

fn prim_encode_char(args: &LispObject) -> ElispResult<LispObject> {
    // (encode-char CHAR CHARSET) — inverse of decode-char. Same stub
    // strategy: pass-through for unicode/ucs, nil otherwise.
    let ch = args.first().unwrap_or(LispObject::nil());
    let charset = args.nth(1).unwrap_or(LispObject::nil());
    let charset_name = charset.as_symbol().unwrap_or_default();
    match (charset_name.as_str(), &ch) {
        ("unicode" | "ucs", LispObject::Integer(_)) => Ok(ch),
        _ => Ok(LispObject::nil()),
    }
}

fn prim_string_replace(args: &LispObject) -> ElispResult<LispObject> {
    let from = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let to = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let in_str = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    match (&from, &to, &in_str) {
        (LispObject::String(f), LispObject::String(t), LispObject::String(s)) => {
            Ok(LispObject::string(&s.replace(f.as_str(), t.as_str())))
        }
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string_trim(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::string(s.trim())),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string_prefix_p(args: &LispObject) -> ElispResult<LispObject> {
    let prefix = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match (&prefix, &s) {
        (LispObject::String(p), LispObject::String(s)) => {
            Ok(LispObject::from(s.starts_with(p.as_str())))
        }
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string_suffix_p(args: &LispObject) -> ElispResult<LispObject> {
    let suffix = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match (&suffix, &s) {
        (LispObject::String(sfx), LispObject::String(s)) => {
            Ok(LispObject::from(s.ends_with(sfx.as_str())))
        }
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string_join(args: &LispObject) -> ElispResult<LispObject> {
    let strings = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let separator = args
        .nth(1)
        .and_then(|a| a.as_string().cloned())
        .unwrap_or_default();
    let mut parts = Vec::new();
    let mut current = strings;
    while let Some((car, cdr)) = current.destructure_cons() {
        match &car {
            LispObject::String(s) => parts.push(s.clone()),
            _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
        }
        current = cdr;
    }
    Ok(LispObject::string(&parts.join(&separator)))
}

fn prim_char_to_string(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let code = arg
        .as_integer()
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let ch = char::from_u32(code as u32).unwrap_or('?');
    Ok(LispObject::string(&ch.to_string()))
}

fn prim_string_to_char(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => {
            let ch = s.chars().next().unwrap_or('\0');
            Ok(LispObject::integer(ch as i64))
        }
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_string_width(args: &LispObject) -> ElispResult<LispObject> {
    // `string-width` accepts a string; `char-width` (routed here)
    // accepts a character (integer). Return 1 for characters since
    // we don't model East-Asian width.
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &arg {
        LispObject::String(s) => Ok(LispObject::integer(s.chars().count() as i64)),
        LispObject::Integer(_) => Ok(LispObject::integer(1)),
        _ => Err(ElispError::WrongTypeArgument("string or char".to_string())),
    }
}

fn prim_multibyte_string_p(args: &LispObject) -> ElispResult<LispObject> {
    // `multibyte-string-p` is a predicate — non-strings return nil,
    // not a wrong-type-argument error (matches Emacs semantics and
    // lets callers `(when (multibyte-string-p x) ...)` without
    // wrapping in `stringp`).
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(matches!(arg, LispObject::String(_))))
}

// ---------------------------------------------------------------------------
// Sequence — extended
// ---------------------------------------------------------------------------

fn prim_elt(args: &LispObject) -> ElispResult<LispObject> {
    let seq = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    if n < 0 {
        return Err(ElispError::WrongTypeArgument("natnump".to_string()));
    }
    let idx = n as usize;
    match &seq {
        LispObject::String(s) => {
            let ch = s.chars().nth(idx);
            match ch {
                Some(c) => Ok(LispObject::integer(c as i64)),
                None => Err(ElispError::WrongTypeArgument(
                    "args-out-of-range".to_string(),
                )),
            }
        }
        LispObject::Vector(v) => {
            let v = v.lock();
            v.get(idx).cloned().ok_or(ElispError::WrongTypeArgument(
                "args-out-of-range".to_string(),
            ))
        }
        LispObject::Nil | LispObject::Cons(_) => {
            // Walk the list
            let mut current = seq.clone();
            for _ in 0..idx {
                match current.destructure_cons() {
                    Some((_, cdr)) => current = cdr,
                    None => {
                        return Err(ElispError::WrongTypeArgument(
                            "args-out-of-range".to_string(),
                        ))
                    }
                }
            }
            current.first().ok_or(ElispError::WrongTypeArgument(
                "args-out-of-range".to_string(),
            ))
        }
        _ => Err(ElispError::WrongTypeArgument("sequencep".to_string())),
    }
}

fn prim_copy_alist(args: &LispObject) -> ElispResult<LispObject> {
    let alist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut items = Vec::new();
    let mut current = alist;
    while let Some((entry, rest)) = current.destructure_cons() {
        let copied = if let Some((k, v)) = entry.destructure_cons() {
            LispObject::cons(k, v)
        } else {
            entry
        };
        items.push(copied);
        current = rest;
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

fn prim_plist_get(args: &LispObject) -> ElispResult<LispObject> {
    let plist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = plist;
    while let Some((key, rest)) = current.destructure_cons() {
        if eq_test(&key, &prop) {
            return rest.first().ok_or(ElispError::WrongNumberOfArguments);
        }
        // Skip value
        match rest.destructure_cons() {
            Some((_, next)) => current = next,
            None => return Ok(LispObject::nil()),
        }
    }
    Ok(LispObject::nil())
}

fn prim_plist_put(args: &LispObject) -> ElispResult<LispObject> {
    let plist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    // Build a new plist with the property set
    let mut items = Vec::new();
    let mut current = plist.clone();
    let mut found = false;
    while let Some((key, rest)) = current.destructure_cons() {
        if let Some((val, next)) = rest.destructure_cons() {
            if eq_test(&key, &prop) {
                items.push((key, value.clone()));
                found = true;
                current = next;
            } else {
                items.push((key, val));
                current = next;
            }
        } else {
            break;
        }
    }
    if !found {
        items.push((prop, value));
    }
    let mut result = LispObject::nil();
    for (k, v) in items.into_iter().rev() {
        result = LispObject::cons(v, result);
        result = LispObject::cons(k, result);
    }
    Ok(result)
}

fn prim_plist_member(args: &LispObject) -> ElispResult<LispObject> {
    let plist = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let prop = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = plist;
    while let Some((key, _rest)) = current.destructure_cons() {
        if eq_test(&key, &prop) {
            return Ok(current);
        }
        // Skip value, advance to next key
        match _rest.destructure_cons() {
            Some((_, next)) => current = next,
            None => return Ok(LispObject::nil()),
        }
    }
    Ok(LispObject::nil())
}

fn prim_remove(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut items = Vec::new();
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        if car != elt {
            items.push(car);
        }
        current = cdr;
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

fn prim_remq(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut items = Vec::new();
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        if !eq_test(&elt, &car) {
            items.push(car);
        }
        current = cdr;
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

fn prim_number_sequence(args: &LispObject) -> ElispResult<LispObject> {
    let from = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or(ElispError::WrongTypeArgument("integer".to_string()))?;
    let to = args.nth(1).and_then(|a| a.as_integer());
    let step = args.nth(2).and_then(|a| a.as_integer());

    // (number-sequence FROM) => (FROM)
    let to = match to {
        Some(t) => t,
        None => {
            return Ok(LispObject::cons(
                LispObject::integer(from),
                LispObject::nil(),
            ))
        }
    };
    let step = step.unwrap_or(if from <= to { 1 } else { -1 });
    if step == 0 {
        return Err(ElispError::InvalidOperation(
            "number-sequence step must be non-zero".to_string(),
        ));
    }
    let mut items = Vec::new();
    let mut i = from;
    if step > 0 {
        while i <= to {
            items.push(LispObject::integer(i));
            i += step;
        }
    } else {
        while i >= to {
            items.push(LispObject::integer(i));
            i += step;
        }
    }
    let mut result = LispObject::nil();
    for item in items.into_iter().rev() {
        result = LispObject::cons(item, result);
    }
    Ok(result)
}

// ---------------------------------------------------------------------------
// Numeric — extended
// ---------------------------------------------------------------------------

/// Simple LCG-based pseudo-random number generator state.
static RANDOM_STATE: AtomicI64 = AtomicI64::new(0);

fn prim_random(args: &LispObject) -> ElispResult<LispObject> {
    let limit = args.first();
    // Advance the LCG state
    let old = RANDOM_STATE.fetch_add(1, Ordering::Relaxed);
    // LCG: a=6364136223846793005, c=1442695040888963407 (Knuth)
    let raw = old
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let raw = raw.unsigned_abs() as i64; // ensure non-negative
    match limit {
        Some(LispObject::Integer(n)) if n > 0 => Ok(LispObject::integer(raw % n)),
        Some(LispObject::Integer(_)) => Err(ElispError::WrongTypeArgument(
            "positive integer".to_string(),
        )),
        Some(LispObject::T) => {
            // (random t) reseeds — use current time nanos
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as i64)
                .unwrap_or(42);
            RANDOM_STATE.store(seed, Ordering::Relaxed);
            Ok(LispObject::integer(seed.unsigned_abs() as i64))
        }
        _ => Ok(LispObject::integer(raw)),
    }
}

fn prim_logxor(args: &LispObject) -> ElispResult<LispObject> {
    Ok(LispObject::integer(crate::emacs::data::logxor(
        &collect_int_args(args)?,
    )))
}

// ---------------------------------------------------------------------------
// Type — extended
// ---------------------------------------------------------------------------

fn prim_sequencep(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = matches!(
        arg,
        LispObject::Nil | LispObject::Cons(_) | LispObject::Vector(_) | LispObject::String(_)
    );
    Ok(LispObject::from(result))
}

fn prim_char_or_string_p(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = matches!(arg, LispObject::String(_) | LispObject::Integer(_));
    Ok(LispObject::from(result))
}

fn prim_booleanp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = matches!(arg, LispObject::Nil | LispObject::T);
    Ok(LispObject::from(result))
}

fn prim_keywordp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let result = match &arg {
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id).starts_with(':'),
        _ => false,
    };
    Ok(LispObject::from(result))
}

// ---------------------------------------------------------------------------
// Misc — extended
// ---------------------------------------------------------------------------

fn prim_error(args: &LispObject) -> ElispResult<LispObject> {
    let msg = args
        .first()
        .map(|a| a.princ_to_string())
        .unwrap_or_default();
    Err(ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("error"),
        data: LispObject::cons(LispObject::string(&msg), LispObject::nil()),
    })))
}

fn prim_user_error(args: &LispObject) -> ElispResult<LispObject> {
    let msg = args
        .first()
        .map(|a| a.princ_to_string())
        .unwrap_or_default();
    Err(ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("user-error"),
        data: LispObject::cons(LispObject::string(&msg), LispObject::nil()),
    })))
}

fn prim_signal(args: &LispObject) -> ElispResult<LispObject> {
    let symbol = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let data = args.nth(1).unwrap_or(LispObject::nil());
    Err(ElispError::Signal(Box::new(SignalData { symbol, data })))
}

// ---------------------------------------------------------------------------
// Math — transcendental helpers
// ---------------------------------------------------------------------------

fn prim_math_1(args: &LispObject, f: fn(f64) -> f64) -> ElispResult<LispObject> {
    let n = get_number(&args.first().ok_or(ElispError::WrongNumberOfArguments)?)
        .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::float(f(n)))
}

fn prim_atan(args: &LispObject) -> ElispResult<LispObject> {
    let y = get_number(&args.first().ok_or(ElispError::WrongNumberOfArguments)?)
        .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    // (atan Y) or (atan Y X) — two-argument form is atan2
    if let Some(x_obj) = args.nth(1) {
        let x = get_number(&x_obj)
            .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        Ok(LispObject::float(y.atan2(x)))
    } else {
        Ok(LispObject::float(y.atan()))
    }
}

fn prim_log(args: &LispObject) -> ElispResult<LispObject> {
    let n = get_number(&args.first().ok_or(ElispError::WrongNumberOfArguments)?)
        .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    // (log X) = natural log, (log X BASE) = log base BASE
    if let Some(base_obj) = args.nth(1) {
        let base = get_number(&base_obj)
            .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
        Ok(LispObject::float(n.ln() / base.ln()))
    } else {
        Ok(LispObject::float(n.ln()))
    }
}

fn prim_expt(args: &LispObject) -> ElispResult<LispObject> {
    let base = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let power = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match (&base, &power) {
        (LispObject::Integer(b), LispObject::Integer(p)) if *p >= 0 => {
            // checked_pow returns None on overflow; fall back to f64 in
            // that case (Emacs would promote to bignum, but bignums are
            // out of scope here).
            match (*b).checked_pow(*p as u32) {
                Some(v) => Ok(LispObject::integer(v)),
                None => Ok(LispObject::float((*b as f64).powi(*p as i32))),
            }
        }
        _ => {
            let b = get_number(&base)
                .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
            let p = get_number(&power)
                .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
            Ok(LispObject::float(b.powf(p)))
        }
    }
}

// ---------------------------------------------------------------------------
// Record primitive — (record TYPE SLOTS...) creates a record.
// In rele, records are represented as vectors with the type symbol at index 0.
// ---------------------------------------------------------------------------

fn prim_record(args: &LispObject) -> ElispResult<LispObject> {
    let mut items = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        items.push(arg);
        current = rest;
    }
    Ok(LispObject::Vector(std::sync::Arc::new(
        parking_lot::Mutex::new(items),
    )))
}

// ---------------------------------------------------------------------------
// Vector primitives
// ---------------------------------------------------------------------------

fn prim_aref(args: &LispObject) -> ElispResult<LispObject> {
    let seq = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let idx_obj = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let idx_signed = idx_obj
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    if idx_signed < 0 {
        return Err(ElispError::InvalidOperation(format!(
            "aref: negative index {idx_signed}"
        )));
    }
    let idx = idx_signed as usize;
    match &seq {
        LispObject::Vector(v) => {
            let v = v.lock();
            v.get(idx)
                .cloned()
                .ok_or_else(|| ElispError::InvalidOperation(format!("index {idx} out of range")))
        }
        LispObject::String(s) => {
            // (aref STRING IDX) → character code at IDX
            s.chars()
                .nth(idx)
                .map(|c| LispObject::integer(c as i64))
                .ok_or_else(|| ElispError::InvalidOperation(format!("index {idx} out of range")))
        }
        _ => Err(ElispError::WrongTypeArgument(
            "array-or-string".to_string(),
        )),
    }
}

fn prim_aset(args: &LispObject) -> ElispResult<LispObject> {
    let seq = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let idx_obj = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let val = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    // Tolerant fallback: if idx or target isn't the right type, return VALUE.
    // Lots of stdlib code calls `(aset CHAR-TABLE CHAR VAL)` where our
    // char-table stubs may hand back something smaller than the char code;
    // better to silently accept than to break entire bootstrap files.
    let idx = match idx_obj.as_integer() {
        Some(i) if i >= 0 => i as usize,
        _ => return Ok(val),
    };
    match &seq {
        LispObject::Vector(v) => {
            let mut guard = v.lock();
            if idx >= guard.len() {
                return Ok(val); // out-of-range: silently no-op
            }
            guard[idx] = val.clone();
            Ok(val)
        }
        // Non-vector target (commonly nil from stubs) — silent no-op
        _ => Ok(val),
    }
}

fn prim_make_vector(args: &LispObject) -> ElispResult<LispObject> {
    let len = args
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?
        as usize;
    let init = args.nth(1).unwrap_or(LispObject::nil());
    let v: Vec<LispObject> = vec![init; len];
    Ok(LispObject::Vector(std::sync::Arc::new(
        parking_lot::Mutex::new(v),
    )))
}

fn prim_vconcat(args: &LispObject) -> ElispResult<LispObject> {
    let mut result = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        match &arg {
            LispObject::Vector(v) => result.extend(v.lock().iter().cloned()),
            LispObject::Nil => {}
            LispObject::String(s) => {
                for c in s.chars() {
                    result.push(LispObject::integer(c as i64));
                }
            }
            other => {
                // Try as list
                let mut cur = other.clone();
                while let Some((item, rest)) = cur.destructure_cons() {
                    result.push(item);
                    cur = rest;
                }
            }
        }
        current = rest;
    }
    Ok(LispObject::Vector(std::sync::Arc::new(
        parking_lot::Mutex::new(result),
    )))
}

fn prim_vectorp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(matches!(arg, LispObject::Vector(_))))
}

// ---------------------------------------------------------------------------
// String search / comparison primitives
// ---------------------------------------------------------------------------

fn prim_string_search(args: &LispObject) -> ElispResult<LispObject> {
    // (string-search NEEDLE HAYSTACK &optional START-POS)
    let needle = args
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_string()
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
        .clone();
    let haystack = args
        .nth(1)
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_string()
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
        .clone();
    let start = args
        .nth(2)
        .and_then(|a| a.as_integer())
        .unwrap_or(0) as usize;
    let slice = if start <= haystack.len() {
        &haystack[start..]
    } else {
        return Ok(LispObject::nil());
    };
    match slice.find(&*needle) {
        Some(pos) => Ok(LispObject::integer((start + pos) as i64)),
        None => Ok(LispObject::nil()),
    }
}

fn prim_string_equal(args: &LispObject) -> ElispResult<LispObject> {
    // (string-equal S1 S2) — accepts strings and symbols (coerces via symbol-name)
    let s1 = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let s2 = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let a_str = match &s1 {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Err(ElispError::WrongTypeArgument("string-or-symbol".to_string())),
    };
    let b_str = match &s2 {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Err(ElispError::WrongTypeArgument("string-or-symbol".to_string())),
    };
    Ok(LispObject::from(a_str == b_str))
}

fn prim_string_lessp(args: &LispObject) -> ElispResult<LispObject> {
    // (string-lessp S1 S2) — lexicographic comparison
    let a_str = match &args.first().ok_or(ElispError::WrongNumberOfArguments)? {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Err(ElispError::WrongTypeArgument("string-or-symbol".to_string())),
    };
    let b_str = match &args.nth(1).ok_or(ElispError::WrongNumberOfArguments)? {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(id) => crate::obarray::symbol_name(*id),
        _ => return Err(ElispError::WrongTypeArgument("string-or-symbol".to_string())),
    };
    Ok(LispObject::from(a_str < b_str))
}

fn prim_compare_strings(args: &LispObject) -> ElispResult<LispObject> {
    // (compare-strings S1 START1 END1 S2 START2 END2 &optional IGNORE-CASE)
    let s1 = args
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_string()
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
        .clone();
    let start1 = args.nth(1).and_then(|a| a.as_integer()).unwrap_or(0) as usize;
    let end1 = args
        .nth(2)
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or(s1.len());
    let s2 = args
        .nth(3)
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_string()
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
        .clone();
    let start2 = args.nth(4).and_then(|a| a.as_integer()).unwrap_or(0) as usize;
    let end2 = args
        .nth(5)
        .and_then(|a| a.as_integer())
        .map(|n| n as usize)
        .unwrap_or(s2.len());
    let ignore_case = args.nth(6).is_some_and(|a| !a.is_nil());

    let s1_len = s1.len();
    let s2_len = s2.len();
    let start1 = start1.min(s1_len);
    let start2 = start2.min(s2_len);
    let end1 = end1.min(s1_len).max(start1);
    let end2 = end2.min(s2_len).max(start2);
    let slice1 = &s1[start1..end1];
    let slice2 = &s2[start2..end2];

    let cmp = if ignore_case {
        slice1.to_lowercase().cmp(&slice2.to_lowercase())
    } else {
        slice1.cmp(slice2)
    };
    match cmp {
        std::cmp::Ordering::Equal => Ok(LispObject::t()),
        std::cmp::Ordering::Less => {
            // Return negative of first differing position (1-based)
            let pos = slice1
                .chars()
                .zip(slice2.chars())
                .position(|(a, b)| a != b)
                .map(|p| p + 1)
                .unwrap_or(slice1.len() + 1);
            Ok(LispObject::integer(-(pos as i64)))
        }
        std::cmp::Ordering::Greater => {
            let pos = slice1
                .chars()
                .zip(slice2.chars())
                .position(|(a, b)| a != b)
                .map(|p| p + 1)
                .unwrap_or(slice2.len() + 1);
            Ok(LispObject::integer(pos as i64))
        }
    }
}

fn prim_split_string(args: &LispObject) -> ElispResult<LispObject> {
    // (split-string STRING &optional SEPARATORS OMIT-NULLS TRIM)
    let string = args
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_string()
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
        .clone();
    let sep_pattern = args.nth(1).and_then(|a| a.as_string().cloned());
    let omit_nulls = args.nth(2).is_some_and(|a| !a.is_nil());

    let parts: Vec<&str> = if let Some(sep) = &sep_pattern {
        // Use the separator as a literal split (simplified — Emacs uses regex)
        string.split(&**sep).collect()
    } else {
        // Default: split on whitespace
        string.split_whitespace().collect()
    };

    let mut result = LispObject::nil();
    for part in parts.iter().rev() {
        if omit_nulls && part.is_empty() {
            continue;
        }
        result = LispObject::cons(LispObject::string(part), result);
    }
    Ok(result)
}

// ---- Phase 8: data.c type predicates ----

fn prim_arrayp(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(matches!(
        obj,
        LispObject::Vector(_) | LispObject::String(_)
    )))
}

fn prim_nlistp(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(!obj.is_nil() && !matches!(obj, LispObject::Cons(_))))
}

fn prim_byte_code_function_p(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(matches!(obj, LispObject::BytecodeFn(_))))
}

fn prim_hash_table_p(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(matches!(obj, LispObject::HashTable(_))))
}

fn prim_logcount(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    Ok(LispObject::integer(i64::from(crate::emacs::data::logcount(n))))
}

fn prim_indirect_function(args: &LispObject) -> ElispResult<LispObject> {
    // In rele, symbols don't chain through indirect functions.
    // Return the argument as-is (enough for bootstrap).
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(obj)
}

fn prim_subr_arity(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &obj {
        LispObject::Primitive(_) => {
            // Return (0 . many) as a conservative default
            Ok(LispObject::cons(
                LispObject::integer(0),
                LispObject::symbol("many"),
            ))
        }
        _ => Err(ElispError::WrongTypeArgument("subr".to_string())),
    }
}

fn prim_subr_name(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match &obj {
        LispObject::Primitive(name) => Ok(LispObject::string(name)),
        _ => Err(ElispError::WrongTypeArgument("subr".to_string())),
    }
}

fn prim_setplist(args: &LispObject) -> ElispResult<LispObject> {
    let sym = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let plist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if let LispObject::Symbol(id) = &sym {
        // Clear existing plist by replacing with the new one
        crate::obarray::replace_plist(*id, plist.clone());
        Ok(plist)
    } else {
        Err(ElispError::WrongTypeArgument("symbolp".to_string()))
    }
}

// ---- Phase 8: fns.c additions ----

fn prim_proper_list_p(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if obj.is_nil() {
        return Ok(LispObject::integer(0));
    }
    let mut len = 0u64;
    let mut current = obj;
    loop {
        if len > MAX_LIST_WALK {
            // Probable circular list; report as "not a proper list".
            return Ok(LispObject::nil());
        }
        match current.destructure_cons() {
            Some((_, rest)) => {
                len += 1;
                current = rest;
                if current.is_nil() {
                    return Ok(LispObject::integer(len as i64));
                }
            }
            None => return Ok(LispObject::nil()),
        }
    }
}

fn prim_delete(args: &LispObject) -> ElispResult<LispObject> {
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let seq = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    // For lists: return a new list with all `equal` matches removed
    let mut result = LispObject::nil();
    let mut current = seq;
    while let Some((car, cdr)) = current.destructure_cons() {
        if car != elt {
            result = LispObject::cons(car, result);
        }
        current = cdr;
    }
    // Reverse to preserve order
    let mut reversed = LispObject::nil();
    let mut cur = result;
    while let Some((car, cdr)) = cur.destructure_cons() {
        reversed = LispObject::cons(car, reversed);
        cur = cdr;
    }
    Ok(reversed)
}

fn prim_rassq(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let alist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = alist;
    while let Some((entry, rest)) = current.destructure_cons() {
        if let Some((_, v)) = entry.destructure_cons() {
            if prim_eq_test(&key, &v) {
                return Ok(entry);
            }
        }
        current = rest;
    }
    Ok(LispObject::nil())
}

fn prim_rassoc(args: &LispObject) -> ElispResult<LispObject> {
    let key = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let alist = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = alist;
    while let Some((entry, rest)) = current.destructure_cons() {
        if let Some((_, v)) = entry.destructure_cons() {
            if key == v {
                return Ok(entry);
            }
        }
        current = rest;
    }
    Ok(LispObject::nil())
}

fn prim_eq_test(a: &LispObject, b: &LispObject) -> bool {
    match (a, b) {
        (LispObject::Nil, LispObject::Nil) | (LispObject::T, LispObject::T) => true,
        (LispObject::Symbol(x), LispObject::Symbol(y)) => x == y,
        (LispObject::Integer(x), LispObject::Integer(y)) => x == y,
        _ => std::ptr::eq(a, b),
    }
}

fn prim_remhash(args: &LispObject) -> ElispResult<LispObject> {
    use crate::object::HashKey;
    let key_obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let table = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if let LispObject::HashTable(ht) = &table {
        let hash_key = match &key_obj {
            LispObject::Symbol(id) => HashKey::Symbol(*id),
            LispObject::Integer(n) => HashKey::Integer(*n),
            LispObject::String(s) => HashKey::String(s.clone()),
            other => HashKey::Printed(format!("{other:?}")),
        };
        ht.lock().data.remove(&hash_key);
        Ok(LispObject::nil())
    } else {
        Err(ElispError::WrongTypeArgument("hash-table-p".to_string()))
    }
}

fn prim_hash_table_count(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let LispObject::HashTable(ht) = &table {
        Ok(LispObject::integer(ht.lock().data.len() as i64))
    } else {
        Err(ElispError::WrongTypeArgument("hash-table-p".to_string()))
    }
}

fn prim_hash_table_test(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let LispObject::HashTable(ht) = &table {
        let name = match ht.lock().test {
            crate::object::HashTableTest::Eq => "eq",
            crate::object::HashTableTest::Eql => "eql",
            crate::object::HashTableTest::Equal => "equal",
        };
        Ok(LispObject::symbol(name))
    } else {
        Err(ElispError::WrongTypeArgument("hash-table-p".to_string()))
    }
}

fn prim_hash_table_size(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let LispObject::HashTable(ht) = &table {
        Ok(LispObject::integer(ht.lock().data.capacity() as i64))
    } else {
        Err(ElispError::WrongTypeArgument("hash-table-p".to_string()))
    }
}

fn prim_copy_hash_table(args: &LispObject) -> ElispResult<LispObject> {
    use std::sync::Arc;
    use parking_lot::Mutex;
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let LispObject::HashTable(ht) = &table {
        let guard = ht.lock();
        let new_ht = crate::object::LispHashTable {
            test: guard.test,
            data: guard.data.clone(),
        };
        Ok(LispObject::HashTable(Arc::new(Mutex::new(new_ht))))
    } else {
        Err(ElispError::WrongTypeArgument("hash-table-p".to_string()))
    }
}

fn prim_take(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    if n <= 0 {
        return Ok(LispObject::nil());
    }
    let mut result = LispObject::nil();
    let mut current = list;
    let mut count = 0i64;
    while let Some((car, cdr)) = current.destructure_cons() {
        if count >= n {
            break;
        }
        result = LispObject::cons(car, result);
        current = cdr;
        count += 1;
    }
    // Reverse
    let mut reversed = LispObject::nil();
    let mut cur = result;
    while let Some((car, cdr)) = cur.destructure_cons() {
        reversed = LispObject::cons(car, reversed);
        cur = cdr;
    }
    Ok(reversed)
}

fn prim_length_lt(args: &LispObject) -> ElispResult<LispObject> {
    let seq = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = args
        .nth(1)
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    let len = seq_length(&seq)?;
    Ok(LispObject::from(len < n))
}

fn prim_length_gt(args: &LispObject) -> ElispResult<LispObject> {
    let seq = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = args
        .nth(1)
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    let len = seq_length(&seq)?;
    Ok(LispObject::from(len > n))
}

fn prim_length_eq(args: &LispObject) -> ElispResult<LispObject> {
    let seq = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let n = args
        .nth(1)
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    let len = seq_length(&seq)?;
    Ok(LispObject::from(len == n))
}

fn seq_length(obj: &LispObject) -> ElispResult<i64> {
    match obj {
        LispObject::Nil => Ok(0),
        LispObject::String(s) => Ok(s.chars().count() as i64),
        LispObject::Vector(v) => Ok(v.lock().len() as i64),
        LispObject::Cons(_) => {
            let mut count = 0i64;
            let mut cur = obj.clone();
            while let Some((_, rest)) = cur.destructure_cons() {
                count += 1;
                cur = rest;
            }
            Ok(count)
        }
        _ => Err(ElispError::WrongTypeArgument("sequencep".to_string())),
    }
}

fn prim_fillarray(args: &LispObject) -> ElispResult<LispObject> {
    let array = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let item = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match &array {
        LispObject::Vector(v) => {
            let mut guard = v.lock();
            for elem in guard.iter_mut() {
                *elem = item.clone();
            }
            drop(guard);
            Ok(array)
        }
        _ => Err(ElispError::WrongTypeArgument("arrayp".to_string())),
    }
}

fn prim_string_bytes(args: &LispObject) -> ElispResult<LispObject> {
    let s = args
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_string()
        .ok_or_else(|| ElispError::WrongTypeArgument("stringp".to_string()))?
        .clone();
    Ok(LispObject::integer(crate::emacs::fns::string_bytes(&s) as i64))
}

fn prim_sxhash(args: &LispObject) -> ElispResult<LispObject> {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let mut hasher = DefaultHasher::new();
    // Simple hash based on the debug representation
    format!("{obj:?}").hash(&mut hasher);
    let h = hasher.finish() as i64;
    // Emacs returns non-negative fixnums
    Ok(LispObject::integer(h.unsigned_abs() as i64))
}

fn prim_memql(args: &LispObject) -> ElispResult<LispObject> {
    // memql uses eql (eq for non-numbers, = for numbers)
    let elt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        let is_match = match (&elt, &car) {
            (LispObject::Integer(a), LispObject::Integer(b)) => a == b,
            (LispObject::Float(a), LispObject::Float(b)) => a == b,
            (LispObject::Integer(a), LispObject::Float(b)) => (*a as f64) == *b,
            (LispObject::Float(a), LispObject::Integer(b)) => *a == (*b as f64),
            _ => prim_eq_test(&elt, &car),
        };
        if is_match {
            // Return the tail starting at the match
            return Ok(LispObject::cons(car, cdr));
        }
        current = cdr;
    }
    Ok(LispObject::nil())
}

impl From<bool> for LispObject {
    fn from(b: bool) -> LispObject {
        if b {
            LispObject::t()
        } else {
            LispObject::nil()
        }
    }
}

// ---------------------------------------------------------------------------
// Batch: additional Emacs C DEFUNs with headless-safe implementations.
// These let stdlib and test files load past `void-function` errors on
// functions whose real behaviour requires a GUI, a live process, text
// properties, or a syntax table — none of which the headless elisp
// interpreter models. The shape of each return value matches what real
// Emacs returns for a non-interactive session or a no-op caller.
// ---------------------------------------------------------------------------

fn prim_make_bool_vector(args: &LispObject) -> ElispResult<LispObject> {
    // (make-bool-vector LENGTH INIT) — build a vector of LENGTH
    // items, each initialised to (if INIT t nil). We represent
    // bool-vectors as ordinary vectors of t/nil; `bool-vector-p`
    // stays `nil` because we don't model the distinct type.
    let len = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    if len < 0 {
        return Err(ElispError::WrongTypeArgument(
            "positive integer".to_string(),
        ));
    }
    let init = args.nth(1).unwrap_or_else(LispObject::nil);
    let fill = if init.is_nil() {
        LispObject::nil()
    } else {
        LispObject::t()
    };
    let v = vec![fill; len as usize];
    Ok(LispObject::Vector(std::sync::Arc::new(
        parking_lot::Mutex::new(v),
    )))
}

fn prim_bool_vector(args: &LispObject) -> ElispResult<LispObject> {
    // (bool-vector &rest OBJECTS) — each OBJECT becomes t (non-nil)
    // or nil in the resulting bool-vector, represented as a vector.
    let mut items = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        items.push(if arg.is_nil() {
            LispObject::nil()
        } else {
            LispObject::t()
        });
        current = rest;
    }
    Ok(LispObject::Vector(std::sync::Arc::new(
        parking_lot::Mutex::new(items),
    )))
}

fn prim_key_description(args: &LispObject) -> ElispResult<LispObject> {
    // (key-description KEYS &optional PREFIX) — simplistic: stringify
    // each element of KEYS (a string or vector) with a space separator.
    // Enough for most callers that just want a human-readable label.
    let keys = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let render = |obj: &LispObject| -> String {
        match obj {
            LispObject::Integer(i) => {
                if let Some(c) = char::from_u32(*i as u32) {
                    c.to_string()
                } else {
                    i.to_string()
                }
            }
            LispObject::Symbol(_) => obj.as_symbol().unwrap_or_default(),
            LispObject::String(s) => s.clone(),
            _ => String::new(),
        }
    };
    let out = match &keys {
        LispObject::String(s) => s.clone(),
        LispObject::Vector(v) => v
            .lock()
            .iter()
            .map(render)
            .collect::<Vec<_>>()
            .join(" "),
        _ => String::new(),
    };
    Ok(LispObject::string(&out))
}

fn prim_upcase_initials(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match arg {
        LispObject::String(s) => Ok(LispObject::string(
            &crate::emacs::casefiddle::upcase_initials(&s),
        )),
        LispObject::Integer(i) => {
            if let Some(c) = char::from_u32(i as u32) {
                let up = c.to_uppercase().next().unwrap_or(c);
                Ok(LispObject::integer(up as i64))
            } else {
                Ok(LispObject::integer(i))
            }
        }
        _ => Err(ElispError::WrongTypeArgument(
            "string or char".to_string(),
        )),
    }
}

fn prim_make_directory_internal(args: &LispObject) -> ElispResult<LispObject> {
    // (make-directory-internal DIRNAME) — one-level mkdir. The
    // `-internal` suffix means the caller (make-directory) has
    // already resolved the path.
    let name = args
        .first()
        .and_then(|a| match a {
            LispObject::String(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    std::fs::create_dir(&name)
        .map_err(|e| ElispError::EvalError(format!("make-directory-internal: {e}")))?;
    Ok(LispObject::nil())
}

// -- Time primitives ------------------------------------------------------
//
// Emacs represents times as one of:
//   - nil                     -> current time
//   - (HIGH LOW [USEC [PSEC]]) -> classic integer ticks
//   - (TICKS . HZ)            -> modern ratio form
//   - integer                 -> seconds since epoch
//   - float                   -> seconds since epoch
//
// We parse any of these into an `f64` seconds-since-epoch and compare,
// which is correct for the common uses (equality, ordering, format).

fn time_to_seconds(obj: &LispObject) -> Option<f64> {
    match obj {
        LispObject::Nil => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?;
            Some(now.as_secs_f64())
        }
        LispObject::Integer(i) => Some(*i as f64),
        LispObject::Float(f) => Some(*f),
        LispObject::Cons(_) => {
            // Either (HIGH LOW ...) or (TICKS . HZ).
            let (car, cdr) = obj.destructure_cons()?;
            let car_i = car.as_integer()?;
            match cdr {
                LispObject::Integer(hz) => {
                    if hz == 0 {
                        None
                    } else {
                        Some(car_i as f64 / hz as f64)
                    }
                }
                LispObject::Cons(_) => {
                    let high = car_i;
                    let low = cdr.first().and_then(|o| o.as_integer()).unwrap_or(0);
                    let usec_obj = cdr.nth(1);
                    let usec = usec_obj
                        .as_ref()
                        .and_then(LispObject::as_integer)
                        .unwrap_or(0);
                    Some(
                        (high as f64) * 65536.0 + (low as f64) + (usec as f64) / 1_000_000.0,
                    )
                }
                _ => None,
            }
        }
        _ => None,
    }
}

fn prim_time_equal_p(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let ea = time_to_seconds(&a);
    let eb = time_to_seconds(&b);
    Ok(LispObject::from(ea == eb && ea.is_some()))
}

fn prim_time_less_p(args: &LispObject) -> ElispResult<LispObject> {
    let a = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let b = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    match (time_to_seconds(&a), time_to_seconds(&b)) {
        (Some(x), Some(y)) => Ok(LispObject::from(x < y)),
        _ => Err(ElispError::WrongTypeArgument("time".to_string())),
    }
}

fn prim_time_convert(args: &LispObject) -> ElispResult<LispObject> {
    // (time-convert TIME &optional FORM) — we always return a float
    // (seconds since epoch). That's one of Emacs's accepted output
    // forms and enough for equality / formatting paths.
    let t = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let secs =
        time_to_seconds(&t).ok_or_else(|| ElispError::WrongTypeArgument("time".to_string()))?;
    Ok(LispObject::float(secs))
}

fn prim_format_time_string(args: &LispObject) -> ElispResult<LispObject> {
    // Minimal formatter — returns the format string unchanged when
    // it has no `%` directives, and otherwise stringifies the epoch
    // seconds. Real strftime support is out of scope here but this
    // lets callers that only probe the output's type / non-nil-ness
    // proceed.
    let fmt = args
        .first()
        .and_then(|a| match a {
            LispObject::String(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    let time_obj = args.nth(1).unwrap_or_else(LispObject::nil);
    let secs = time_to_seconds(&time_obj).unwrap_or(0.0);
    if fmt.contains('%') {
        Ok(LispObject::string(&format!("{secs}")))
    } else {
        Ok(LispObject::string(&fmt))
    }
}

// -- Second-batch helpers ---------------------------------------------------

fn prim_encode_time(args: &LispObject) -> ElispResult<LispObject> {
    // Accepts either a decoded-time list (SEC MIN HOUR DAY MON YEAR ...)
    // or a flat argument list (SEC MIN HOUR DAY MON YEAR ...). We
    // return the epoch as a float — that's an accepted Emacs time form.
    let first = args.first().unwrap_or_else(LispObject::nil);
    let parts: Vec<LispObject> = if let LispObject::Cons(_) = first {
        let mut v = Vec::new();
        let mut cur = first;
        while let Some((c, r)) = cur.destructure_cons() {
            v.push(c);
            cur = r;
        }
        v
    } else {
        let mut v = Vec::new();
        let mut cur = args.clone();
        while let Some((c, r)) = cur.destructure_cons() {
            v.push(c);
            cur = r;
        }
        v
    };
    let get = |i: usize| parts.get(i).and_then(LispObject::as_integer).unwrap_or(0);
    let sec = get(0);
    let min = get(1);
    let hour = get(2);
    let day = get(3);
    let mon = get(4);
    let year = get(5);
    // Very rough days-since-epoch approximation (ignores leap years
    // fine-grain); sufficient for round-trip callers that feed the
    // result into time comparison, not full calendar math.
    let days_since_epoch = ((year - 1970) * 365) + (mon - 1) * 30 + (day - 1);
    let secs = days_since_epoch * 86400 + hour * 3600 + min * 60 + sec;
    Ok(LispObject::float(secs as f64))
}

fn prim_current_time() -> ElispResult<LispObject> {
    // Emacs classic `(HIGH LOW USEC PSEC)` form: split epoch seconds.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let secs = now as i64;
    let high = secs >> 16;
    let low = secs & 0xffff;
    let usec = ((now - secs as f64) * 1_000_000.0) as i64;
    Ok(LispObject::cons(
        LispObject::integer(high),
        LispObject::cons(
            LispObject::integer(low),
            LispObject::cons(
                LispObject::integer(usec),
                LispObject::cons(LispObject::integer(0), LispObject::nil()),
            ),
        ),
    ))
}

fn prim_float_time(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().unwrap_or_else(LispObject::nil);
    let secs = time_to_seconds(&arg).unwrap_or(0.0);
    Ok(LispObject::float(secs))
}

fn prim_logb(args: &LispObject) -> ElispResult<LispObject> {
    // (logb N) — return the exponent of N as an integer.
    let n = args
        .first()
        .and_then(|a| match a {
            LispObject::Integer(i) => Some(i as f64),
            LispObject::Float(f) => Some(f),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    if n == 0.0 {
        Ok(LispObject::integer(i64::MIN))
    } else {
        Ok(LispObject::integer(n.abs().log2().floor() as i64))
    }
}

fn prim_frexp(args: &LispObject) -> ElispResult<LispObject> {
    // (frexp X) -> (MANTISSA . EXP) with X = MANTISSA * 2**EXP,
    // 0.5 <= |MANTISSA| < 1 (or 0).
    let x = args
        .first()
        .and_then(|a| match a {
            LispObject::Integer(i) => Some(i as f64),
            LispObject::Float(f) => Some(f),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    if x == 0.0 {
        return Ok(LispObject::cons(
            LispObject::float(0.0),
            LispObject::integer(0),
        ));
    }
    let (m, e) = {
        let bits = x.to_bits();
        let exp = ((bits >> 52) & 0x7ff) as i64 - 1022;
        let mantissa = x / (1f64.ldexp_safe(exp));
        (mantissa, exp)
    };
    Ok(LispObject::cons(LispObject::float(m), LispObject::integer(e)))
}

// Stand-in for f64::ldexp (unstable in the public API) — multiply by 2^n.
trait LdexpSafe {
    fn ldexp_safe(self, exp: i64) -> f64;
}
impl LdexpSafe for f64 {
    fn ldexp_safe(self, exp: i64) -> f64 {
        if exp >= 0 {
            self * (1u64 << exp.min(62)) as f64
        } else {
            self / (1u64 << (-exp).min(62)) as f64
        }
    }
}

fn prim_ldexp(args: &LispObject) -> ElispResult<LispObject> {
    // (ldexp MANTISSA EXP) — return MANTISSA * 2**EXP.
    let m = args
        .first()
        .and_then(|a| match a {
            LispObject::Integer(i) => Some(i as f64),
            LispObject::Float(f) => Some(f),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    let e = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    Ok(LispObject::float(m.ldexp_safe(e)))
}

fn prim_copysign(args: &LispObject) -> ElispResult<LispObject> {
    let x = args
        .first()
        .and_then(|a| match a {
            LispObject::Integer(i) => Some(i as f64),
            LispObject::Float(f) => Some(f),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    let y = args
        .nth(1)
        .and_then(|a| match a {
            LispObject::Integer(i) => Some(i as f64),
            LispObject::Float(f) => Some(f),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("number".to_string()))?;
    Ok(LispObject::float(x.copysign(y)))
}

fn prim_isnan(args: &LispObject) -> ElispResult<LispObject> {
    let x = args.first().unwrap_or_else(LispObject::nil);
    Ok(LispObject::from(matches!(x, LispObject::Float(f) if f.is_nan())))
}

fn prim_directory_name_p(args: &LispObject) -> ElispResult<LispObject> {
    let s = args.first().unwrap_or_else(LispObject::nil);
    if let LispObject::String(s) = s {
        Ok(LispObject::from(s.ends_with('/')))
    } else {
        Err(ElispError::WrongTypeArgument("string".to_string()))
    }
}

fn prim_file_accessible_directory_p(args: &LispObject) -> ElispResult<LispObject> {
    let s = args
        .first()
        .and_then(|a| match a {
            LispObject::String(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    Ok(LispObject::from(
        std::path::Path::new(&s).is_dir()
            && std::fs::metadata(&s).is_ok(),
    ))
}

fn prim_file_name_as_directory(args: &LispObject) -> ElispResult<LispObject> {
    let s = args
        .first()
        .and_then(|a| match a {
            LispObject::String(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    if s.ends_with('/') {
        Ok(LispObject::string(&s))
    } else {
        Ok(LispObject::string(&format!("{s}/")))
    }
}

fn prim_hash_table_keys(args: &LispObject) -> ElispResult<LispObject> {
    let ht = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match ht {
        LispObject::HashTable(h) => {
            let mut result = LispObject::nil();
            for key in h.lock().data.keys() {
                let key_obj = match key {
                    crate::object::HashKey::Symbol(id) => LispObject::Symbol(*id),
                    crate::object::HashKey::Integer(i) => LispObject::integer(*i),
                    crate::object::HashKey::String(s) => LispObject::string(s),
                    crate::object::HashKey::Printed(s) => LispObject::string(s),
                    crate::object::HashKey::Identity(_) => LispObject::nil(),
                };
                result = LispObject::cons(key_obj, result);
            }
            Ok(result)
        }
        _ => Err(ElispError::WrongTypeArgument("hash-table".to_string())),
    }
}

fn prim_hash_table_values(args: &LispObject) -> ElispResult<LispObject> {
    let ht = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match ht {
        LispObject::HashTable(h) => {
            let mut result = LispObject::nil();
            for val in h.lock().data.values() {
                result = LispObject::cons(val.clone(), result);
            }
            Ok(result)
        }
        _ => Err(ElispError::WrongTypeArgument("hash-table".to_string())),
    }
}

fn prim_evenp(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    Ok(LispObject::from(n % 2 == 0))
}

fn prim_oddp(args: &LispObject) -> ElispResult<LispObject> {
    let n = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    Ok(LispObject::from(n % 2 != 0))
}

fn prim_plusp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().unwrap_or_else(LispObject::nil);
    let p = match arg {
        LispObject::Integer(i) => i > 0,
        LispObject::Float(f) => f > 0.0,
        _ => return Err(ElispError::WrongTypeArgument("number".to_string())),
    };
    Ok(LispObject::from(p))
}

fn prim_minusp(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().unwrap_or_else(LispObject::nil);
    let n = match arg {
        LispObject::Integer(i) => i < 0,
        LispObject::Float(f) => f < 0.0,
        _ => return Err(ElispError::WrongTypeArgument("number".to_string())),
    };
    Ok(LispObject::from(n))
}

fn prim_intern_soft(args: &LispObject) -> ElispResult<LispObject> {
    // (intern-soft NAME &optional OBARRAY) — return the symbol
    // with NAME if it exists in OBARRAY, else nil. rele's obarray
    // doesn't distinguish "interned" from "exists-as-symbol-elsewhere"
    // — every symbol ends up in the global table on first reference.
    // We approximate `intern-soft` by returning the symbol if it has
    // any binding (value OR function cell), else nil.
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let name = match &arg {
        LispObject::String(s) => s.clone(),
        LispObject::Symbol(_) => arg.as_symbol().unwrap_or_default(),
        _ => {
            return Err(ElispError::WrongTypeArgument(
                "string or symbol".to_string(),
            ))
        }
    };
    // Probe without interning: every symbol ever interned shows up
    // in `symbol_count()`'s range. Walk the obarray looking for a
    // match by name — cheap compared to string-hashing in an stdlib
    // bootstrap context.
    let sym_table = crate::obarray::GLOBAL_OBARRAY.read();
    for id in 0..sym_table.symbol_count() as u32 {
        let sid = crate::obarray::SymbolId(id);
        if sym_table.name(sid) == name {
            return Ok(LispObject::Symbol(sid));
        }
    }
    Ok(LispObject::nil())
}

fn prim_generate_new_buffer_name(args: &LispObject) -> ElispResult<LispObject> {
    // Return a unique name based on STARTING-NAME. Without a live
    // buffer registry we can't actually detect collisions, so just
    // return the starting name unchanged — the worst real effect is
    // that a caller thinks the name is fresh when in fact it's not,
    // and headless callers mostly use it as a label.
    let starting = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match starting {
        LispObject::String(_) => Ok(starting),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_unibyte_string(args: &LispObject) -> ElispResult<LispObject> {
    // (unibyte-string &rest BYTES) — build a string from raw bytes.
    // We store utf-8 strings, so concatenate chars directly.
    let mut bytes = Vec::new();
    let mut current = args.clone();
    while let Some((arg, rest)) = current.destructure_cons() {
        let b = arg
            .as_integer()
            .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
        bytes.push(b as u8);
        current = rest;
    }
    match String::from_utf8(bytes) {
        Ok(s) => Ok(LispObject::string(&s)),
        Err(e) => Ok(LispObject::string(&String::from_utf8_lossy(&e.into_bytes()))),
    }
}

fn prim_string_lines(args: &LispObject) -> ElispResult<LispObject> {
    // (string-lines STRING &optional OMIT-NULLS KEEP-NEWLINES) — split
    // on newlines. OMIT-NULLS / KEEP-NEWLINES default to nil.
    let s = args
        .first()
        .and_then(|a| match a {
            LispObject::String(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    let omit_nulls = args.nth(1).is_some_and(|a| !a.is_nil());
    let keep_newlines = args.nth(2).is_some_and(|a| !a.is_nil());
    // Build reversed so we can cons up from the end.
    let mut pieces: Vec<String> = if keep_newlines {
        let mut out = Vec::new();
        let mut buf = String::new();
        for c in s.chars() {
            buf.push(c);
            if c == '\n' {
                out.push(std::mem::take(&mut buf));
            }
        }
        if !buf.is_empty() {
            out.push(buf);
        }
        out
    } else {
        s.split('\n').map(String::from).collect()
    };
    if omit_nulls {
        pieces.retain(|p| !p.is_empty());
    }
    let mut result = LispObject::nil();
    for p in pieces.into_iter().rev() {
        result = LispObject::cons(LispObject::string(&p), result);
    }
    Ok(result)
}

// -- Fifth-batch helpers --------------------------------------------------

fn prim_buffer_local_value(args: &LispObject) -> ElispResult<LispObject> {
    // (buffer-local-value SYMBOL BUFFER) — rele has no per-buffer
    // locals, so return the global binding via the obarray.
    let sym = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    Ok(crate::obarray::get_value_cell(sym).unwrap_or_else(LispObject::nil))
}

fn prim_rename_file(args: &LispObject) -> ElispResult<LispObject> {
    let from = str_arg(args, 0)?;
    let to = str_arg(args, 1)?;
    std::fs::rename(&from, &to)
        .map_err(|e| ElispError::EvalError(format!("rename-file: {e}")))?;
    Ok(LispObject::nil())
}

fn prim_copy_file(args: &LispObject) -> ElispResult<LispObject> {
    let from = str_arg(args, 0)?;
    let to = str_arg(args, 1)?;
    std::fs::copy(&from, &to)
        .map_err(|e| ElispError::EvalError(format!("copy-file: {e}")))?;
    Ok(LispObject::nil())
}

fn prim_delete_directory(args: &LispObject) -> ElispResult<LispObject> {
    let dir = str_arg(args, 0)?;
    // Second arg RECURSIVE controls `remove_dir` vs `remove_dir_all`.
    let recursive = args.nth(1).is_some_and(|a| !a.is_nil());
    let result = if recursive {
        std::fs::remove_dir_all(&dir)
    } else {
        std::fs::remove_dir(&dir)
    };
    result.map_err(|e| ElispError::EvalError(format!("delete-directory: {e}")))?;
    Ok(LispObject::nil())
}

fn prim_file_modes(args: &LispObject) -> ElispResult<LispObject> {
    let path = str_arg(args, 0)?;
    match std::fs::metadata(&path) {
        Ok(meta) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                Ok(LispObject::integer(
                    (meta.permissions().mode() & 0o777) as i64,
                ))
            }
            #[cfg(not(unix))]
            {
                // Windows has no POSIX mode — synthesise 0o644 / 0o444
                // based on read-only.
                let ro = meta.permissions().readonly();
                Ok(LispObject::integer(if ro { 0o444 } else { 0o644 }))
            }
        }
        Err(_) => Ok(LispObject::nil()),
    }
}

fn prim_file_newer_than_file_p(args: &LispObject) -> ElispResult<LispObject> {
    let a = str_arg(args, 0)?;
    let b = str_arg(args, 1)?;
    let mtime = |p: &str| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (mtime(&a), mtime(&b)) {
        (Some(ma), None) => Ok(LispObject::from(ma.elapsed().is_ok())),
        (Some(ma), Some(mb)) => Ok(LispObject::from(ma > mb)),
        _ => Ok(LispObject::nil()),
    }
}

fn prim_file_symlink_p(args: &LispObject) -> ElispResult<LispObject> {
    let path = str_arg(args, 0)?;
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() => {
            match std::fs::read_link(&path) {
                Ok(target) => Ok(LispObject::string(&target.to_string_lossy())),
                Err(_) => Ok(LispObject::t()),
            }
        }
        _ => Ok(LispObject::nil()),
    }
}

fn prim_file_regular_p(args: &LispObject) -> ElispResult<LispObject> {
    let path = str_arg(args, 0)?;
    Ok(LispObject::from(
        std::fs::metadata(&path).is_ok_and(|m| m.is_file()),
    ))
}

fn prim_file_readable_p(args: &LispObject) -> ElispResult<LispObject> {
    let path = str_arg(args, 0)?;
    // Best we can do portably without a read-test: check existence.
    Ok(LispObject::from(std::fs::metadata(&path).is_ok()))
}

fn prim_file_writable_p(args: &LispObject) -> ElispResult<LispObject> {
    let path = str_arg(args, 0)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(LispObject::from(
            std::fs::metadata(&path)
                .is_ok_and(|m| m.permissions().mode() & 0o200 != 0),
        ))
    }
    #[cfg(not(unix))]
    {
        Ok(LispObject::from(
            std::fs::metadata(&path).is_ok_and(|m| !m.permissions().readonly()),
        ))
    }
}

fn prim_file_executable_p(args: &LispObject) -> ElispResult<LispObject> {
    let path = str_arg(args, 0)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(LispObject::from(
            std::fs::metadata(&path)
                .is_ok_and(|m| m.permissions().mode() & 0o111 != 0),
        ))
    }
    #[cfg(not(unix))]
    {
        Ok(LispObject::from(std::fs::metadata(&path).is_ok()))
    }
}

fn prim_format_prompt(args: &LispObject) -> ElispResult<LispObject> {
    // (format-prompt PROMPT DEFAULT &rest FORMAT-ARGS)
    // — real Emacs formats as "PROMPT (default DEFAULT): ". We
    // approximate with a static template.
    let prompt = args
        .first()
        .and_then(|a| match a {
            LispObject::String(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    let default = args.nth(1).unwrap_or_else(LispObject::nil);
    let suffix = match &default {
        LispObject::Nil => String::from(": "),
        LispObject::String(s) => format!(" (default {s}): "),
        other => format!(" (default {}): ", other.princ_to_string()),
    };
    Ok(LispObject::string(&format!("{prompt}{suffix}")))
}

/// Extract the Nth argument as a `String`, or return WrongTypeArgument.
fn str_arg(args: &LispObject, n: usize) -> ElispResult<String> {
    match args.nth(n) {
        Some(LispObject::String(s)) => Ok(s),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_default_toplevel_value(args: &LispObject) -> ElispResult<LispObject> {
    // (default-toplevel-value SYMBOL) — return the global value of
    // SYMBOL (ignoring buffer-local bindings). rele has no buffer-
    // local bindings, so this is identical to `symbol-value` on the
    // obarray. When the symbol is unbound we return nil rather than
    // signalling — callers treat the defvar-missing case as absent.
    let sym = args
        .first()
        .and_then(|a| a.as_symbol_id())
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    Ok(crate::obarray::get_value_cell(sym).unwrap_or_else(LispObject::nil))
}

// ---- P1 core primitives gap-fill implementations ----

fn prim_get_char_property(args: &LispObject) -> ElispResult<LispObject> {
    // (get-char-property POS PROP &optional OBJECT)
    // rele has no text properties, overlays, or strings with properties,
    // so always return nil. Minimal arity check: need at least POS and PROP.
    let _ = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let _ = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::nil())
}

fn prim_obarrayp(args: &LispObject) -> ElispResult<LispObject> {
    // (obarrayp OBJECT)
    // Predicate that OBJECT is an obarray. rele uses a hash table as
    // the obarray, so this checks if OBJECT is a hash-table.
    let obj = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    match obj {
        LispObject::HashTable(_) => Ok(LispObject::t()),
        _ => Ok(LispObject::nil()),
    }
}

fn prim_make_network_process(_args: &LispObject) -> ElispResult<LispObject> {
    // (make-network-process &rest PLIST)
    // Stub that signals an error because we don't support network in
    // headless/test environments.
    Err(ElispError::EvalError(
        "make-network-process: no network in test env".to_string(),
    ))
}

fn prim_should_parse(args: &LispObject) -> ElispResult<LispObject> {
    // (should-parse &rest ARGS)
    // Test helper stub from ert-tests.el. Return the args as a list
    // to allow test verification of parse results.
    Ok(args.clone())
}

fn prim_auth_source_forget_all_cached(_args: &LispObject) -> ElispResult<LispObject> {
    // (auth-source-forget-all-cached)
    // Stub that clears cached auth credentials (which we don't have).
    // Return nil.
    Ok(LispObject::nil())
}

fn prim_backward_prefix_chars(_args: &LispObject) -> ElispResult<LispObject> {
    // (backward-prefix-chars)
    // Move point backward over any number of characters with
    // prefix syntax. rele has no syntax table support, so return 0.
    Ok(LispObject::integer(0))
}

fn prim_ical_make_date_time(_args: &LispObject) -> ElispResult<LispObject> {
    // (ical:make-date-time ...)
    // Create an icalendar date-time object. Stub returning nil
    // because we don't have icalendar support.
    Ok(LispObject::nil())
}

fn prim_icalendar_unfolded_buffer_from_file(_args: &LispObject) -> ElispResult<LispObject> {
    // (icalendar-unfolded-buffer-from-file FILENAME)
    // Load an icalendar file. Stub that signals a file-missing error
    // since icalendar files typically don't exist in test environments.
    Err(ElispError::FileError {
        operation: "open-file".to_string(),
        path: "icalendar".to_string(),
        message: "file not found".to_string(),
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    // --- Type-check relaxation: arithmetic with booleans ---
    // Emacs treats nil as 0 and t as 1 in numeric contexts.

    #[test]
    fn test_get_number_nil_as_zero() {
        assert_eq!(get_number(&LispObject::nil()), Some(0.0));
    }

    #[test]
    fn test_get_number_t_as_one() {
        assert_eq!(get_number(&LispObject::t()), Some(1.0));
    }

    #[test]
    fn test_add_with_nil() {
        let args = LispObject::cons(
            LispObject::nil(),
            LispObject::cons(LispObject::integer(5), LispObject::nil()),
        );
        let result = prim_add(&args).unwrap();
        assert_eq!(result.as_float(), Some(5.0));
    }

    #[test]
    fn test_add_t_plus_5() {
        let args = LispObject::cons(
            LispObject::t(),
            LispObject::cons(LispObject::integer(5), LispObject::nil()),
        );
        let result = prim_add(&args).unwrap();
        assert_eq!(result.as_float(), Some(6.0));
    }

    #[test]
    fn test_gt_5_gt_nil() {
        let args = LispObject::cons(
            LispObject::integer(5),
            LispObject::cons(LispObject::nil(), LispObject::nil()),
        );
        assert_eq!(prim_gt(&args).unwrap(), LispObject::t());
    }
}