//! Category, case-table, syntax-table, and charset primitives.
//!
//! These are intentionally lightweight. Emacs has rich per-buffer table
//! machinery; here we preserve the table shapes and the common ASCII
//! answers that bootstrap and ERT need in a headless runtime.

use crate::buffer;
use crate::error::{ElispError, ElispResult};
use crate::eval::InterpreterState;
use crate::object::LispObject;
use std::sync::Arc;

const STANDARD_CASE_TABLE: &str = "rele--standard-case-table";
const STANDARD_CATEGORY_TABLE: &str = "rele--standard-category-table";
const STANDARD_SYNTAX_TABLE: &str = "rele--standard-syntax-table";
const CHARSET_PRIORITY_LIST: &str = "rele--charset-priority-list";

const BUILTIN_CHARSETS: &[&str] = &[
    "ascii",
    "us-ascii",
    "unicode",
    "utf-8",
    "iso-8859-1",
    "latin-1",
    "emacs-mule",
    "eight-bit",
    "binary",
];

pub const CATEGORY_PRIMITIVE_NAMES: &[&str] = &[
    "make-char-table",
    "optimize-char-table",
    "set-char-table-parent",
    "char-table-parent",
    "char-table-range",
    "set-char-table-range",
    "char-table-extra-slot",
    "set-char-table-extra-slot",
    "char-table-subtype",
    "map-char-table",
    "make-category-table",
    "category-table",
    "standard-category-table",
    "set-category-table",
    "copy-category-table",
    "category-table-p",
    "define-category",
    "modify-category-entry",
    "category-docstring",
    "category-set-mnemonics",
    "make-category-set",
    "char-category-set",
    "make-syntax-table",
    "standard-syntax-table",
    "syntax-table",
    "set-syntax-table",
    "copy-syntax-table",
    "syntax-table-p",
    "modify-syntax-entry",
    "char-syntax",
    "syntax-class-to-char",
    "syntax-after",
    "string-to-syntax",
    "standard-case-table",
    "current-case-table",
    "set-standard-case-table",
    "set-case-table",
    "case-table-p",
    "set-case-syntax",
    "set-case-syntax-pair",
    "set-case-syntax-delims",
    "charsetp",
    "char-charset",
    "charset-after",
    "charset-list",
    "charset-priority-list",
    "charset-plist",
    "set-charset-plist",
    "charset-id-internal",
    "charset-dimension",
    "set-charset-priority",
    "clear-charset-maps",
    "declare-equiv-charset",
    "define-charset",
    "define-charset-alias",
    "define-charset-internal",
    "get-charset-property",
    "put-charset-property",
    "map-charset-chars",
    "find-charset-string",
    "find-charset-region",
    "find-coding-systems-for-charsets",
    "get-unused-iso-final-char",
    "iso-charset",
    "sort-charsets",
    "w32-add-charset-info",
];

#[allow(clippy::too_many_lines)]
pub fn call_category_primitive(
    name: &str,
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> Option<ElispResult<LispObject>> {
    Some(match name {
        "make-char-table" => prim_make_char_table(args),
        "optimize-char-table" => Ok(args.first().unwrap_or_else(LispObject::nil)),
        "set-char-table-parent" => prim_set_char_table_parent(args),
        "char-table-parent" => prim_char_table_parent(args),
        "char-table-range" => prim_char_table_range(args),
        "set-char-table-range" => prim_set_char_table_range(args),
        "char-table-extra-slot" => prim_char_table_extra_slot(args),
        "set-char-table-extra-slot" => prim_set_char_table_extra_slot(args),
        "char-table-subtype" => prim_char_table_subtype(args),
        "map-char-table" => Ok(LispObject::nil()),

        "make-category-table" => Ok(make_category_table()),
        "category-table" | "standard-category-table" => Ok(current_table(
            state,
            STANDARD_CATEGORY_TABLE,
            "category-table",
            false,
        )),
        "set-category-table" => prim_set_named_table(args, state, STANDARD_CATEGORY_TABLE),
        "copy-category-table" => prim_copy_named_table(
            args,
            state,
            STANDARD_CATEGORY_TABLE,
            "category-table",
            false,
        ),
        "category-table-p" => prim_table_p(args, "category-table"),
        "define-category" => prim_define_category(args, state),
        "modify-category-entry" => prim_modify_category_entry(args, state),
        "category-docstring" => prim_category_docstring(args, state),
        "category-set-mnemonics" => prim_category_set_mnemonics(args),
        "make-category-set" => prim_make_category_set(args),
        "char-category-set" => prim_char_category_set(args, state),

        "make-syntax-table" => prim_make_or_copy_table(args, "syntax-table"),
        "standard-syntax-table" | "syntax-table" => Ok(current_table(
            state,
            STANDARD_SYNTAX_TABLE,
            "syntax-table",
            false,
        )),
        "set-syntax-table" => prim_set_named_table(args, state, STANDARD_SYNTAX_TABLE),
        "copy-syntax-table" => {
            prim_copy_named_table(args, state, STANDARD_SYNTAX_TABLE, "syntax-table", false)
        }
        "syntax-table-p" => prim_table_p(args, "syntax-table"),
        "modify-syntax-entry" => prim_modify_syntax_entry(args, state),
        "char-syntax" => prim_char_syntax(args, state),
        "syntax-class-to-char" => prim_syntax_class_to_char(args),
        "syntax-after" => prim_syntax_after(args, state),
        "string-to-syntax" => prim_string_to_syntax(args),

        "standard-case-table" | "current-case-table" => Ok(current_table(
            state,
            STANDARD_CASE_TABLE,
            "case-table",
            true,
        )),
        "set-standard-case-table" | "set-case-table" => {
            prim_set_named_table(args, state, STANDARD_CASE_TABLE)
        }
        "case-table-p" => prim_table_p(args, "case-table"),
        "set-case-syntax" | "set-case-syntax-delims" => Ok(LispObject::nil()),
        "set-case-syntax-pair" => prim_set_case_syntax_pair(args, state),

        "charsetp" => prim_charsetp(args, state),
        "char-charset" => prim_char_charset(args),
        "charset-after" => prim_charset_after(args),
        "charset-list" => Ok(list_symbols(BUILTIN_CHARSETS)),
        "charset-priority-list" => prim_charset_priority_list(args, state),
        "charset-plist" => prim_charset_plist(args, state),
        "set-charset-plist" => prim_set_charset_plist(args, state),
        "charset-id-internal" => prim_charset_id(args),
        "charset-dimension" => prim_charset_dimension(args),
        "set-charset-priority" => prim_set_charset_priority(args, state),
        "clear-charset-maps"
        | "declare-equiv-charset"
        | "map-charset-chars"
        | "w32-add-charset-info" => Ok(LispObject::nil()),
        "define-charset" | "define-charset-internal" | "define-charset-alias" => {
            prim_define_charset(args, state)
        }
        "get-charset-property" => prim_get_charset_property(args, state),
        "put-charset-property" => prim_put_charset_property(args, state),
        "find-charset-string" => prim_find_charset_string(args),
        "find-charset-region" => prim_find_charset_region(args),
        "find-coding-systems-for-charsets" => Ok(list_symbols(&["utf-8-unix"])),
        "get-unused-iso-final-char" => Ok(LispObject::nil()),
        "iso-charset" => prim_iso_charset(args),
        "sort-charsets" => prim_sort_charsets(args, state),
        _ => return None,
    })
}

fn prim_make_char_table(args: &LispObject) -> ElispResult<LispObject> {
    let purpose = args.first().unwrap_or_else(LispObject::nil);
    let init = args.nth(1).unwrap_or_else(LispObject::nil);
    Ok(crate::primitives::core::make_char_table(purpose, init))
}

fn prim_set_char_table_parent(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let parent = args.nth(1).unwrap_or_else(LispObject::nil);
    crate::primitives::core::char_table_set_parent(&table, parent)
}

fn prim_char_table_parent(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    crate::primitives::core::char_table_parent(&table)
}

fn prim_char_table_range(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let range = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    crate::primitives::core::char_table_range(&table, &range)
}

fn prim_set_char_table_range(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let range = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    crate::primitives::core::char_table_set_range(&table, &range, value)
}

fn prim_char_table_extra_slot(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let slot = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    crate::primitives::core::char_table_extra_slot(&table, &slot)
}

fn prim_set_char_table_extra_slot(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let slot = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;
    crate::primitives::core::char_table_set_extra_slot(&table, &slot, value)
}

fn prim_char_table_subtype(args: &LispObject) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(crate::primitives::core::char_table_subtype(&table))
}

fn make_category_table() -> LispObject {
    crate::primitives::core::make_char_table(
        LispObject::symbol("category-table"),
        LispObject::string(""),
    )
}

fn make_case_table() -> LispObject {
    let table = crate::primitives::core::make_char_table(
        LispObject::symbol("case-table"),
        LispObject::nil(),
    );
    let upcase = crate::primitives::core::make_char_table(
        LispObject::symbol("case-table"),
        LispObject::nil(),
    );
    let _ =
        crate::primitives::core::char_table_set_extra_slot(&table, &LispObject::integer(0), upcase);
    table
}

fn make_table_for_purpose(purpose: &str, with_upcase_slot: bool) -> LispObject {
    if purpose == "category-table" {
        return make_category_table();
    }
    if with_upcase_slot {
        return make_case_table();
    }
    crate::primitives::core::make_char_table(LispObject::symbol(purpose), LispObject::nil())
}

fn current_table(
    state: Option<&InterpreterState>,
    storage_symbol: &str,
    purpose: &str,
    with_upcase_slot: bool,
) -> LispObject {
    let Some(state) = state else {
        return make_table_for_purpose(purpose, with_upcase_slot);
    };
    let id = crate::obarray::intern(storage_symbol);
    if let Some(table) = state.get_value_cell(id) {
        return table;
    }
    let table = make_table_for_purpose(purpose, with_upcase_slot);
    state.set_value_cell(id, table.clone());
    table
}

fn prim_set_named_table(
    args: &LispObject,
    state: Option<&InterpreterState>,
    storage_symbol: &str,
) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    if let Some(state) = state {
        state.set_value_cell(crate::obarray::intern(storage_symbol), table.clone());
    }
    Ok(table)
}

fn prim_copy_named_table(
    args: &LispObject,
    state: Option<&InterpreterState>,
    storage_symbol: &str,
    purpose: &str,
    with_upcase_slot: bool,
) -> ElispResult<LispObject> {
    let table = args
        .first()
        .filter(|value| !value.is_nil())
        .unwrap_or_else(|| current_table(state, storage_symbol, purpose, with_upcase_slot));
    Ok(
        copy_char_table(&table)
            .unwrap_or_else(|| make_table_for_purpose(purpose, with_upcase_slot)),
    )
}

fn prim_make_or_copy_table(args: &LispObject, purpose: &str) -> ElispResult<LispObject> {
    if let Some(table) = args.first()
        && !table.is_nil()
    {
        return Ok(
            copy_char_table(&table).unwrap_or_else(|| make_table_for_purpose(purpose, false))
        );
    }
    Ok(make_table_for_purpose(purpose, false))
}

fn prim_table_p(args: &LispObject, purpose: &str) -> ElispResult<LispObject> {
    let table = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(table_purpose_is(&table, purpose)))
}

fn table_purpose_is(table: &LispObject, purpose: &str) -> bool {
    crate::primitives::core::is_char_table(table)
        && crate::primitives::core::char_table_subtype(table)
            .as_symbol()
            .as_deref()
            == Some(purpose)
}

fn copy_char_table(table: &LispObject) -> Option<LispObject> {
    if !crate::primitives::core::is_char_table(table) {
        return None;
    }
    let LispObject::HashTable(hash) = table else {
        return None;
    };
    Some(LispObject::HashTable(Arc::new(
        crate::eval::SyncRefCell::new(hash.lock().clone()),
    )))
}

fn category_table_arg(
    args: &LispObject,
    idx: usize,
    state: Option<&InterpreterState>,
) -> LispObject {
    args.nth(idx)
        .filter(|value| !value.is_nil())
        .unwrap_or_else(|| current_table(state, STANDARD_CATEGORY_TABLE, "category-table", false))
}

fn syntax_table_arg(args: &LispObject, idx: usize, state: Option<&InterpreterState>) -> LispObject {
    args.nth(idx)
        .filter(|value| !value.is_nil())
        .unwrap_or_else(|| current_table(state, STANDARD_SYNTAX_TABLE, "syntax-table", false))
}

fn case_table_arg(args: &LispObject, idx: usize, state: Option<&InterpreterState>) -> LispObject {
    args.nth(idx)
        .filter(|value| !value.is_nil())
        .unwrap_or_else(|| current_table(state, STANDARD_CASE_TABLE, "case-table", true))
}

fn char_arg(obj: &LispObject) -> ElispResult<char> {
    if let Some(code) = obj.as_integer() {
        let code = u32::try_from(code)
            .ok()
            .and_then(char::from_u32)
            .ok_or_else(|| ElispError::WrongTypeArgument("character".to_string()))?;
        return Ok(code);
    }
    if let Some(s) = obj.as_string()
        && s.chars().count() == 1
    {
        return Ok(s.chars().next().unwrap_or('\0'));
    }
    Err(ElispError::WrongTypeArgument("character".to_string()))
}

fn char_code(ch: char) -> LispObject {
    LispObject::integer(i64::from(u32::from(ch)))
}

fn category_char(obj: &LispObject) -> ElispResult<char> {
    char_arg(obj)
}

fn category_string(obj: &LispObject) -> ElispResult<String> {
    if obj.is_nil() {
        return Ok(String::new());
    }
    if let Some(s) = obj.as_string() {
        return Ok(canonical_category_set(s));
    }
    Ok(category_char(obj)?.to_string())
}

fn canonical_category_set(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if !out.contains(ch) {
            out.push(ch);
        }
    }
    out
}

fn add_category(existing: &str, category: char) -> String {
    if existing.contains(category) {
        existing.to_string()
    } else {
        let mut out = existing.to_string();
        out.push(category);
        out
    }
}

fn remove_category(existing: &str, category: char) -> String {
    existing.chars().filter(|&ch| ch != category).collect()
}

fn prim_define_category(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let category = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let docstring = args.nth(1).unwrap_or_else(LispObject::nil);
    let table = category_table_arg(args, 2, state);
    let slot = char_code(category_char(&category)?);
    crate::primitives::core::char_table_set_extra_slot(&table, &slot, docstring)?;
    Ok(LispObject::nil())
}

fn prim_category_docstring(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let category = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let table = category_table_arg(args, 1, state);
    let slot = char_code(category_char(&category)?);
    crate::primitives::core::char_table_extra_slot(&table, &slot)
}

fn prim_category_set_mnemonics(args: &LispObject) -> ElispResult<LispObject> {
    let set = args.first().unwrap_or_else(LispObject::nil);
    Ok(LispObject::string(&category_string(&set)?))
}

fn prim_make_category_set(args: &LispObject) -> ElispResult<LispObject> {
    let categories = args.first().unwrap_or_else(LispObject::nil);
    Ok(LispObject::string(&category_string(&categories)?))
}

fn prim_char_category_set(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let ch = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let table = category_table_arg(args, 1, state);
    let set = crate::primitives::core::char_table_range(&table, &char_code(char_arg(&ch)?))?;
    Ok(LispObject::string(&category_string(&set)?))
}

fn prim_modify_category_entry(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let range = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let category = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let table = category_table_arg(args, 2, state);
    let reset = args.nth(3).is_some_and(|value| !value.is_nil());
    let category = category_char(&category)?;
    if let Some(ch) = range
        .as_integer()
        .and_then(|code| u32::try_from(code).ok())
        .and_then(char::from_u32)
    {
        let current = crate::primitives::core::char_table_range(&table, &char_code(ch))
            .unwrap_or_else(|_| LispObject::nil());
        let current = category_string(&current)?;
        let updated = if reset {
            remove_category(&current, category)
        } else {
            add_category(&current, category)
        };
        crate::primitives::core::char_table_set_range(
            &table,
            &char_code(ch),
            LispObject::string(&updated),
        )?;
    } else {
        let value = if reset {
            LispObject::string("")
        } else {
            LispObject::string(&category.to_string())
        };
        crate::primitives::core::char_table_set_range(&table, &range, value)?;
    }
    Ok(LispObject::nil())
}

fn syntax_class(ch: char) -> char {
    if ch.is_alphanumeric() {
        'w'
    } else if ch == '_' {
        '_'
    } else if ch.is_whitespace() {
        ' '
    } else if "([{".contains(ch) {
        '('
    } else if ")]}".contains(ch) {
        ')'
    } else if ch == '"' {
        '"'
    } else if ch == '\'' {
        '\''
    } else if ch == '\\' {
        '\\'
    } else {
        '.'
    }
}

const SYNTAX_CLASS_CHARS: &str = " .w_()'\"$\\/<>@!|";

fn syntax_class_to_char(class: i64) -> Option<char> {
    if !(0..16).contains(&class) {
        return None;
    }
    SYNTAX_CLASS_CHARS.chars().nth(class as usize)
}

fn syntax_code_to_char(obj: &LispObject) -> Option<char> {
    if let Some(n) = obj.as_integer() {
        return syntax_class_to_char(n);
    }
    if let Some((car, _)) = obj.destructure_cons()
        && let Some(n) = car.as_integer()
    {
        return syntax_class_to_char(n);
    }
    obj.as_string().and_then(|s| s.chars().next())
}

fn syntax_entry(table: &LispObject, ch: char) -> Option<char> {
    let value = crate::primitives::core::char_table_range(table, &char_code(ch)).ok()?;
    if value.is_nil() {
        None
    } else {
        syntax_code_to_char(&value)
    }
}

fn prim_modify_syntax_entry(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let range = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let syntax = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let table = syntax_table_arg(args, 2, state);
    let value = string_to_syntax_value(&syntax)?;
    crate::primitives::core::char_table_set_range(&table, &range, value)?;
    Ok(LispObject::nil())
}

fn prim_char_syntax(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let ch = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let ch = char_arg(&ch)?;
    let table = syntax_table_arg(args, 1, state);
    let class = syntax_entry(&table, ch).unwrap_or_else(|| syntax_class(ch));
    Ok(char_code(class))
}

fn prim_syntax_class_to_char(args: &LispObject) -> ElispResult<LispObject> {
    let class = args
        .first()
        .and_then(|obj| obj.as_integer())
        .ok_or_else(|| ElispError::WrongTypeArgument("integer".to_string()))?;
    let ch = syntax_class_to_char(class).ok_or_else(|| {
        ElispError::WrongTypeArgument("syntax-class in 0..=15".to_string())
    })?;
    Ok(char_code(ch))
}

fn prim_syntax_after(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|obj| obj.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(pos) = usize::try_from(pos).ok() else {
        return Ok(LispObject::nil());
    };
    let table = syntax_table_arg(args, 1, state);
    let ch = buffer::with_current(|b| b.char_at(pos));
    Ok(ch
        .map(|ch| char_code(syntax_entry(&table, ch).unwrap_or_else(|| syntax_class(ch))))
        .unwrap_or_else(LispObject::nil))
}

fn string_to_syntax_value(obj: &LispObject) -> ElispResult<LispObject> {
    let s = obj
        .as_string()
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    let Some(first) = s.chars().next() else {
        return Ok(LispObject::nil());
    };
    let Some(class) = SYNTAX_CLASS_CHARS.chars().position(|c| c == first) else {
        return Ok(LispObject::nil());
    };
    // Real Emacs returns (CLASS . FLAGS); we don't model flags, so just
    // emit (CLASS . nil). The car still satisfies callers that read the
    // syntax class out of the descriptor.
    Ok(LispObject::cons(
        LispObject::integer(class as i64),
        LispObject::nil(),
    ))
}

fn prim_string_to_syntax(args: &LispObject) -> ElispResult<LispObject> {
    let syntax = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    string_to_syntax_value(&syntax)
}

fn prim_set_case_syntax_pair(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let upper = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let lower = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let table = case_table_arg(args, 2, state);
    let upper = char_arg(&upper)?;
    let lower = char_arg(&lower)?;
    crate::primitives::core::char_table_set_range(&table, &char_code(upper), char_code(lower))?;
    let upcase = crate::primitives::core::char_table_extra_slot(&table, &LispObject::integer(0))?;
    if crate::primitives::core::is_char_table(&upcase) {
        crate::primitives::core::char_table_set_range(
            &upcase,
            &char_code(lower),
            char_code(upper),
        )?;
    }
    Ok(LispObject::nil())
}

fn charset_name(obj: &LispObject) -> Option<String> {
    obj.as_symbol()
        .or_else(|| obj.as_string().map(ToString::to_string))
        .map(|name| {
            name.strip_suffix("-unix")
                .or_else(|| name.strip_suffix("-dos"))
                .or_else(|| name.strip_suffix("-mac"))
                .unwrap_or(&name)
                .to_string()
        })
}

fn charset_defined_symbol(name: &str) -> String {
    format!("rele--charset-defined--{name}")
}

fn charset_plist_symbol(name: &str) -> String {
    format!("rele--charset-plist--{name}")
}

fn known_charset(name: &str, state: Option<&InterpreterState>) -> bool {
    BUILTIN_CHARSETS.contains(&name)
        || state.is_some_and(|state| {
            state
                .get_value_cell(crate::obarray::intern(&charset_defined_symbol(name)))
                .is_some_and(|value| !value.is_nil())
        })
}

fn default_charset_priority_list() -> LispObject {
    list_symbols(BUILTIN_CHARSETS)
}

fn charset_priority_list(state: Option<&InterpreterState>) -> LispObject {
    state
        .and_then(|state| state.get_value_cell(crate::obarray::intern(CHARSET_PRIORITY_LIST)))
        .unwrap_or_else(default_charset_priority_list)
}

fn prim_charset_priority_list(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let priority = charset_priority_list(state);
    if args.first().is_some_and(|arg| !arg.is_nil()) {
        Ok(priority.first().unwrap_or_else(LispObject::nil))
    } else {
        Ok(priority)
    }
}

fn prim_set_charset_priority(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    if let Some(state) = state {
        let priority = if args.is_nil() {
            default_charset_priority_list()
        } else {
            merged_charset_priority(args.clone())
        };
        state.set_value_cell(crate::obarray::intern(CHARSET_PRIORITY_LIST), priority);
    }
    Ok(LispObject::nil())
}

fn merged_charset_priority(priority: LispObject) -> LispObject {
    let mut items = list_to_vec(priority);
    for name in BUILTIN_CHARSETS {
        let charset = LispObject::symbol(name);
        if !items.iter().any(|item| item == &charset) {
            items.push(charset);
        }
    }
    list_from_vec(items)
}

fn prim_charsetp(args: &LispObject, state: Option<&InterpreterState>) -> ElispResult<LispObject> {
    let charset = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::from(
        charset_name(&charset).is_some_and(|name| known_charset(&name, state)),
    ))
}

fn charset_for_char(ch: char) -> LispObject {
    if ch.is_ascii() {
        LispObject::symbol("ascii")
    } else {
        LispObject::symbol("unicode")
    }
}

fn prim_char_charset(args: &LispObject) -> ElispResult<LispObject> {
    let ch = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(charset_for_char(char_arg(&ch)?))
}

fn prim_charset_after(args: &LispObject) -> ElispResult<LispObject> {
    let pos = args
        .first()
        .and_then(|obj| obj.as_integer())
        .unwrap_or_else(|| buffer::with_current(|b| i64::try_from(b.point).unwrap_or(i64::MAX)));
    let Some(pos) = usize::try_from(pos).ok() else {
        return Ok(LispObject::nil());
    };
    Ok(buffer::with_current(|b| b.char_at(pos))
        .map(charset_for_char)
        .unwrap_or_else(LispObject::nil))
}

fn prim_charset_plist(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let charset = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(name) = charset_name(&charset) else {
        return Ok(LispObject::nil());
    };
    Ok(state
        .and_then(|state| {
            state.get_value_cell(crate::obarray::intern(&charset_plist_symbol(&name)))
        })
        .unwrap_or_else(LispObject::nil))
}

fn prim_set_charset_plist(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let charset = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let plist = args.nth(1).unwrap_or_else(LispObject::nil);
    if let (Some(state), Some(name)) = (state, charset_name(&charset)) {
        state.set_value_cell(
            crate::obarray::intern(&charset_plist_symbol(&name)),
            plist.clone(),
        );
    }
    Ok(plist)
}

fn prim_charset_id(args: &LispObject) -> ElispResult<LispObject> {
    let charset = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(name) = charset_name(&charset) else {
        return Ok(LispObject::nil());
    };
    let id = match name.as_str() {
        "ascii" | "us-ascii" => 0,
        "unicode" | "utf-8" => 1,
        "iso-8859-1" | "latin-1" => 2,
        "eight-bit" | "binary" => 3,
        "emacs-mule" => 4,
        _ => return Ok(LispObject::nil()),
    };
    Ok(LispObject::integer(id))
}

fn prim_charset_dimension(args: &LispObject) -> ElispResult<LispObject> {
    let charset = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    Ok(LispObject::integer(if charset_name(&charset).is_some() {
        1
    } else {
        0
    }))
}

fn prim_define_charset(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let name = args
        .first()
        .unwrap_or_else(|| LispObject::symbol("unicode"));
    if let (Some(state), Some(name_str)) = (state, charset_name(&name)) {
        state.set_value_cell(
            crate::obarray::intern(&charset_defined_symbol(&name_str)),
            LispObject::t(),
        );
    }
    Ok(name)
}

fn prim_get_charset_property(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let charset = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let property = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let plist = prim_charset_plist(&LispObject::cons(charset, LispObject::nil()), state)?;
    Ok(plist_get(&plist, &property).unwrap_or_else(LispObject::nil))
}

fn prim_put_charset_property(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let charset = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let property = args.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let value = args.nth(2).unwrap_or_else(LispObject::nil);
    if let Some(state) = state {
        let plist = prim_charset_plist(
            &LispObject::cons(charset.clone(), LispObject::nil()),
            Some(state),
        )?;
        let updated = plist_put(plist, property, value.clone());
        let set_args = LispObject::cons(charset, LispObject::cons(updated, LispObject::nil()));
        let _ = prim_set_charset_plist(&set_args, Some(state))?;
    }
    Ok(value)
}

fn prim_find_charset_string(args: &LispObject) -> ElispResult<LispObject> {
    let string = args
        .first()
        .and_then(|obj| obj.as_string().cloned())
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    Ok(charsets_for_chars(string.chars()))
}

fn prim_find_charset_region(args: &LispObject) -> ElispResult<LispObject> {
    let start = args
        .first()
        .and_then(|obj| obj.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    let end = args
        .nth(1)
        .and_then(|obj| obj.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    let Some(start) = usize::try_from(start).ok() else {
        return Ok(LispObject::nil());
    };
    let Some(end) = usize::try_from(end).ok() else {
        return Ok(LispObject::nil());
    };
    let chars = buffer::with_current(|b| {
        (start..end)
            .filter_map(|pos| b.char_at(pos))
            .collect::<Vec<_>>()
    });
    Ok(charsets_for_chars(chars))
}

fn charsets_for_chars<I>(chars: I) -> LispObject
where
    I: IntoIterator<Item = char>,
{
    let mut has_ascii = false;
    let mut has_unicode = false;
    for ch in chars {
        if ch.is_ascii() {
            has_ascii = true;
        } else {
            has_unicode = true;
        }
    }

    let mut names = Vec::new();
    if has_ascii {
        names.push("ascii");
    }
    if has_unicode {
        names.push("unicode");
    }
    list_symbols(&names)
}

fn prim_iso_charset(args: &LispObject) -> ElispResult<LispObject> {
    let dimension = args
        .first()
        .and_then(|arg| arg.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    let chars = args
        .nth(1)
        .and_then(|arg| arg.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    let final_char = args
        .nth(2)
        .and_then(|arg| arg.as_integer())
        .ok_or(ElispError::WrongNumberOfArguments)?;
    if dimension == 1 && chars == 94 && final_char == i64::from(b'B') {
        Ok(LispObject::symbol("ascii"))
    } else {
        Ok(LispObject::nil())
    }
}

fn prim_sort_charsets(
    args: &LispObject,
    state: Option<&InterpreterState>,
) -> ElispResult<LispObject> {
    let charsets = args.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let priority = list_to_vec(charset_priority_list(state));
    let mut items = list_to_vec(charsets);
    items.sort_by_key(|item| {
        priority
            .iter()
            .position(|priority_item| priority_item == item)
            .unwrap_or(usize::MAX)
    });
    Ok(list_from_vec(items))
}

fn plist_get(plist: &LispObject, key: &LispObject) -> Option<LispObject> {
    let mut cur = plist.clone();
    while let Some((k, rest)) = cur.destructure_cons() {
        let Some((value, next)) = rest.destructure_cons() else {
            break;
        };
        if &k == key {
            return Some(value);
        }
        cur = next;
    }
    None
}

fn plist_put(plist: LispObject, key: LispObject, value: LispObject) -> LispObject {
    let mut items = Vec::new();
    let mut replaced = false;
    let mut cur = plist;
    while let Some((k, rest)) = cur.destructure_cons() {
        let Some((old_value, next)) = rest.destructure_cons() else {
            break;
        };
        if k == key {
            items.push(k);
            items.push(value.clone());
            replaced = true;
        } else {
            items.push(k);
            items.push(old_value);
        }
        cur = next;
    }
    if !replaced {
        items.push(key);
        items.push(value);
    }
    list_from_vec(items)
}

fn list_symbols(names: &[&str]) -> LispObject {
    names.iter().rev().fold(LispObject::nil(), |tail, name| {
        LispObject::cons(LispObject::symbol(name), tail)
    })
}

fn list_to_vec(list: LispObject) -> Vec<LispObject> {
    let mut items = Vec::new();
    let mut cur = list;
    while let Some((item, rest)) = cur.destructure_cons() {
        items.push(item);
        cur = rest;
    }
    items
}

fn list_from_vec(items: Vec<LispObject>) -> LispObject {
    items
        .into_iter()
        .rev()
        .fold(LispObject::nil(), |tail, item| LispObject::cons(item, tail))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(items: Vec<LispObject>) -> LispObject {
        list_from_vec(items)
    }

    #[test]
    fn charset_priority_highest_tracks_set_charset_priority() {
        let interp = crate::Interpreter::new();
        let args = list(vec![LispObject::symbol("unicode")]);

        prim_set_charset_priority(&args, Some(&interp.state)).unwrap();

        assert_eq!(
            prim_charset_priority_list(&list(vec![LispObject::t()]), Some(&interp.state)).unwrap(),
            LispObject::symbol("unicode"),
        );
        assert_eq!(
            prim_charset_priority_list(&LispObject::nil(), Some(&interp.state))
                .unwrap()
                .first(),
            Some(LispObject::symbol("unicode")),
        );
    }

    #[test]
    fn iso_charset_reports_ascii_designation() {
        assert_eq!(
            prim_iso_charset(&list(vec![
                LispObject::integer(1),
                LispObject::integer(94),
                LispObject::integer(i64::from(b'B')),
            ]))
            .unwrap(),
            LispObject::symbol("ascii"),
        );
    }

    #[test]
    fn sort_charsets_uses_priority_order() {
        let input = list(vec![
            LispObject::symbol("unicode"),
            LispObject::symbol("ascii"),
        ]);
        let result = prim_sort_charsets(&list(vec![input]), None).unwrap();
        assert_eq!(
            result,
            list(vec![
                LispObject::symbol("ascii"),
                LispObject::symbol("unicode"),
            ]),
        );
    }
}
