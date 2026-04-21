use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "identity" => Some(prim_identity(args)),
        "ignore" => Some(prim_ignore(args)),
        "rele--rx-translate" => Some(prim_rele_rx_translate(args)),
        "autoload-do-load" => Some(prim_autoload_do_load(args)),
        "autoload-compute-prefixes" => Some(Ok(LispObject::nil())),
        "load-history-filename-element" => Some(prim_load_history_filename_element(args)),
        "format-message" => Some(super::string::prim_concat(args)),
        _ => None,
    }
}

pub fn prim_identity(args: &LispObject) -> ElispResult<LispObject> {
    args.first().ok_or(ElispError::WrongNumberOfArguments)
}

pub fn prim_ignore(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args;
    Ok(LispObject::nil())
}

pub fn prim_rele_rx_translate(args: &LispObject) -> ElispResult<LispObject> {
    let form = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    // The argument is a list of rx forms (the &rest from the macro).
    // Concatenate translations of each form (implicit seq).
    let result = rx_seq(&form);
    Ok(LispObject::string(&result))
}

fn rx_symbol_to_regexp(sym: &str) -> Option<&'static str> {
    Some(match sym {
        "bol" | "line-start" => "^",
        "eol" | "line-end" => "$",
        "bos" | "buffer-start" | "bot" | "point-min" | "string-start" => "\\`",
        "eos" | "buffer-end" | "eot" | "point-max" | "string-end" => "\\'",
        "bow" | "word-start" => "\\<",
        "eow" | "word-end" => "\\>",
        "word-boundary" => "\\b",
        "not-word-boundary" => "\\B",
        "symbol-start" => "\\_<",
        "symbol-end" => "\\_>",
        "digit" | "num" | "numeric" => "[[:digit:]]",
        "alpha" | "alphabetic" | "letter" => "[[:alpha:]]",
        "alnum" | "alphanumeric" => "[[:alnum:]]",
        "upper" | "upper-case" => "[[:upper:]]",
        "lower" | "lower-case" => "[[:lower:]]",
        "space" | "whitespace" => "[[:space:]]",
        "blank" => "[[:blank:]]",
        "punct" => "[[:punct:]]",
        "cntrl" | "control" => "[[:cntrl:]]",
        "print" | "printing" => "[[:print:]]",
        "graph" | "graphic" => "[[:graph:]]",
        "hex-digit" | "hex" | "xdigit" => "[[:xdigit:]]",
        "ascii" => "[[:ascii:]]",
        "nonascii" => "[[:nonascii:]]",
        "any" | "anything" | "nonl" | "not-newline" => ".",
        "word" | "wordchar" => "\\w",
        "not-wordchar" => "\\W",
        "unmatchable" => "\\`\\'",
        _ => return None,
    })
}

fn rx_form_to_regexp(form: &LispObject) -> String {
    match form {
        LispObject::Symbol(_) => {
            let Some(name) = form.as_symbol() else {
                return String::new();
            };
            rx_symbol_to_regexp(&name)
                .map(String::from)
                .unwrap_or_default()
        }
        LispObject::String(s) => crate::emacs::search::regexp_quote(s),
        LispObject::Cons(_) => {
            let head = form.first().unwrap_or(LispObject::nil());
            let tail = form.rest().unwrap_or(LispObject::nil());
            let head_name = head.as_symbol().unwrap_or_default();
            match head_name.as_str() {
                "seq" | ":" | "sequence" | "and" | "" => rx_seq(&tail),
                "or" | "|" => rx_or(&tail),
                "zero-or-more" | "0+" | "*" | "*?" => {
                    format!("{}*", rx_seq(&tail))
                }
                "one-or-more" | "1+" | "+" | "+?" => {
                    format!("{}+", rx_seq(&tail))
                }
                "zero-or-one" | "optional" | "opt" | "?" | "??" => {
                    format!("{}?", rx_seq(&tail))
                }
                "group" | "submatch" => {
                    format!("\\({}\\)", rx_seq(&tail))
                }
                "regexp" | "regex" => {
                    tail.first()
                        .and_then(|x| match x {
                            LispObject::String(s) => Some(s.to_string()),
                            _ => None,
                        })
                        .unwrap_or_default()
                }
                "literal" | "eval" => String::new(),
                _ => String::new(),
            }
        }
        _ => String::new(),
    }
}

fn rx_seq(list: &LispObject) -> String {
    let mut out = String::new();
    let mut cur = list.clone();
    while let Some((car, cdr)) = cur.destructure_cons() {
        out.push_str(&rx_form_to_regexp(&car));
        cur = cdr;
    }
    out
}

fn prim_autoload_do_load(args: &LispObject) -> ElispResult<LispObject> {
    let _ = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let funname = args.nth(1);
    if let Some(sym_obj) = funname {
        if let Some(sym_name) = sym_obj.as_symbol() {
            if let Some(sym_id) = crate::obarray::GLOBAL_OBARRAY.read().find(&sym_name) {
                if let Some(def) = crate::obarray::get_function_cell(sym_id) {
                    let is_autoload_stub = def
                        .first()
                        .and_then(|head| head.as_symbol())
                        .is_some_and(|s| s == "autoload");
                    if !def.is_nil() && !is_autoload_stub {
                        return Ok(def);
                    }
                }
            }
        }
    }
    Ok(LispObject::nil())
}

fn prim_load_history_filename_element(args: &LispObject) -> ElispResult<LispObject> {
    let filename_arg = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let filename = match filename_arg.as_string() {
        Some(s) => s.clone(),
        None => return Ok(LispObject::nil()),
    };
    let load_history_id = crate::obarray::intern("load-history");
    let load_history =
        crate::obarray::get_value_cell(load_history_id).unwrap_or(LispObject::nil());
    let mut current = load_history;
    while let Some((entry, rest)) = current.destructure_cons() {
        if let Some((key, _)) = entry.destructure_cons() {
            if let Some(key_str) = key.as_string() {
                if *key_str == filename {
                    return Ok(entry);
                }
            }
        }
        current = rest;
    }
    Ok(LispObject::nil())
}

fn rx_or(list: &LispObject) -> String {
    let alternatives: Vec<String> = {
        let mut cur = list.clone();
        let mut alts = Vec::new();
        while let Some((car, cdr)) = cur.destructure_cons() {
            alts.push(rx_form_to_regexp(&car));
            cur = cdr;
        }
        alts
    };
    if alternatives.len() == 1 {
        alternatives[0].clone()
    } else {
        alternatives.join("\\|")
    }
}