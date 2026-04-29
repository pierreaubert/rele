use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "identity" => Some(prim_identity(args)),
        "ignore" => Some(prim_ignore(args)),
        "rele--rx-translate" => Some(prim_rele_rx_translate(args)),
        "autoload-compute-prefixes" => Some(Ok(LispObject::nil())),
        "connection-local-value" => Some(prim_identity(args)),
        "connection-local-p" => Some(Ok(LispObject::nil())),
        "file-system-info" => Some(Ok(LispObject::nil())),
        "propertized-buffer-identification" => Some(prim_propertized_buffer_identification(args)),
        "substitute-command-keys" => Some(prim_identity(args)),
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

pub fn prim_propertized_buffer_identification(args: &LispObject) -> ElispResult<LispObject> {
    let fmt = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::cons(fmt, LispObject::nil()))
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
                "regexp" | "regex" => tail
                    .first()
                    .and_then(|x| match x {
                        LispObject::String(s) => Some(s.to_string()),
                        _ => None,
                    })
                    .unwrap_or_default(),
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
