//! R8: cl-defmethod with qualifier like `:printer` must not be invoked
//! on the primary dispatch path, and must not appear to be a "void function"
//! when the generic is called.

use rele_elisp::add_primitives;
use rele_elisp::eval::Interpreter;
use rele_elisp::LispObject;

fn make_interp() -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp
}

fn eval(src: &str) -> LispObject {
    let interp = make_interp();
    interp
        .eval_source(src)
        .unwrap_or_else(|(_i, e)| panic!("eval error in {:?}: {:?}", src, e))
}

/// A `cl-defmethod` with a qualifier keyword (`:printer`) must NOT be
/// the method called on a normal dispatch. Calling the generic after both
/// a primary and a qualified method were defined must route to the primary.
#[test]
fn qualified_method_does_not_shadow_primary() {
    // Order: primary first, qualified second. Without the fix, the qualified
    // definition overwrites the primary and `(foo 5)` returns 42.
    let result = eval(
        r#"
        (cl-defgeneric foo (x))
        (cl-defmethod foo (x) x)
        (cl-defmethod foo :printer (x) 42)
        (foo 5)
        "#,
    );
    assert_eq!(
        result.as_integer(),
        Some(5),
        "primary (foo (x) x) must be the effective method, not the :printer qualifier"
    );
}

/// Reverse order: qualified first, primary second.
#[test]
fn primary_defined_after_qualifier_wins() {
    let result = eval(
        r#"
        (cl-defgeneric foo2 (x))
        (cl-defmethod foo2 :printer (x) 42)
        (cl-defmethod foo2 (x) x)
        (foo2 5)
        "#,
    );
    assert_eq!(result.as_integer(), Some(5));
}

/// Only a qualified method exists — calling the generic should fall through
/// to the `cl-defgeneric` default (which we install as an empty body
/// returning nil). It must NOT signal "void function: :printer".
#[test]
fn only_qualified_method_does_not_signal_void_function() {
    let interp = make_interp();
    let res = interp.eval_source(
        r#"
        (cl-defgeneric foo3 (x))
        (cl-defmethod foo3 :printer (x) 42)
        (foo3 5)
        "#,
    );
    match res {
        Ok(_) => {} // nil is fine; any non-error return is acceptable
        Err((_i, e)) => {
            let msg = format!("{:?}", e);
            assert!(
                !msg.contains(":printer"),
                "must not signal 'void function: :printer': got {:?}",
                e
            );
        }
    }
}

/// `:before`, `:after`, `:around` keyword qualifiers must also be accepted
/// and must not overwrite the primary.
#[test]
fn before_after_around_qualifiers_do_not_shadow_primary() {
    let result = eval(
        r#"
        (cl-defgeneric foo4 (x))
        (cl-defmethod foo4 (x) x)
        (cl-defmethod foo4 :before (x) 'before)
        (cl-defmethod foo4 :after (x) 'after)
        (cl-defmethod foo4 :around (x) 'around)
        (foo4 7)
        "#,
    );
    assert_eq!(result.as_integer(), Some(7));
}
