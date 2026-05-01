//! Reader / minibuffer prompt primitives. There is no interactive
//! input in the headless interpreter, so each primitive returns the
//! supplied default (where one exists) or nil.

use crate::error::ElispResult;
use crate::object::LispObject;

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "completing-read",
        "read-from-minibuffer",
        "read-no-blanks-input",
        "read-number",
        "read-buffer",
        "read-file-name",
        "read-directory-name",
        "read-key-sequence",
        "read-key-sequence-vector",
        "read-key",
        "read-char",
        "read-char-exclusive",
        "read-event",
        "read-passwd",
        "yes-or-no-p",
        "y-or-n-p",
        "message-box",
        "ding",
        "beep",
        "minibufferp",
        "minibuffer-depth",
        "minibuffer-message",
        "abort-minibuffers",
        "exit-minibuffer",
        "current-message",
        "input-pending-p",
        "this-command-keys",
        "this-command-keys-vector",
        "this-single-command-keys",
        "this-single-command-raw-keys",
        "recent-keys",
        "recursive-edit",
        "exit-recursive-edit",
        "abort-recursive-edit",
        "top-level",
        "set-input-mode",
        "current-input-mode",
        "execute-kbd-macro",
        "current-input-method",
        "activate-input-method",
        "deactivate-input-method",
        "set-input-method",
        "toggle-input-method",
        "describe-input-method",
        "register-input-method",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        // Prompts that take a DEFAULT argument: return it.
        // Layout matches Emacs's signatures:
        //   (completing-read PROMPT COLLECTION &optional PREDICATE
        //                    REQUIRE-MATCH INITIAL-INPUT HIST DEFAULT-VALUE)
        //   (read-from-minibuffer PROMPT INITIAL KEYMAP READ HIST DEFAULT-VALUE)
        //   (read-string PROMPT &optional INITIAL HISTORY DEFAULT-VALUE)
        // We pick a sensible "default-or-nil" position.
        "completing-read" => Ok(args.nth(6).unwrap_or_else(LispObject::nil)),
        "read-from-minibuffer" => Ok(args.nth(5).unwrap_or_else(LispObject::nil)),
        "read-no-blanks-input" | "read-number" => {
            Ok(args.nth(1).unwrap_or_else(LispObject::nil))
        }
        "read-buffer" | "read-file-name" | "read-directory-name" => {
            Ok(args.nth(2).unwrap_or_else(LispObject::nil))
        }
        "read-passwd" => Ok(LispObject::string("")),
        "yes-or-no-p" | "y-or-n-p" => Ok(LispObject::nil()),
        "message-box" | "ding" | "beep" | "minibuffer-message" => Ok(LispObject::nil()),
        "minibufferp" | "minibuffer-depth" | "abort-minibuffers" | "exit-minibuffer" => {
            Ok(LispObject::nil())
        }
        "current-message" | "input-pending-p" => Ok(LispObject::nil()),
        "read-key-sequence" | "read-key-sequence-vector" | "read-key" | "read-char"
        | "read-char-exclusive" | "read-event" => Ok(LispObject::nil()),
        "this-command-keys" | "this-command-keys-vector" | "this-single-command-keys"
        | "this-single-command-raw-keys" | "recent-keys" => Ok(LispObject::nil()),
        "recursive-edit" | "exit-recursive-edit" | "abort-recursive-edit" | "top-level" => {
            Ok(LispObject::nil())
        }
        "set-input-mode" | "current-input-mode" | "execute-kbd-macro" => Ok(LispObject::nil()),
        "current-input-method"
        | "activate-input-method"
        | "deactivate-input-method"
        | "set-input-method"
        | "toggle-input-method"
        | "describe-input-method"
        | "register-input-method" => Ok(LispObject::nil()),
        _ => return None,
    };
    Some(result)
}
