//! Test module for P8 v2 — type-check relaxation audit.
//!
//! This module verifies that our primitive type checks match GNU Emacs semantics:
//! - Operations that should accept certain types DO accept them (positive tests)
//! - Operations that should REJECT certain types STILL reject them (negative tests)
//! - nil and t are NEVER coerced to numbers (regression guard from failed P8 v1)

use rele_elisp::{add_primitives, read, Interpreter, LispObject};

fn eval(code: &str) -> Result<LispObject, String> {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let expr = read(code).map_err(|e| format!("{:?}", e))?;
    interp.eval(expr).map_err(|e| format!("{:?}", e))
}

// ============================================================================
// car / cdr on nil: must return nil, never error
// ============================================================================

#[test]
fn car_on_nil_returns_nil() {
    let result = eval("(car nil)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::nil());
}

#[test]
fn cdr_on_nil_returns_nil() {
    let result = eval("(cdr nil)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::nil());
}

// ============================================================================
// car / cdr on non-lists: must error with WrongTypeArgument
// ============================================================================

#[test]
fn car_on_integer_errors() {
    let result = eval("(car 42)");
    assert!(result.is_err());
    let err_str = format!("{:?}", result.unwrap_err());
    assert!(err_str.contains("WrongTypeArgument"),
            "Expected WrongTypeArgument but got: {}", err_str);
}

#[test]
fn car_on_string_errors() {
    let result = eval("(car \"hello\")");
    assert!(result.is_err());
}

#[test]
fn car_on_symbol_errors() {
    let result = eval("(car 'foo)");
    assert!(result.is_err());
}

#[test]
fn cdr_on_integer_errors() {
    let result = eval("(cdr 42)");
    assert!(result.is_err());
}

// ============================================================================
// length: accepts nil, lists, strings, vectors, hash-tables
// Emacs also accepts char-tables and bool-vectors (which we don't have yet)
// ============================================================================

#[test]
fn length_on_nil_returns_zero() {
    let result = eval("(length nil)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::integer(0));
}

#[test]
fn length_on_list() {
    let result = eval("(length '(a b c))");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::integer(3));
}

#[test]
fn length_on_string() {
    let result = eval("(length \"hello\")");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::integer(5));
}

#[test]
fn length_on_vector() {
    let result = eval("(length [1 2 3])");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::integer(3));
}

#[test]
fn length_on_integer_errors() {
    let result = eval("(length 42)");
    assert!(result.is_err());
}

#[test]
fn length_on_symbol_errors() {
    let result = eval("(length 'foo)");
    assert!(result.is_err());
}

// ============================================================================
// aref: accepts vectors and strings (also char-tables and bool-vectors in Emacs,
// which we don't support yet)
// ============================================================================

#[test]
fn aref_on_vector() {
    let result = eval("(aref [a b c] 1)");
    assert!(result.is_ok());
    let obj = result.unwrap();
    assert_eq!(obj, LispObject::symbol("b"));
}

#[test]
fn aref_on_string_returns_char_code() {
    let result = eval("(aref \"hello\" 1)");
    assert!(result.is_ok());
    // 'e' is char code 101
    assert_eq!(result.unwrap(), LispObject::integer(101));
}

#[test]
fn aref_on_list_errors() {
    let result = eval("(aref '(a b c) 1)");
    assert!(result.is_err());
}

#[test]
fn aref_on_integer_errors() {
    let result = eval("(aref 42 0)");
    assert!(result.is_err());
}

// ============================================================================
// substring: accepts strings and integer indices (not floats per Emacs)
// ============================================================================

#[test]
fn substring_with_integers() {
    let result = eval("(substring \"hello\" 1 3)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::string("el"));
}

#[test]
fn substring_on_list_errors() {
    let result = eval("(substring '(a b c) 1 2)");
    assert!(result.is_err());
}

#[test]
fn substring_with_symbol_as_string_errors() {
    // Emacs does NOT coerce symbols to strings in substring
    let result = eval("(substring 'hello 0 1)");
    assert!(result.is_err());
}

// ============================================================================
// concat: accepts strings, nil, lists of integers, vectors of integers
// ============================================================================

#[test]
fn concat_strings() {
    let result = eval("(concat \"hello\" \" \" \"world\")");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::string("hello world"));
}

#[test]
fn concat_with_nil() {
    let result = eval("(concat \"hello\" nil \"world\")");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::string("helloworld"));
}

#[test]
fn concat_with_list_of_chars() {
    // (concat '(72 101 108 108 111)) => "Hello"
    let result = eval("(concat '(72 101 108 108 111))");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::string("Hello"));
}

#[test]
fn concat_with_vector_of_chars() {
    let result = eval("(concat [72 101 108 108 111])");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::string("Hello"));
}

#[test]
fn concat_with_integer_errors() {
    // concat doesn't accept bare integers
    let result = eval("(concat 42)");
    assert!(result.is_err());
}

// ============================================================================
// Arithmetic: MUST NOT coerce nil or t to numbers
// This is the critical regression guard from failed P8 v1
// ============================================================================

#[test]
fn add_with_nil_errors() {
    let result = eval("(+ nil 5)");
    assert!(result.is_err(), "Arithmetic must reject nil as number");
}

#[test]
fn add_with_t_errors() {
    let result = eval("(+ t 5)");
    assert!(result.is_err(), "Arithmetic must reject t as number");
}

#[test]
fn add_with_integers() {
    let result = eval("(+ 2 3 4)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::integer(9));
}

#[test]
fn add_with_floats() {
    let result = eval("(+ 2.5 3.5)");
    assert!(result.is_ok());
    // Result should be 6.0
    match result.unwrap() {
        LispObject::Float(f) => assert!((f - 6.0).abs() < 0.0001),
        _ => panic!("Expected float"),
    }
}

#[test]
fn add_with_mixed_int_and_float() {
    // Emacs promotes to float when mixing int and float
    let result = eval("(+ 2 3.5)");
    assert!(result.is_ok());
    match result.unwrap() {
        LispObject::Float(f) => assert!((f - 5.5).abs() < 0.0001),
        _ => panic!("Expected float"),
    }
}

#[test]
fn max_with_nil_errors() {
    let result = eval("(max nil 5)");
    assert!(result.is_err(), "max must reject nil");
}

#[test]
fn max_with_t_errors() {
    let result = eval("(max t 5)");
    assert!(result.is_err(), "max must reject t");
}

#[test]
fn min_with_nil_errors() {
    let result = eval("(min nil 5)");
    assert!(result.is_err(), "min must reject nil");
}

// ============================================================================
// string-equal: accepts strings AND symbols (relaxation)
// Symbols are coerced to their symbol names
// ============================================================================

#[test]
fn string_equal_with_strings() {
    let result = eval("(string-equal \"hello\" \"hello\")");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::T);
}

#[test]
fn string_equal_string_not_equal() {
    let result = eval("(string-equal \"hello\" \"world\")");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::nil());
}

#[test]
fn string_equal_with_symbols() {
    let result = eval("(string-equal 'hello 'hello)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::T);
}

#[test]
fn string_equal_symbol_vs_string() {
    // 'hello as a symbol should equal \"hello\" as a string
    let result = eval("(string-equal 'hello \"hello\")");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::T);
}

#[test]
fn string_equal_with_integer_errors() {
    // string-equal should not accept integers
    let result = eval("(string-equal \"42\" 42)");
    assert!(result.is_err());
}

// ============================================================================
// upcase / downcase: accepts strings AND characters (as integers)
// ============================================================================

#[test]
fn upcase_string() {
    let result = eval("(upcase \"hello\")");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::string("HELLO"));
}

#[test]
fn upcase_char_code() {
    // 'a' is 97
    let result = eval("(upcase 97)");
    assert!(result.is_ok());
    // 'A' is 65
    assert_eq!(result.unwrap(), LispObject::integer(65));
}

#[test]
fn downcase_string() {
    let result = eval("(downcase \"HELLO\")");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::string("hello"));
}

#[test]
fn downcase_char_code() {
    let result = eval("(downcase 65)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::integer(97));
}

#[test]
fn upcase_with_list_errors() {
    let result = eval("(upcase '(a b c))");
    assert!(result.is_err());
}

#[test]
fn upcase_with_symbol_errors() {
    // upcase accepts strings or chars, NOT symbols
    let result = eval("(upcase 'hello)");
    assert!(result.is_err());
}

// ============================================================================
// nth: requires integer index
// ============================================================================

#[test]
fn nth_with_integer_index() {
    let result = eval("(nth 1 '(a b c))");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::symbol("b"));
}

#[test]
fn nth_with_symbol_index_errors() {
    let result = eval("(nth 'one '(a b c))");
    assert!(result.is_err());
}

// ============================================================================
// make-string: requires integer length and character code
// ============================================================================

#[test]
fn make_string_with_integers() {
    let result = eval("(make-string 3 65)"); // 3 times 'A'
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::string("AAA"));
}

#[test]
fn make_string_with_float_length_errors() {
    let result = eval("(make-string 3.5 65)");
    assert!(result.is_err(), "make-string requires integer length");
}

#[test]
fn make_string_with_float_char_errors() {
    let result = eval("(make-string 3 65.5)");
    assert!(result.is_err(), "make-string requires integer char code");
}

// ============================================================================
// Verify type predicates don't coerce
// ============================================================================

#[test]
fn numberp_rejects_nil() {
    let result = eval("(numberp nil)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::nil());
}

#[test]
fn numberp_rejects_t() {
    let result = eval("(numberp t)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::nil());
}

#[test]
fn numberp_accepts_integer() {
    let result = eval("(numberp 42)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::T);
}

#[test]
fn numberp_accepts_float() {
    let result = eval("(numberp 3.14)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::T);
}

#[test]
fn integerp_rejects_float() {
    let result = eval("(integerp 3.14)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::nil());
}

#[test]
fn stringp_rejects_symbol() {
    let result = eval("(stringp 'hello)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::nil());
}

#[test]
fn symbolp_rejects_string() {
    let result = eval("(symbolp \"hello\")");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispObject::nil());
}
