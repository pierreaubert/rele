/// Integration tests for reader edge cases added in S2.
///
/// Covers:
///   - `#s(TYPE ...)` struct/record literals
///   - `#^[...]` / `#^^[...]` char-table and sub-char-table literals
///   - `#N=FORM` / `#N#` shared-structure labels
///   - `?\C-x`, `?\M-x`, `?\S-x`, `?\H-x`, `?\A-x`, `?\s-x` character modifiers
///   - `?\uNNNN`, `?\UNNNNNNNN` unicode hex escapes
///   - `?\N{UNICODE NAME}` unicode named characters
///   - `?\xNN` hex escapes
use rele_elisp::LispObject;
use rele_elisp::{Interpreter, add_primitives, read, read_all};

// ──────────────────────────────────────────────────────────────────────────────
// #s(…) — record / struct literals
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_record_literal_simple() {
    // #s(my-type field1 field2) → vector [my-type field1 field2]
    let obj = read("#s(my-type 1 2)").unwrap();
    if let LispObject::Vector(v) = obj {
        let v = v.lock();
        assert_eq!(v.len(), 3);
        assert_eq!(v[0], LispObject::symbol("my-type"));
        assert_eq!(v[1], LispObject::integer(1));
        assert_eq!(v[2], LispObject::integer(2));
    } else {
        panic!("expected Vector, got {obj:?}");
    }
}

#[test]
fn test_record_literal_empty() {
    // #s(empty-type) → vector with just the type tag
    let obj = read("#s(empty-type)").unwrap();
    if let LispObject::Vector(v) = obj {
        let v = v.lock();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0], LispObject::symbol("empty-type"));
    } else {
        panic!("expected Vector");
    }
}

#[test]
fn test_record_literal_nested() {
    // #s(outer #s(inner 42))
    let obj = read("#s(outer #s(inner 42))").unwrap();
    if let LispObject::Vector(v) = obj {
        let v = v.lock();
        assert_eq!(v.len(), 2);
        assert_eq!(v[0], LispObject::symbol("outer"));
        // inner should also be a vector
        assert!(matches!(v[1], LispObject::Vector(_)));
    } else {
        panic!("expected Vector");
    }
}

#[test]
fn test_hash_table_literal() {
    // #s(hash-table ...) → hash table object
    let obj = read("#s(hash-table test equal data nil)").unwrap();
    assert!(matches!(obj, LispObject::HashTable(_)));
}

#[test]
fn test_record_in_list() {
    // Records appearing inside regular lists parse correctly
    let forms = read_all("(list #s(point 3 5) #s(point 7 11))").unwrap();
    assert_eq!(forms.len(), 1);
    let list = &forms[0];
    assert!(list.is_cons());
}

// ──────────────────────────────────────────────────────────────────────────────
// #^[…] and #^^[…] — char-table / sub-char-table literals
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_char_table_literal() {
    // #^[nil nil 65 nil] → plain vector [nil nil 65 nil]
    let obj = read("#^[nil nil 65 nil]").unwrap();
    if let LispObject::Vector(v) = obj {
        let v = v.lock();
        assert_eq!(v.len(), 4);
        assert_eq!(v[2], LispObject::integer(65));
    } else {
        panic!("expected Vector for #^[...]");
    }
}

#[test]
fn test_char_table_empty() {
    let obj = read("#^[]").unwrap();
    assert!(matches!(obj, LispObject::Vector(_)));
    if let LispObject::Vector(v) = obj {
        assert_eq!(v.lock().len(), 0);
    }
}

#[test]
fn test_sub_char_table_literal() {
    // #^^[1 2 3] → plain vector [1 2 3]
    let obj = read("#^^[1 2 3]").unwrap();
    if let LispObject::Vector(v) = obj {
        let v = v.lock();
        assert_eq!(v.len(), 3);
        assert_eq!(v[0], LispObject::integer(1));
        assert_eq!(v[1], LispObject::integer(2));
        assert_eq!(v[2], LispObject::integer(3));
    } else {
        panic!("expected Vector for #^^[...]");
    }
}

#[test]
fn test_sub_char_table_in_list() {
    let forms = read_all("(setq tbl #^^[0 1 2])").unwrap();
    assert_eq!(forms.len(), 1);
}

// ──────────────────────────────────────────────────────────────────────────────
// #N= / #N# — shared-structure labels
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_shared_structure_define_and_reference() {
    // #1=foo followed by #1# — both should be the symbol foo
    // (We use a simple atom, not a cons, to avoid Arc aliasing in PartialEq.)
    let forms = read_all("#1=foo #1#").unwrap();
    assert_eq!(forms.len(), 2);
    assert_eq!(forms[0], LispObject::symbol("foo"));
    assert_eq!(forms[1], LispObject::symbol("foo"));
}

#[test]
fn test_shared_structure_multiple_labels() {
    let forms = read_all("#1=foo #2=bar #1# #2#").unwrap();
    assert_eq!(forms.len(), 4);
    assert_eq!(forms[0], LispObject::symbol("foo"));
    assert_eq!(forms[1], LispObject::symbol("bar"));
    assert_eq!(forms[2], LispObject::symbol("foo"));
    assert_eq!(forms[3], LispObject::symbol("bar"));
}

#[test]
fn test_shared_structure_vector() {
    let obj = read("#42=[1 2 3]").unwrap();
    assert!(matches!(obj, LispObject::Vector(_)));
}

#[test]
fn test_shared_structure_in_list() {
    // (a #1=b #1#) — third element is same object as second
    let obj = read("(a #1=hello #1#)").unwrap();
    // Collect elements
    let car = obj.first().unwrap();
    assert_eq!(car, LispObject::symbol("a"));
    let cdr = obj.rest().unwrap();
    let second = cdr.first().unwrap();
    let cdr2 = cdr.rest().unwrap();
    let third = cdr2.first().unwrap();
    assert_eq!(second, LispObject::symbol("hello"));
    assert_eq!(third, LispObject::symbol("hello"));
}

#[test]
fn test_hash_table_circular_shared_data_errors() {
    assert!(read("#s(hash-table data #0=(#0# . #0#))").is_err());
}

// ──────────────────────────────────────────────────────────────────────────────
// Character modifier escapes
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_char_control_modifier() {
    // ?\C-a == 1 (Ctrl+A)
    assert_eq!(read("?\\C-a").unwrap(), LispObject::integer(1));
    // ?\C-b == 2
    assert_eq!(read("?\\C-b").unwrap(), LispObject::integer(2));
    // ?\C-z == 26
    assert_eq!(read("?\\C-z").unwrap(), LispObject::integer(26));
    // ?\^a also == 1 (alternative notation)
    assert_eq!(read("?\\^a").unwrap(), LispObject::integer(1));
}

#[test]
fn test_char_meta_modifier() {
    // ?\M-a == 97 | 0x8000000
    let expected = 97_i64 | 0x8000000_i64;
    assert_eq!(read("?\\M-a").unwrap(), LispObject::integer(expected));
}

#[test]
fn test_char_shift_modifier() {
    // ?\S-a == 97 | 0x2000000
    let expected = 97_i64 | 0x2000000_i64;
    assert_eq!(read("?\\S-a").unwrap(), LispObject::integer(expected));
}

#[test]
fn test_char_hyper_modifier() {
    // ?\H-a == 97 | 0x1000000
    let expected = 97_i64 | 0x1000000_i64;
    assert_eq!(read("?\\H-a").unwrap(), LispObject::integer(expected));
}

#[test]
fn test_char_alt_modifier() {
    // ?\A-a == 97 | 0x400000
    let expected = 97_i64 | 0x400000_i64;
    assert_eq!(read("?\\A-a").unwrap(), LispObject::integer(expected));
}

#[test]
fn test_unfinished_char_modifier_escapes_error() {
    for source in [
        "?\\",
        "?\\^",
        "?\\C",
        "?\\M",
        "?\\S",
        "?\\H",
        "?\\A",
        "?\\C-",
        "?\\M-",
        "?\\S-",
        "?\\H-",
        "?\\A-",
        "?\\s-",
        "?\\C-\\",
        "?\\C-\\M",
        "?\\C-\\M-",
        "?\\x",
        "?\\u",
        "?\\u234",
        "?\\U",
        "?\\U0010010",
        "?\\N",
        "?\\N{",
        "?\\N{SPACE",
    ] {
        assert!(read(source).is_err(), "{source} should signal");
    }
}

#[test]
fn test_char_super_modifier() {
    // ?\s-a == 97 | 0x800000
    let expected = 97_i64 | 0x800000_i64;
    assert_eq!(read("?\\s-a").unwrap(), LispObject::integer(expected));
}

#[test]
fn test_char_meta_control_combined() {
    // ?\M-\C-a == meta of control-a == 1 | 0x8000000
    let expected = 1_i64 | 0x8000000_i64;
    assert_eq!(read("?\\M-\\C-a").unwrap(), LispObject::integer(expected));
}

// ──────────────────────────────────────────────────────────────────────────────
// Unicode hex escapes in character literals
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_char_unicode_small_u() {
    // ?\u03B1 == Greek small letter alpha (U+03B1)
    assert_eq!(
        read("?\\u03B1").unwrap(),
        LispObject::integer('\u{03B1}' as i64)
    );
    // ?\u0041 == 'A'
    assert_eq!(read("?\\u0041").unwrap(), LispObject::integer(65));
}

#[test]
fn test_char_unicode_large_u() {
    // ?\U0001F600 == emoji (U+1F600), if representable
    // Just ensure it parses without error
    let result = read("?\\U0001F600");
    assert!(result.is_ok(), "should parse \\U escape: {result:?}");
}

// ──────────────────────────────────────────────────────────────────────────────
// ?\N{UNICODE NAME} — named unicode character
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_char_named_unicode_latin() {
    // ?\N{LATIN SMALL LETTER A} == 'a' == 97
    assert_eq!(
        read("?\\N{LATIN SMALL LETTER A}").unwrap(),
        LispObject::integer(97)
    );
    assert_eq!(
        read("?\\N{LATIN CAPITAL LETTER Z}").unwrap(),
        LispObject::integer(90)
    );
}

#[test]
fn test_char_named_unicode_greek() {
    // ?\N{GREEK SMALL LETTER ALPHA} == U+03B1
    assert_eq!(
        read("?\\N{GREEK SMALL LETTER ALPHA}").unwrap(),
        LispObject::integer('\u{03B1}' as i64)
    );
    assert_eq!(
        read("?\\N{GREEK CAPITAL LETTER OMEGA}").unwrap(),
        LispObject::integer('\u{03A9}' as i64)
    );
}

#[test]
fn test_char_named_unicode_control() {
    assert_eq!(read("?\\N{NULL}").unwrap(), LispObject::integer(0));
    assert_eq!(read("?\\N{SPACE}").unwrap(), LispObject::integer(32));
    assert_eq!(read("?\\N{NEWLINE}").unwrap(), LispObject::integer(10));
}

#[test]
fn test_char_named_unicode_unknown_yields_null() {
    // Unknown names → U+0000, parsing must not fail
    let obj = read("?\\N{SOME UNKNOWN CHARACTER NAME}").unwrap();
    assert_eq!(obj, LispObject::integer(0));
}

#[test]
fn test_char_named_unicode_in_list() {
    // Named chars inside larger forms parse correctly
    let forms = read_all("(list ?\\N{LATIN SMALL LETTER A} ?\\N{SPACE})").unwrap();
    assert_eq!(forms.len(), 1);
}

// ──────────────────────────────────────────────────────────────────────────────
// Hex escape in char literals (regression / audit)
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_char_hex_escape() {
    assert_eq!(read("?\\x41").unwrap(), LispObject::integer(65)); // 'A'
    assert_eq!(read("?\\xFF").unwrap(), LispObject::integer(255));
    assert_eq!(read("?\\x0A").unwrap(), LispObject::integer(10)); // newline
}

// ──────────────────────────────────────────────────────────────────────────────
// Regression: ensure non-reader forms still parse after edge-case additions
// ──────────────────────────────────────────────────────────────────────────────

#[test]
fn test_regression_normal_forms_unaffected() {
    assert_eq!(read("nil").unwrap(), LispObject::nil());
    assert_eq!(read("t").unwrap(), LispObject::t());
    assert_eq!(read("42").unwrap(), LispObject::integer(42));
    assert_eq!(read("3.25").unwrap(), LispObject::float(3.25));
    assert_eq!(read("\"hello\"").unwrap(), LispObject::string("hello"));
    assert_eq!(read("foo").unwrap(), LispObject::symbol("foo"));
    let list = read("(1 2 3)").unwrap();
    assert!(list.is_cons());
    let vec = read("[1 2 3]").unwrap();
    assert!(matches!(vec, LispObject::Vector(_)));
}

#[test]
fn test_read_buffer_unreadable_object_signals_invalid_read_syntax() {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    let result = interp
        .eval_source(
            r##"(with-temp-buffer
                  (insert "#<symbol lambda at 10>")
                  (goto-char (point-min))
                  (equal (should-error (read (current-buffer)))
                         '(invalid-read-syntax "#<" 1 2)))"##,
        )
        .unwrap();
    assert_eq!(result, LispObject::t());
}

#[test]
fn test_regression_bytecode_literal_still_works() {
    let obj = read("#[257 \"\\x54\\x87\" [] 2]").unwrap();
    assert!(matches!(obj, LispObject::BytecodeFn(_)));
}

#[test]
fn test_regression_function_shorthand() {
    let obj = read("#'my-func").unwrap();
    assert!(obj.is_cons());
    assert_eq!(obj.first().unwrap(), LispObject::symbol("function"));
}
