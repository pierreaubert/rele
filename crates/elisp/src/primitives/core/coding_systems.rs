//! Coding-system shape primitives. Headless build only knows about
//! UTF-8 (and a handful of canonical aliases) — coding-system data
//! beyond this is metadata that returns nil-shaped defaults rather
//! than ever round-tripping bytes.

use crate::error::ElispResult;
use crate::object::LispObject;

const KNOWN_CODING_SYSTEMS: &[&str] = &[
    "utf-8",
    "utf-8-unix",
    "utf-8-dos",
    "utf-8-mac",
    "utf-8-emacs",
    "utf-8-emacs-unix",
    "utf-8-auto",
    "raw-text",
    "raw-text-unix",
    "no-conversion",
    "binary",
    "us-ascii",
    "iso-latin-1",
    "iso-8859-1",
    "latin-1",
    "undecided",
    "undecided-unix",
    "undecided-dos",
];

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "coding-system-p",
        "coding-system-aliases",
        "coding-system-plist",
        "coding-system-base",
        "coding-system-change-eol-conversion",
        "coding-system-priority-list",
        "set-coding-system-priority",
        "prefer-coding-system",
        "keyboard-coding-system",
        "terminal-coding-system",
        "set-keyboard-coding-system",
        "set-terminal-coding-system",
        "find-coding-systems-string",
        "find-coding-systems-region",
        "find-coding-systems-for-charsets",
        "detect-coding-region",
        "detect-coding-string",
        "decode-coding-region",
        "encode-coding-region",
        "decode-coding-string",
        "encode-coding-string",
        "check-coding-systems-region",
        "define-coding-system-internal",
        "define-coding-system-alias",
        "set-safe-terminal-coding-system-internal",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        "coding-system-p" => is_coding_system(args.first().unwrap_or_else(LispObject::nil)),
        "coding-system-aliases" => LispObject::nil(),
        "coding-system-plist" => LispObject::nil(),
        "coding-system-base" | "coding-system-change-eol-conversion" => {
            args.first().unwrap_or_else(LispObject::nil)
        }
        "coding-system-priority-list" => {
            LispObject::cons(LispObject::symbol("utf-8"), LispObject::nil())
        }
        "set-coding-system-priority" | "prefer-coding-system" => LispObject::nil(),
        "keyboard-coding-system" | "terminal-coding-system" => LispObject::symbol("utf-8"),
        "set-keyboard-coding-system" | "set-terminal-coding-system" => LispObject::nil(),
        "find-coding-systems-string"
        | "find-coding-systems-region"
        | "find-coding-systems-for-charsets" => {
            LispObject::cons(LispObject::symbol("utf-8"), LispObject::nil())
        }
        "detect-coding-region" | "detect-coding-string" => LispObject::symbol("utf-8"),
        "decode-coding-region" | "encode-coding-region" => {
            // Buffer is already UTF-8 internally — region coding is a
            // no-op. Return nil (Emacs returns the result coding system).
            LispObject::nil()
        }
        "decode-coding-string" | "encode-coding-string" => {
            args.first().unwrap_or_else(LispObject::nil)
        }
        "check-coding-systems-region" => LispObject::nil(),
        "define-coding-system-internal"
        | "define-coding-system-alias"
        | "set-safe-terminal-coding-system-internal" => LispObject::nil(),
        _ => return None,
    };
    Some(Ok(result))
}

fn is_coding_system(value: LispObject) -> LispObject {
    let Some(name) = value.as_symbol() else {
        return LispObject::nil();
    };
    if KNOWN_CODING_SYSTEMS.contains(&name.as_str()) {
        LispObject::t()
    } else {
        LispObject::nil()
    }
}
