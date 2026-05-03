//! `libxml-parse-xml-region`, `libxml-parse-html-region`, and the
//! lower-level `xml-parse-{string,region,file}` family. Backed by the
//! pure-Rust `quick-xml` crate, so no system libxml2 install is needed.
//!
//! Output shape mirrors Emacs:
//!   <root attr="v">text<child/></root>
//! becomes
//!   (root ((attr . "v")) "text" (child nil))

use std::path::PathBuf;

use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "libxml-parse-xml-region",
        "libxml-parse-html-region",
        "xml-parse-region",
        "xml-parse-string",
        "xml-parse-file",
        "libxml-available-p",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        "libxml-available-p" => Ok(LispObject::t()),
        "libxml-parse-xml-region" | "xml-parse-region" => parse_region(args, false),
        "libxml-parse-html-region" => parse_region(args, true),
        "xml-parse-string" => parse_string(args),
        "xml-parse-file" => parse_file(args),
        _ => return None,
    };
    Some(result)
}

fn parse_region(args: &LispObject, _html: bool) -> ElispResult<LispObject> {
    let start = args
        .first()
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".into()))?
        .max(0) as usize;
    let end = args
        .nth(1)
        .and_then(|a| a.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".into()))?
        .max(0) as usize;
    let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
    let text = crate::buffer::with_current(|b| b.substring(lo, hi));
    parse_xml_text(&text)
}

fn parse_string(args: &LispObject) -> ElispResult<LispObject> {
    let s = args
        .first()
        .and_then(|a| a.as_string().cloned())
        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    parse_xml_text(&s)
}

fn parse_file(args: &LispObject) -> ElispResult<LispObject> {
    let path = args
        .first()
        .and_then(|a| a.as_string().cloned())
        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    // xml-parse-file is a synchronous Lisp primitive; this is the
    // worker thread that the elisp evaluator runs on, not the UI
    // thread, so blocking I/O is correct.
    #[allow(clippy::disallowed_methods)]
    let text = std::fs::read_to_string(PathBuf::from(path))
        .map_err(|e| ElispError::EvalError(format!("xml-parse-file: {e}")))?;
    parse_xml_text(&text)
}

/// One frame on the parser stack: open tag's name, its attributes
/// (alist string→string), and the LispObject children accumulated so
/// far for that tag.
type OpenFrame = (String, Vec<(String, String)>, Vec<LispObject>);

/// Convert a raw XML/HTML byte slice into the (TAG ATTRS . CHILDREN)
/// list shape Emacs uses.
fn parse_xml_text(text: &str) -> ElispResult<LispObject> {
    let mut reader = Reader::from_str(text);
    reader.config_mut().trim_text(false);
    let mut stack: Vec<OpenFrame> = Vec::new();
    let mut top_level: Vec<LispObject> = Vec::new();
    loop {
        match reader
            .read_event()
            .map_err(|e| ElispError::EvalError(format!("xml: {e}")))?
        {
            Event::Eof => break,
            Event::Start(e) => {
                let (name, attrs) = element_name_and_attrs(&e)?;
                stack.push((name, attrs, Vec::new()));
            }
            Event::End(_) => {
                let Some((name, attrs, children)) = stack.pop() else {
                    continue;
                };
                let node = make_node(&name, &attrs, children);
                if let Some((_, _, parent)) = stack.last_mut() {
                    parent.push(node);
                } else {
                    top_level.push(node);
                }
            }
            Event::Empty(e) => {
                let (name, attrs) = element_name_and_attrs(&e)?;
                let node = make_node(&name, &attrs, Vec::new());
                if let Some((_, _, parent)) = stack.last_mut() {
                    parent.push(node);
                } else {
                    top_level.push(node);
                }
            }
            Event::Text(t) => {
                let raw = t.unescape().unwrap_or_default().into_owned();
                if !raw.is_empty() {
                    if let Some((_, _, parent)) = stack.last_mut() {
                        parent.push(LispObject::string(&raw));
                    } else {
                        top_level.push(LispObject::string(&raw));
                    }
                }
            }
            Event::CData(c) => {
                let raw = String::from_utf8(c.into_inner().to_vec())
                    .unwrap_or_default();
                if let Some((_, _, parent)) = stack.last_mut() {
                    parent.push(LispObject::string(&raw));
                } else {
                    top_level.push(LispObject::string(&raw));
                }
            }
            Event::Comment(c) => {
                let raw = String::from_utf8(c.as_ref().to_vec()).unwrap_or_default();
                let node = make_node("comment", &[], vec![LispObject::string(&raw)]);
                if let Some((_, _, parent)) = stack.last_mut() {
                    parent.push(node);
                } else {
                    top_level.push(node);
                }
            }
            _ => {}
        }
    }
    if top_level.len() == 1 {
        Ok(top_level.into_iter().next().unwrap_or_else(LispObject::nil))
    } else if top_level.is_empty() {
        Ok(LispObject::nil())
    } else {
        // Multiple top-level nodes (e.g. an element followed by
        // sibling comments) — Emacs wraps them in a synthetic `top`
        // element so the return value is still a single tree.
        Ok(make_node("top", &[], top_level))
    }
}

fn element_name_and_attrs<'a>(
    e: &'a quick_xml::events::BytesStart<'a>,
) -> ElispResult<(String, Vec<(String, String)>)> {
    let name = qname_to_str(e.name());
    let mut attrs = Vec::new();
    for attr in e.attributes().with_checks(false).flatten() {
        let key = qname_to_str(QName(attr.key.as_ref()));
        let value = String::from_utf8_lossy(attr.value.as_ref()).into_owned();
        attrs.push((key, value));
    }
    Ok((name, attrs))
}

fn qname_to_str(name: QName) -> String {
    String::from_utf8_lossy(name.as_ref()).into_owned()
}

fn make_node(
    name: &str,
    attrs: &[(String, String)],
    children: Vec<LispObject>,
) -> LispObject {
    // attrs as alist (((key . value) ...))
    let mut attr_list = LispObject::nil();
    for (k, v) in attrs.iter().rev() {
        attr_list = LispObject::cons(
            LispObject::cons(LispObject::symbol(k), LispObject::string(v)),
            attr_list,
        );
    }
    let mut child_tail = LispObject::nil();
    for child in children.into_iter().rev() {
        child_tail = LispObject::cons(child, child_tail);
    }
    LispObject::cons(
        LispObject::symbol(name),
        LispObject::cons(attr_list, child_tail),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: Vec<LispObject>) -> LispObject {
        items
            .into_iter()
            .rev()
            .fold(LispObject::nil(), |tail, item| LispObject::cons(item, tail))
    }

    #[test]
    fn parse_simple_xml() {
        let r = parse_string(&args(vec![LispObject::string(
            r#"<root attr="v">text<child/></root>"#,
        )]))
        .expect("parse");
        // (root ((attr . "v")) "text" (child nil))
        let s = r.princ_to_string();
        assert!(s.starts_with("(root"), "expected root tag, got {s}");
        assert!(s.contains("\"text\""), "expected text content, got {s}");
        assert!(s.contains("(child"), "expected child element, got {s}");
    }
}
