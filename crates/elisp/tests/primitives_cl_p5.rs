//! Tests for Stream P5: cl-lib native primitives
//! Testing new implementations of cl-nth-value, cl-mapcan, and enhanced cl-typep.

use rele_elisp::LispObject;
use rele_elisp::primitives_cl;

// ---- cl-nth-value tests ------------------------------------------------

#[test]
fn test_cl_nth_value_zero() {
    // (cl-nth-value 0 FORM) should return FORM
    let form = LispObject::integer(42);
    let args = LispObject::cons(
        LispObject::integer(0),
        LispObject::cons(form.clone(), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_nth_value(&args).unwrap();
    assert_eq!(result, form);
}

#[test]
fn test_cl_nth_value_nonzero() {
    // (cl-nth-value N FORM) where N > 0 should return nil
    let form = LispObject::integer(42);
    let args = LispObject::cons(
        LispObject::integer(1),
        LispObject::cons(form, LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_nth_value(&args).unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_cl_nth_value_large_n() {
    // (cl-nth-value 100 FORM) should return nil
    let form = LispObject::string("test");
    let args = LispObject::cons(
        LispObject::integer(100),
        LispObject::cons(form, LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_nth_value(&args).unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_cl_nth_value_with_symbol() {
    // (cl-nth-value 0 'symbol) should return symbol
    let sym = LispObject::symbol("mysym");
    let args = LispObject::cons(
        LispObject::integer(0),
        LispObject::cons(sym.clone(), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_nth_value(&args).unwrap();
    assert_eq!(result, sym);
}

// ---- cl-mapcan tests ------------------------------------------------

#[test]
fn test_cl_mapcan_empty() {
    // (cl-mapcan PRED '()) should return nil
    let pred = LispObject::symbol("identity");
    let args = LispObject::cons(pred, LispObject::cons(LispObject::nil(), LispObject::nil()));
    let result = primitives_cl::prim_cl_mapcan(&args).unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_cl_mapcan_single_list() {
    // (cl-mapcan PRED '(1 2 3)) should process elements
    let pred = LispObject::symbol("identity");
    let list = {
        let mut l = LispObject::nil();
        for i in [3, 2, 1].iter() {
            l = LispObject::cons(LispObject::integer(*i), l);
        }
        l
    };
    let args = LispObject::cons(pred, LispObject::cons(list, LispObject::nil()));
    let result = primitives_cl::prim_cl_mapcan(&args).unwrap();
    // Result should be a list with elements from the input
    // Simplified: just check it's not nil
    assert!(!matches!(result, LispObject::Nil));
}

#[test]
fn test_cl_mapcan_multiple_lists_same_length() {
    // (cl-mapcan PRED '(a b) '(1 2)) should process pairwise
    let pred = LispObject::symbol("cons");
    let list1 = {
        let mut l = LispObject::nil();
        l = LispObject::cons(LispObject::symbol("b"), l);
        l = LispObject::cons(LispObject::symbol("a"), l);
        l
    };
    let list2 = {
        let mut l = LispObject::nil();
        l = LispObject::cons(LispObject::integer(2), l);
        l = LispObject::cons(LispObject::integer(1), l);
        l
    };
    let args = LispObject::cons(
        pred,
        LispObject::cons(list1, LispObject::cons(list2, LispObject::nil())),
    );
    let result = primitives_cl::prim_cl_mapcan(&args).unwrap();
    // Result should contain elements from both lists
    assert!(!matches!(result, LispObject::Nil));
}

// ---- Enhanced cl-typep tests (combinators) -----

#[test]
fn test_cl_typep_basic_integer() {
    // (cl-typep 42 'integer)
    let args = LispObject::cons(
        LispObject::integer(42),
        LispObject::cons(LispObject::symbol("integer"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_basic_string() {
    // (cl-typep "hello" 'string)
    let args = LispObject::cons(
        LispObject::string("hello"),
        LispObject::cons(LispObject::symbol("string"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_basic_symbol() {
    // (cl-typep 'foo 'symbol)
    let args = LispObject::cons(
        LispObject::symbol("foo"),
        LispObject::cons(LispObject::symbol("symbol"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_basic_list() {
    // (cl-typep '(1 2 3) 'list)
    let list = {
        let mut l = LispObject::nil();
        l = LispObject::cons(LispObject::integer(3), l);
        l = LispObject::cons(LispObject::integer(2), l);
        l = LispObject::cons(LispObject::integer(1), l);
        l
    };
    let args = LispObject::cons(
        list,
        LispObject::cons(LispObject::symbol("list"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_basic_null() {
    // (cl-typep nil 'null)
    let args = LispObject::cons(
        LispObject::nil(),
        LispObject::cons(LispObject::symbol("null"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_basic_number() {
    // (cl-typep 42 'number)
    let args = LispObject::cons(
        LispObject::integer(42),
        LispObject::cons(LispObject::symbol("number"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_basic_float() {
    // (cl-typep 3.14 'float)
    let args = LispObject::cons(
        LispObject::float(3.14),
        LispObject::cons(LispObject::symbol("float"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_mismatch() {
    // (cl-typep "hello" 'integer) should be false
    let args = LispObject::cons(
        LispObject::string("hello"),
        LispObject::cons(LispObject::symbol("integer"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_cl_typep_or_combinator() {
    // (cl-typep 42 '(or string integer)) should be true
    let or_type = {
        let mut t = LispObject::nil();
        t = LispObject::cons(LispObject::symbol("integer"), t);
        t = LispObject::cons(LispObject::symbol("string"), t);
        t = LispObject::cons(LispObject::symbol("or"), t);
        t
    };
    let args = LispObject::cons(
        LispObject::integer(42),
        LispObject::cons(or_type, LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_or_combinator_string_match() {
    // (cl-typep "hello" '(or string integer)) should be true
    let or_type = {
        let mut t = LispObject::nil();
        t = LispObject::cons(LispObject::symbol("integer"), t);
        t = LispObject::cons(LispObject::symbol("string"), t);
        t = LispObject::cons(LispObject::symbol("or"), t);
        t
    };
    let args = LispObject::cons(
        LispObject::string("hello"),
        LispObject::cons(or_type, LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_or_combinator_no_match() {
    // (cl-typep '(a b) '(or string integer)) should be false
    let list = {
        let mut l = LispObject::nil();
        l = LispObject::cons(LispObject::symbol("b"), l);
        l = LispObject::cons(LispObject::symbol("a"), l);
        l
    };
    let or_type = {
        let mut t = LispObject::nil();
        t = LispObject::cons(LispObject::symbol("integer"), t);
        t = LispObject::cons(LispObject::symbol("string"), t);
        t = LispObject::cons(LispObject::symbol("or"), t);
        t
    };
    let args = LispObject::cons(list, LispObject::cons(or_type, LispObject::nil()));
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_cl_typep_and_combinator() {
    // (cl-typep 42 '(and number integer)) should be true (both match)
    let and_type = {
        let mut t = LispObject::nil();
        t = LispObject::cons(LispObject::symbol("integer"), t);
        t = LispObject::cons(LispObject::symbol("number"), t);
        t = LispObject::cons(LispObject::symbol("and"), t);
        t
    };
    let args = LispObject::cons(
        LispObject::integer(42),
        LispObject::cons(and_type, LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_and_combinator_partial_match() {
    // (cl-typep "hello" '(and atom string)) should be true
    let and_type = {
        let mut t = LispObject::nil();
        t = LispObject::cons(LispObject::symbol("string"), t);
        t = LispObject::cons(LispObject::symbol("atom"), t);
        t = LispObject::cons(LispObject::symbol("and"), t);
        t
    };
    let args = LispObject::cons(
        LispObject::string("hello"),
        LispObject::cons(and_type, LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_and_combinator_no_match() {
    // (cl-typep "hello" '(and integer string)) should be false
    let and_type = {
        let mut t = LispObject::nil();
        t = LispObject::cons(LispObject::symbol("string"), t);
        t = LispObject::cons(LispObject::symbol("integer"), t);
        t = LispObject::cons(LispObject::symbol("and"), t);
        t
    };
    let args = LispObject::cons(
        LispObject::string("hello"),
        LispObject::cons(and_type, LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_cl_typep_not_combinator() {
    // (cl-typep 42 '(not string)) should be true
    let not_type = {
        let mut t = LispObject::nil();
        t = LispObject::cons(LispObject::symbol("string"), t);
        t = LispObject::cons(LispObject::symbol("not"), t);
        t
    };
    let args = LispObject::cons(
        LispObject::integer(42),
        LispObject::cons(not_type, LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_not_combinator_negation() {
    // (cl-typep "hello" '(not string)) should be false
    let not_type = {
        let mut t = LispObject::nil();
        t = LispObject::cons(LispObject::symbol("string"), t);
        t = LispObject::cons(LispObject::symbol("not"), t);
        t
    };
    let args = LispObject::cons(
        LispObject::string("hello"),
        LispObject::cons(not_type, LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_cl_typep_member_combinator() {
    // (cl-typep 42 '(member 10 42 99)) should be true
    let member_type = {
        let mut t = LispObject::nil();
        t = LispObject::cons(LispObject::integer(99), t);
        t = LispObject::cons(LispObject::integer(42), t);
        t = LispObject::cons(LispObject::integer(10), t);
        t = LispObject::cons(LispObject::symbol("member"), t);
        t
    };
    let args = LispObject::cons(
        LispObject::integer(42),
        LispObject::cons(member_type, LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_member_combinator_no_match() {
    // (cl-typep 50 '(member 10 42 99)) should be false
    let member_type = {
        let mut t = LispObject::nil();
        t = LispObject::cons(LispObject::integer(99), t);
        t = LispObject::cons(LispObject::integer(42), t);
        t = LispObject::cons(LispObject::integer(10), t);
        t = LispObject::cons(LispObject::symbol("member"), t);
        t
    };
    let args = LispObject::cons(
        LispObject::integer(50),
        LispObject::cons(member_type, LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_cl_typep_nested_combinators() {
    // (cl-typep 42 '(or (and number atom) string)) should be true
    let inner_and = {
        let mut t = LispObject::nil();
        t = LispObject::cons(LispObject::symbol("atom"), t);
        t = LispObject::cons(LispObject::symbol("number"), t);
        t = LispObject::cons(LispObject::symbol("and"), t);
        t
    };
    let or_type = {
        let mut t = LispObject::nil();
        t = LispObject::cons(LispObject::symbol("string"), t);
        t = LispObject::cons(inner_and, t);
        t = LispObject::cons(LispObject::symbol("or"), t);
        t
    };
    let args = LispObject::cons(
        LispObject::integer(42),
        LispObject::cons(or_type, LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_t_type() {
    // (cl-typep ANYTHING 't) should always be true
    let args = LispObject::cons(
        LispObject::integer(42),
        LispObject::cons(LispObject::symbol("t"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_function_type() {
    // (cl-typep (lambda () 1) 'function) should be true
    let lambda = LispObject::cons(
        LispObject::symbol("lambda"),
        LispObject::cons(
            LispObject::nil(),
            LispObject::cons(LispObject::integer(1), LispObject::nil()),
        ),
    );
    let args = LispObject::cons(
        lambda,
        LispObject::cons(LispObject::symbol("function"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_atom_type() {
    // (cl-typep 42 'atom) should be true
    let args = LispObject::cons(
        LispObject::integer(42),
        LispObject::cons(LispObject::symbol("atom"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_atom_type_cons_fail() {
    // (cl-typep '(a b) 'atom) should be false
    let list = {
        let mut l = LispObject::nil();
        l = LispObject::cons(LispObject::symbol("b"), l);
        l = LispObject::cons(LispObject::symbol("a"), l);
        l
    };
    let args = LispObject::cons(
        list,
        LispObject::cons(LispObject::symbol("atom"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::nil());
}

#[test]
fn test_cl_typep_sequence_type() {
    // (cl-typep "hello" 'sequence) should be true (strings are sequences)
    let args = LispObject::cons(
        LispObject::string("hello"),
        LispObject::cons(LispObject::symbol("sequence"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_cl_typep_vector_type() {
    // (cl-typep #(a b c) 'vector) should be true
    let vec = LispObject::Vector(std::sync::Arc::new(parking_lot::Mutex::new(vec![
        LispObject::symbol("a"),
        LispObject::symbol("b"),
        LispObject::symbol("c"),
    ])));
    let args = LispObject::cons(
        vec,
        LispObject::cons(LispObject::symbol("vector"), LispObject::nil()),
    );
    let result = primitives_cl::prim_cl_typep(&args).unwrap();
    assert_eq!(result, LispObject::t());
}
