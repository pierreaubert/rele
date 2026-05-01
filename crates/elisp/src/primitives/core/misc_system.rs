//! Miscellaneous system primitives. Mostly host-OS introspection
//! helpers that return reasonable defaults headlessly.

use crate::error::ElispResult;
use crate::object::LispObject;

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "load-average",
        "num-processors",
        "daemonp",
        "kill-emacs",
        "pdumper-stats",
        "system-name",
        "locale-info",
        "emacs-pid",
        "byteorder",
        "recursion-depth",
        "x-display-list",
        "x-list-fonts",
        "x-get-resource",
        "x-parse-geometry",
        "x-popup-menu",
        "x-popup-dialog",
        "popup-menu",
        "menu-bar-lines",
        "menu-bar-mode",
        "tool-bar-mode",
        "tab-bar-mode",
        "frame-terminal",
        "terminal-list",
        "terminal-live-p",
        "track-mouse",
        "mouse-position",
        "mouse-pixel-position",
        "set-mouse-position",
        "handle-switch-frame",
        "make-frame-visible",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let _ = args;
    let result = match name {
        "load-average" => Ok(LispObject::nil()),
        "num-processors" => Ok(LispObject::integer(1)),
        "daemonp" => Ok(LispObject::nil()),
        "kill-emacs" => Ok(LispObject::nil()),
        "pdumper-stats" => Ok(LispObject::nil()),
        "system-name" => Ok(LispObject::string("rele")),
        "locale-info" => Ok(LispObject::nil()),
        "emacs-pid" => Ok(LispObject::integer(1)),
        "byteorder" => Ok(LispObject::integer(if cfg!(target_endian = "little") {
            108
        } else {
            66
        })),
        "recursion-depth" => Ok(LispObject::integer(0)),
        "x-display-list" | "x-list-fonts" => Ok(LispObject::nil()),
        "x-get-resource" => Ok(LispObject::nil()),
        "x-parse-geometry" => Ok(LispObject::nil()),
        "x-popup-menu" | "x-popup-dialog" | "popup-menu" => Ok(LispObject::nil()),
        "menu-bar-lines" | "menu-bar-mode" | "tool-bar-mode" | "tab-bar-mode" => {
            Ok(LispObject::nil())
        }
        "frame-terminal" | "terminal-list" => Ok(LispObject::nil()),
        "terminal-live-p" => Ok(LispObject::t()),
        "track-mouse" | "mouse-position" | "mouse-pixel-position" | "set-mouse-position" => {
            Ok(LispObject::nil())
        }
        "handle-switch-frame" | "make-frame-visible" => Ok(LispObject::nil()),
        _ => return None,
    };
    Some(result)
}
