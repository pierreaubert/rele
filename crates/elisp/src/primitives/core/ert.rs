use crate::error::{ElispError, ElispResult, SignalData};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "ert-get-test" => Some(prim_ert_get_test(args)),
        "ert-test-boundp" => Some(prim_ert_test_boundp(args)),
        "ert-running-test" => Some(prim_ert_running_test(args)),
        "ert-test-name" => Some(prim_ert_test_name(args)),
        "ert-test-tags" => Some(prim_ert_test_tags(args)),
        "ert-test-body" => Some(prim_ert_test_body(args)),
        "ert-test-most-recent-result" => Some(Ok(LispObject::nil())),
        "ert-test-expected-result-type" => Some(Ok(LispObject::symbol(":passed"))),
        "ert-test-documentation" => Some(prim_ert_test_documentation(args)),
        "ert-test-file-name" => Some(prim_ert_test_file_name(args)),
        "ert-test-result-duration" => Some(Ok(LispObject::integer(0))),
        "ert-fail" => Some(prim_ert_fail(args)),
        "ert-skip" => Some(prim_ert_skip(args)),
        _ => None,
    }
}

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in ERT_PRIMITIVE_NAMES {
        interp.define(name, LispObject::primitive(name));
    }
}

pub const ERT_PRIMITIVE_NAMES: &[&str] = &[
    "ert-get-test",
    "ert-test-boundp",
    "ert-running-test",
    "ert-test-name",
    "ert-test-tags",
    "ert-test-body",
    "ert-test-most-recent-result",
    "ert-test-expected-result-type",
    "ert-test-documentation",
    "ert-test-file-name",
    "ert-test-result-duration",
    "ert-fail",
    "ert-skip",
];

/// Tag used on the head of our fake ert-test list.
const RELE_ERT_TEST_TAG: &str = "rele-ert-test";

thread_local! {
    /// The `ert-test` struct object for the currently-running test, or
    /// nil when no test is active. Set by the runner around each
    /// `(funcall BODY)` call.
    pub(crate) static CURRENT_ERT_TEST: std::cell::RefCell<LispObject>
        = std::cell::RefCell::new(LispObject::nil());
}

/// Record the `ert-test` struct for the currently-running test. Pass
/// nil to clear it.
pub fn set_current_ert_test(test: LispObject) {
    CURRENT_ERT_TEST.with(|cell| *cell.borrow_mut() = test);
}

/// Build the shared ert-test object for a test registered via
/// `ert-deftest`. `tags` is the `:tags` form (nil if absent). `body`
/// is the captured closure so `(funcall (ert-test-body ...))` works
/// from wrapper macros.
pub fn make_ert_test_obj(
    name: &str,
    tags: LispObject,
    docstring: LispObject,
    body: LispObject,
) -> LispObject {
    // (rele-ert-test NAME TAGS DOC BODY FILE)
    let mut tail = LispObject::nil();
    tail = LispObject::cons(LispObject::nil(), tail); // FILE slot
    tail = LispObject::cons(body, tail);
    tail = LispObject::cons(docstring, tail);
    tail = LispObject::cons(tags, tail);
    tail = LispObject::cons(LispObject::symbol(name), tail);
    LispObject::cons(LispObject::symbol(RELE_ERT_TEST_TAG), tail)
}

/// Destructure a `(rele-ert-test NAME TAGS DOC BODY FILE)` cons into
/// its five slots.
fn unpack_rele_ert_test(
    obj: &LispObject,
) -> Option<(LispObject, LispObject, LispObject, LispObject, LispObject)> {
    let (head, rest) = obj.destructure_cons()?;
    if head.as_symbol().as_deref() != Some(RELE_ERT_TEST_TAG) {
        return None;
    }
    let (name, rest) = rest.destructure_cons()?;
    let (tags, rest) = rest.destructure_cons()?;
    let (doc, rest) = rest.destructure_cons()?;
    let (body, rest) = rest.destructure_cons()?;
    let file = rest
        .destructure_cons()
        .map(|(f, _)| f)
        .unwrap_or_else(LispObject::nil);
    Some((name, tags, doc, body, file))
}

fn prim_ert_get_test(args: &LispObject) -> ElispResult<LispObject> {
    let sym = match args.first().and_then(|a| a.as_symbol_id()) {
        Some(s) => s,
        None => return Ok(LispObject::nil()),
    };
    let key = crate::obarray::intern("ert--test");
    let v = crate::obarray::get_plist(sym, key);
    if v.is_nil() {
        Ok(LispObject::nil())
    } else {
        Ok(v)
    }
}

fn prim_ert_test_boundp(args: &LispObject) -> ElispResult<LispObject> {
    let sym = match args.first().and_then(|a| a.as_symbol_id()) {
        Some(s) => s,
        None => return Ok(LispObject::nil()),
    };
    let key = crate::obarray::intern("ert--test");
    if crate::obarray::get_plist(sym, key).is_nil() {
        Ok(LispObject::nil())
    } else {
        Ok(LispObject::t())
    }
}

fn prim_ert_running_test(_args: &LispObject) -> ElispResult<LispObject> {
    Ok(CURRENT_ERT_TEST.with(|cell| cell.borrow().clone()))
}

fn prim_ert_test_name(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or_else(LispObject::nil);
    if obj.is_nil() {
        return Ok(LispObject::nil());
    }
    if let Some((name, _, _, _, _)) = unpack_rele_ert_test(&obj) {
        return Ok(name);
    }
    Ok(LispObject::nil())
}

fn prim_ert_test_tags(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or_else(LispObject::nil);
    if obj.is_nil() {
        return Ok(LispObject::nil());
    }
    if let Some((_, tags, _, _, _)) = unpack_rele_ert_test(&obj) {
        return Ok(tags);
    }
    Ok(LispObject::nil())
}

fn prim_ert_test_body(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or_else(LispObject::nil);
    if obj.is_nil() {
        return Ok(LispObject::nil());
    }
    if let Some((_, _, _, body, _)) = unpack_rele_ert_test(&obj) {
        return Ok(body);
    }
    Ok(LispObject::nil())
}

fn prim_ert_test_documentation(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or_else(LispObject::nil);
    if obj.is_nil() {
        return Ok(LispObject::nil());
    }
    if let Some((_, _, doc, _, _)) = unpack_rele_ert_test(&obj) {
        return Ok(doc);
    }
    Ok(LispObject::nil())
}

fn prim_ert_test_file_name(args: &LispObject) -> ElispResult<LispObject> {
    let obj = args.first().unwrap_or_else(LispObject::nil);
    if obj.is_nil() {
        return Ok(LispObject::nil());
    }
    if let Some((_, _, _, _, file)) = unpack_rele_ert_test(&obj) {
        return Ok(file);
    }
    Ok(LispObject::nil())
}

fn prim_ert_fail(args: &LispObject) -> ElispResult<LispObject> {
    let data = args.first().unwrap_or_else(LispObject::nil);
    Err(ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("ert-test-failed"),
        data: LispObject::cons(data, LispObject::nil()),
    })))
}

fn prim_ert_skip(args: &LispObject) -> ElispResult<LispObject> {
    let data = args.first().unwrap_or_else(LispObject::nil);
    Err(ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("ert-test-skipped"),
        data: LispObject::cons(data, LispObject::nil()),
    })))
}
