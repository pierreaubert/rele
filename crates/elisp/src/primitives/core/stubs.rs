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
        | "native-comp-available-p" => Ok(LispObject::nil()),

        // Text-property primitives are real, see primitives/buffer.rs.

        // Buffer/editing stubs
        "backward-prefix-chars" | "undo-boundary" => Ok(LispObject::nil()),
        "buffer-text-pixel-size" => Ok(LispObject::cons(
            LispObject::integer(0),
            LispObject::integer(0),
        )),
        "get-load-suffixes" => Ok(LispObject::nil()),
        "get-truename-buffer" => Ok(LispObject::nil()),
        "backtrace-frame--internal" => Ok(LispObject::nil()),
        "add-minor-mode" => Ok(LispObject::nil()),

        // Process / network primitives are real, see
        // primitives/core/processes.rs (and stateful_make_process in
        // eval/functions/functions.rs for the make-process /
        // start-process / process-* family).
        "make-indirect-buffer" => Ok(LispObject::nil()),
        // `easy-menu-do-define` is now a stateful primitive that
        // populates the menu symbol's value cell — see
        // `crates/elisp/src/eval/functions/functions.rs`.
        "tool-bar-local-item" | "tool-bar-local-item-from-menu" => Ok(LispObject::nil()),

        // Reader / minibuffer primitives are real, see
        // primitives/core/minibuf.rs.

        // Font/charset stubs
        "bidi-resolved-levels" => Ok(LispObject::nil()),

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

        // Variable/binding primitives are real stateful primitives,
        // see eval/functions/functions.rs.

        // User ID stubs
        "user-uid" | "user-real-uid" => Ok(LispObject::integer(1000)),
        "group-gid" | "group-real-gid" => Ok(LispObject::integer(1000)),


        // Coding-system primitives are real, see primitives/core/coding_systems.rs.
        "command-remapping" => Ok(LispObject::nil()),

        // Completion / minibuffer prompts are real, see primitives/core/minibuf.rs.

        // Char/text utilities
        "char-displayable-p" => Ok(LispObject::t()),
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
        // Face / font / color primitives are real, see primitives/core/faces.rs.

        // Menu / tool-bar stubs
        "menu-bar-lines" | "menu-bar-mode" | "tool-bar-mode" | "tab-bar-mode" | "popup-menu"
        | "x-popup-menu" => Ok(LispObject::nil()),

        // Obarray / symbol stubs
        "mapatoms" | "unintern" => Ok(LispObject::nil()),
        "symbol-plist" => Ok(LispObject::nil()),
        "add-to-list" | "add-to-ordered-list" => Ok(LispObject::nil()),
        "remove-from-invisibility-spec" | "add-to-invisibility-spec" => Ok(LispObject::nil()),

        // Buffer ops are real, see primitives/buffer.rs.
        "set-face-underline" | "set-face-strike-through" => Ok(LispObject::nil()),

        // multibyte-char-to-unibyte: see helper.
        "multibyte-char-to-unibyte" => multibyte_char_to_unibyte_impl(args),
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
        "text-char-description" => Ok(super::key_descriptions::text_char_description(args)),
        "single-key-description" => Ok(super::key_descriptions::single_key_description(args)),
        "listify-key-sequence" => Ok(super::key_descriptions::listify_key_sequence(args)),

        // Timer / thread / mutex primitives are real, see
        // primitives/core/timers.rs and primitives/core/threads.rs.

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
        | "posn-object-width-height" => Ok(LispObject::nil()),

        // Keymap-adjacent headless stubs.
        "keyboard-quit" => Ok(LispObject::nil()),

        // Buffer-local stubs
        "generate-new-buffer" => Ok(args.first().unwrap_or(LispObject::nil())),
        "symbol-file" => Ok(LispObject::nil()),

        // Indentation stubs
        "current-column" | "current-indentation" => Ok(LispObject::integer(0)),
        // Mode-dependent indentation (no major-mode infrastructure
        // headlessly). The basic ops live in primitives/buffer.rs.
        "indent-according-to-mode" | "indent-for-tab-command" => Ok(LispObject::nil()),

        // Sexp navigation primitives that still depend on a richer
        // syntax-table model than we ship headlessly.
        "backward-up-list" | "up-list" | "down-list" => Ok(LispObject::nil()),
        // Input-method / recursive-edit primitives live in primitives/core/minibuf.rs.
        "keyboard-escape-quit" => Ok(LispObject::nil()),
        "translation-table-id" => Ok(LispObject::nil()),

        // Hook primitives are real, see eval/functions/functions_2.rs.

        // Abbrev primitives are real, see primitives/core/abbrevs.rs.

        // File visit stubs
        "set-visited-file-name"
        | "clear-visited-file-modtime"
        | "verify-visited-file-modtime"
        | "visited-file-modtime"
        | "file-locked-p"
        | "ask-user-about-lock"
        | "ask-user-about-supersession-threat" => Ok(LispObject::nil()),
        "set-buffer-multibyte" => Ok(args.first().unwrap_or(LispObject::nil())),

        // Kill ring stubs (kill-word/backward-kill-word are real, see
        // primitives/buffer.rs).
        "kill-line" | "kill-whole-line" | "kill-region"
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

        // GUI/X selection primitives are real, see primitives/core/selections.rs.

        // Advice family — `advice-add` / `advice-remove` are now real
        // (no-op) primitives in eval/functions/functions_2.rs. The
        // remaining low-level advice helpers stay as nil-returning.
        "add-function"
        | "remove-function"
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

        // Process primitives are real, see primitives/core/processes.rs
        // and stateful_process_* in eval/functions/functions.rs.

        // XML/JSON stubs
        "xml-parse-string"
        | "xml-parse-region"
        | "xml-parse-file"
        | "libxml-parse-xml-region"
        | "libxml-parse-html-region" => Ok(LispObject::nil()),
        // JSON primitives are real, see primitives/core/json.rs.

        // SQLite primitives are real, see primitives/core/sqlite.rs.

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

        // byteorder lives in primitives/core/misc_system.rs.

        // --- Phase-1 C-level primitive stubs ---

        // Numeric argument helpers
        "prefix-numeric-value" => Ok(LispObject::integer(
            args.first().and_then(|a| a.as_integer()).unwrap_or(1),
        )),

        // File system stubs
        "file-name-all-completions" | "file-name-completion" => Ok(LispObject::nil()),
        "access-file" | "add-name-to-file" => Ok(LispObject::nil()),
        "default-file-modes" => Ok(LispObject::integer(0o644)),
        "file-acl" | "file-selinux-context" | "set-file-acl" => Ok(LispObject::nil()),
        "substitute-in-file-name" => Ok(args.first().unwrap_or(LispObject::nil())),
        "unhandled-file-name-directory" => Ok(LispObject::nil()),

        // self-insert-command: signal on negative count, otherwise no-op.
        "self-insert-command" => match args.first().and_then(|a| a.as_integer()) {
            Some(n) if n < 0 => Err(ElispError::EvalError(
                "Wrong type argument: wholenump, -1".to_string(),
            )),
            _ => Ok(LispObject::nil()),
        },
        // Buffer/editing stubs
        "scroll-up" => Ok(LispObject::nil()),
        "total-line-spacing" => Ok(LispObject::integer(0)),

        // Bool-vector ops are real, see primitives/core/vector.rs.

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
        "byte-to-string" => byte_to_string_impl(args),
        "char-resolve-modifiers" => char_resolve_modifiers_impl(args),
        "secure-hash-algorithms" => Ok(super::key_descriptions::secure_hash_algorithms()),
        "split-char" => Ok(super::key_descriptions::split_char(args)),
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
        // UTF-8 encodes every Unicode codepoint — there are no
        // unencodable chars in our buffer model.
        "unencodable-char-position" => Ok(LispObject::nil()),
        // Records / finalizers
        "make-record" => make_record_impl(args),
        "make-finalizer" => {
            crate::primitives::core::records::register_record_tag("finalizer");
            let func = args.first().unwrap_or_else(LispObject::nil);
            let items = vec![LispObject::symbol("finalizer"), func];
            Ok(LispObject::Vector(std::sync::Arc::new(
                crate::eval::SyncRefCell::new(items),
            )))
        }
        "make-ring" => Ok(LispObject::nil()),
        "ring-convert-sequence-to-ring" => Ok(LispObject::nil()),

        // Hash table literal (Emacs 30)
        "hash-table-literal" => Ok(LispObject::nil()),
        "hash-table-contains-p" => Ok(LispObject::nil()),

        // Obarray
        // obarray primitives are real, see primitives/core/obarrays.rs.

        // Process-related
        "set-process-plist" | "process-contact" | "kill-process" | "kill-emacs" => {
            Ok(LispObject::nil())
        }

        // System info
        "lossage-size" => Ok(LispObject::integer(300)),
        "num-processors" => Ok(LispObject::integer(1)),
        "network-interface-list" => Ok(LispObject::nil()),
        "group-name" => group_name_impl(args),

        // Display/formatting stubs
        "format-mode-line" => Ok(LispObject::string("")),
        "format-network-address" => Ok(LispObject::string("")),
        "format-seconds" => Ok(LispObject::string("")),

        // Misc C primitives
        "set-auto-mode--find-matching-alist-entry" => Ok(LispObject::nil()),
        "setopt" => Ok(LispObject::nil()),

        _ => return None,
    };
    super::stub_telemetry::record_stub_hit(name);
    Some(r)
}

fn byte_to_string_impl(args: &LispObject) -> ElispResult<LispObject> {
    let byte = args
        .first()
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    if !(0..=255).contains(&byte) {
        return Err(ElispError::WrongTypeArgument("character".to_string()));
    }
    let byte =
        u8::try_from(byte).map_err(|_| ElispError::WrongTypeArgument("character".to_string()))?;
    let s = char::from(byte).to_string();
    crate::object::mark_string_multibyte(&s, false);
    Ok(LispObject::string(&s))
}

fn group_name_impl(args: &LispObject) -> ElispResult<LispObject> {
    let Some(group) = args.first() else {
        return Ok(LispObject::nil());
    };
    if group.is_nil() || group.as_integer().is_some() {
        Ok(LispObject::nil())
    } else {
        Err(ElispError::WrongTypeArgument("integer".to_string()))
    }
}

fn multibyte_char_to_unibyte_impl(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let c = arg
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    if (0..256).contains(&c) {
        Ok(LispObject::integer(c))
    } else {
        Ok(LispObject::integer(-1))
    }
}

fn char_resolve_modifiers_impl(args: &LispObject) -> ElispResult<LispObject> {
    let arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let c = arg
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    const SHIFT: i64 = 1 << 25;
    const CTRL: i64 = 1 << 26;
    const CHAR_MASK: i64 = 0x003F_FFFF;
    let mut result = c;
    if result & SHIFT != 0 {
        let base = result & CHAR_MASK;
        if (b'a' as i64..=b'z' as i64).contains(&base) {
            result = (result & !SHIFT & !CHAR_MASK) | (base - 32);
        } else if (b'A' as i64..=b'Z' as i64).contains(&base) {
            result &= !SHIFT;
        }
    }
    if result & CTRL != 0 {
        let base = result & CHAR_MASK;
        let new_base = if (b'@' as i64..=b'_' as i64).contains(&base)
            || (b'a' as i64..=b'z' as i64).contains(&base)
        {
            Some(base & 0x1F)
        } else if base == b'?' as i64 {
            Some(127)
        } else {
            None
        };
        if let Some(nb) = new_base {
            result = (result & !CTRL & !CHAR_MASK) | nb;
        }
    }
    Ok(LispObject::integer(result))
}

fn make_record_impl(args: &LispObject) -> ElispResult<LispObject> {
    let tag = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let length = args
        .nth(1)
        .ok_or(ElispError::WrongNumberOfArguments)?
        .as_integer()
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    let init = args.nth(2).unwrap_or_else(LispObject::nil);
    let length = usize::try_from(length).unwrap_or(0);
    if let Some(t) = tag.as_symbol() {
        crate::primitives::core::records::register_record_tag(&t);
    }
    let mut items = Vec::with_capacity(length + 1);
    items.push(tag);
    for _ in 0..length {
        items.push(init.clone());
    }
    Ok(LispObject::Vector(std::sync::Arc::new(
        crate::eval::SyncRefCell::new(items),
    )))
}
