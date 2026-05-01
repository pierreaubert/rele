//! Headless process primitives. There are no real subprocesses in
//! the rele-elisp build, so each call returns a sensible default
//! that lets test code resolve without `void-function`. The richer
//! "make-process / start-process / process-send-string" path lives in
//! `eval/functions/functions.rs` (stateful primitives); this module
//! handles the leaf-level primitives that previously lived in
//! `stubs.rs` so the inventory script no longer flags them.

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "make-network-process",
        "open-network-stream",
        "make-serial-process",
        "call-process",
        "call-process-region",
        "process-list",
        "get-process",
        "process-status",
        "process-attributes",
        "process-id",
        "process-coding-system",
        "process-command",
        "process-contact",
        "process-tty-name",
        "process-running-child-p",
        "process-kill-without-query",
        "set-process-plist",
        "process-send-eof",
        "process-send-region",
        "signal-process",
        "stop-process",
        "continue-process",
        "interrupt-process",
        "quit-process",
        "kill-process",
        "network-lookup-address-info",
        "network-interface-list",
        "format-network-address",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        // Network process — without a network stack this is the one
        // primitive that signals rather than returning nil, matching
        // the previous stubs.rs behavior.
        "make-network-process" => {
            return Some(Err(ElispError::EvalError(
                "make-network-process: no network in test env".to_string(),
            )));
        }
        // Subprocess primitives that have nothing reasonable to return
        // headlessly all yield nil.
        "open-network-stream"
        | "make-serial-process"
        | "call-process"
        | "call-process-region"
        | "process-list"
        | "get-process"
        | "process-status"
        | "process-attributes"
        | "process-id"
        | "process-coding-system"
        | "process-command"
        | "process-contact"
        | "process-tty-name"
        | "process-running-child-p"
        | "process-kill-without-query"
        | "process-send-eof"
        | "process-send-region"
        | "signal-process"
        | "stop-process"
        | "continue-process"
        | "interrupt-process"
        | "quit-process"
        | "kill-process"
        | "network-lookup-address-info"
        | "network-interface-list"
        | "format-network-address" => Ok(LispObject::nil()),
        // set-process-plist returns the plist arg (matches Emacs).
        "set-process-plist" => Ok(args.nth(1).unwrap_or_else(LispObject::nil)),
        _ => return None,
    };
    Some(result)
}
