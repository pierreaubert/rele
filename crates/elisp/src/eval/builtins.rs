// Builtin functions: put, get, provide, featurep, require, mapcar, mapc, dolist, format.

use super::SyncRefCell as RwLock;
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::sync::Arc;

use super::functions::call_function;
use super::{Environment, InterpreterState, MacroTable, eval, eval_progn};

pub(super) fn eval_put(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let sym = value_to_obj(eval(
        obj_to_value(args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);
    let prop = value_to_obj(eval(
        obj_to_value(args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);
    let val = value_to_obj(eval(
        obj_to_value(args_obj.nth(2).ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);

    // put requires symbols for both SYM and PROP. If either is
    // non-symbol, return val silently (Emacs would error, but many
    // .elc files pass non-symbol values during bootstrap).
    let Some(sym_id) = plist_symbol_id(&sym) else {
        return Ok(obj_to_value(val));
    };
    let Some(prop_id) = plist_symbol_id(&prop) else {
        return Ok(obj_to_value(val));
    };
    state.put_plist(sym_id, prop_id, val.clone());
    Ok(obj_to_value(val))
}
pub(super) fn eval_get(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let sym = value_to_obj(eval(
        obj_to_value(args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);
    let prop = value_to_obj(eval(
        obj_to_value(args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);

    let Some(sym_id) = plist_symbol_id(&sym) else {
        return Ok(Value::nil());
    };
    let Some(prop_id) = plist_symbol_id(&prop) else {
        return Ok(Value::nil());
    };
    Ok(obj_to_value(state.get_plist(sym_id, prop_id)))
}

fn plist_symbol_id(obj: &LispObject) -> Option<crate::obarray::SymbolId> {
    match obj {
        LispObject::Symbol(id) => Some(*id),
        LispObject::T => Some(crate::obarray::intern("t")),
        LispObject::Nil => Some(crate::obarray::intern("nil")),
        _ => None,
    }
}
pub(super) fn eval_provide(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let feature = value_to_obj(eval(
        obj_to_value(args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);
    let name = feature
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let mut features = state.features.write();
    if !features.contains(&name) {
        features.push(name);
    }
    Ok(obj_to_value(feature))
}
pub(super) fn eval_featurep(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let feature = value_to_obj(eval(
        obj_to_value(args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);
    let name = feature
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let features = state.features.read();
    Ok(obj_to_value(LispObject::from(features.contains(&name))))
}
pub(super) fn eval_require(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let feature = value_to_obj(eval(
        obj_to_value(args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?),
        env,
        editor,
        macros,
        state,
    )?);
    let name = feature
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;

    // Already provided — nothing to do
    if state.features.read().contains(&name) {
        return Ok(obj_to_value(feature));
    }

    // Determine the file to load: use explicit filename arg if given, else the feature name
    let file = args_obj
        .nth(1)
        .and_then(|f| {
            let val = value_to_obj(eval(obj_to_value(f), env, editor, macros, state).ok()?);
            val.as_string().map(|s| s.to_string())
        })
        .unwrap_or_else(|| name.clone());

    // Build (load FILE nil) — noerror=nil so missing files signal
    let load_args = LispObject::cons(
        LispObject::string(&file),
        LispObject::cons(LispObject::nil(), LispObject::nil()),
    );
    eval_load(obj_to_value(load_args), env, editor, macros, state)?;

    // Return the feature symbol regardless of whether `provide` was called.
    // Some files don't call provide — that's OK for our purposes.
    Ok(obj_to_value(feature))
}
pub(super) fn eval_mapcar(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let func_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list_expr = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let func = value_to_obj(eval(obj_to_value(func_expr), env, editor, macros, state)?);
    let list = value_to_obj(eval(obj_to_value(list_expr), env, editor, macros, state)?);

    let mut results = Vec::new();
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        let call_args = LispObject::cons(car, LispObject::nil());
        let result = call_function(
            obj_to_value(func.clone()),
            obj_to_value(call_args),
            env,
            editor,
            macros,
            state,
        )?;
        results.push(value_to_obj(result));
        current = cdr;
    }
    let mut result = LispObject::nil();
    for r in results.into_iter().rev() {
        result = LispObject::cons(r, result);
    }
    Ok(obj_to_value(result))
}
pub(super) fn eval_mapc(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let func_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let list_expr = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let func = value_to_obj(eval(obj_to_value(func_expr), env, editor, macros, state)?);
    let list = value_to_obj(eval(obj_to_value(list_expr), env, editor, macros, state)?);

    let mut current = list.clone();
    while let Some((car, cdr)) = current.destructure_cons() {
        let call_args = LispObject::cons(car, LispObject::nil());
        call_function(
            obj_to_value(func.clone()),
            obj_to_value(call_args),
            env,
            editor,
            macros,
            state,
        )?;
        current = cdr;
    }
    Ok(obj_to_value(list))
}
pub(super) fn eval_dolist(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let spec = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().unwrap_or(LispObject::nil());

    let var = spec.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let var_name = var
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let list_expr = spec.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let result_expr = spec.nth(2);

    let list = value_to_obj(eval(obj_to_value(list_expr), env, editor, macros, state)?);

    let body_val = obj_to_value(body);
    let var_id = crate::obarray::intern(&var_name);
    let saved_loop_binding = env.read().get_id_local(var_id);
    let mut current = list;
    while let Some((car, cdr)) = current.destructure_cons() {
        env.write().set_id(var_id, car);
        eval_progn(body_val, env, editor, macros, state)?;
        current = cdr;
    }

    env.write().set_id(var_id, LispObject::nil());
    if let Some(result_expr) = result_expr {
        let result = eval(obj_to_value(result_expr), env, editor, macros, state);
        match saved_loop_binding {
            Some(value) => env.write().set_id(var_id, value),
            None => env.write().unset_id(var_id),
        }
        result
    } else {
        match saved_loop_binding {
            Some(value) => env.write().set_id(var_id, value),
            None => env.write().unset_id(var_id),
        }
        Ok(Value::nil())
    }
}

pub(super) fn eval_dotimes(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let spec = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().unwrap_or(LispObject::nil());

    let var = spec.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let var_name = var
        .as_symbol()
        .ok_or_else(|| ElispError::WrongTypeArgument("symbol".to_string()))?;
    let count_expr = spec.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let result_expr = spec.nth(2);
    let count = value_to_obj(eval(obj_to_value(count_expr), env, editor, macros, state)?)
        .as_integer()
        .unwrap_or(0)
        .max(0);

    let body_val = obj_to_value(body);
    let var_id = crate::obarray::intern(&var_name);
    let saved_loop_binding = env.read().get_id_local(var_id);
    for i in 0..count {
        env.write().set_id(var_id, LispObject::integer(i));
        eval_progn(body_val, env, editor, macros, state)?;
    }

    env.write().set_id(var_id, LispObject::nil());
    if let Some(result_expr) = result_expr {
        let result = eval(obj_to_value(result_expr), env, editor, macros, state);
        match saved_loop_binding {
            Some(value) => env.write().set_id(var_id, value),
            None => env.write().unset_id(var_id),
        }
        result
    } else {
        match saved_loop_binding {
            Some(value) => env.write().set_id(var_id, value),
            None => env.write().unset_id(var_id),
        }
        Ok(Value::nil())
    }
}
pub(super) fn emacs_regex_to_rust(emacs: &str) -> String {
    let mut result = String::new();
    let chars: Vec<char> = emacs.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            match chars[i + 1] {
                '(' => {
                    result.push('(');
                    i += 2;
                }
                ')' => {
                    result.push(')');
                    i += 2;
                }
                '|' => {
                    result.push('|');
                    i += 2;
                }
                '{' => {
                    result.push('{');
                    i += 2;
                }
                '}' => {
                    result.push('}');
                    i += 2;
                }
                'w' => {
                    result.push_str("[[:alnum:]_]");
                    i += 2;
                }
                'b' => {
                    result.push_str("\\b");
                    i += 2;
                }
                's' => {
                    if i + 2 < chars.len() && chars[i + 2] == '-' {
                        result.push_str("\\s");
                        i += 3;
                    } else {
                        result.push_str("\\s");
                        i += 2;
                    }
                }
                '`' => {
                    result.push_str("\\A");
                    i += 2;
                }
                '\'' => {
                    result.push_str("\\z");
                    i += 2;
                }
                c => {
                    result.push('\\');
                    result.push(c);
                    i += 2;
                }
            }
        } else {
            match chars[i] {
                '(' => result.push_str("\\("),
                ')' => result.push_str("\\)"),
                '|' => result.push_str("\\|"),
                c => result.push(c),
            }
            i += 1;
        }
    }
    result
}
pub(super) fn eval_format(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let fmt_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let fmt = value_to_obj(eval(obj_to_value(fmt_expr), env, editor, macros, state)?);
    let fmt_str = match fmt {
        LispObject::String(s) => s,
        LispObject::Nil => return Ok(obj_to_value(LispObject::string("nil"))),
        _ => return Err(ElispError::WrongTypeArgument("string".to_string())),
    };

    let mut format_args = Vec::new();
    let mut rest = args_obj.rest().unwrap_or(LispObject::nil());
    while let Some((arg, next)) = rest.destructure_cons() {
        let val = value_to_obj(eval(obj_to_value(arg), env, editor, macros, state)?);
        format_args.push(val);
        rest = next;
    }

    let mut result = String::new();
    let mut arg_idx = 0;
    // Cap format-string length: even with bounded inner loops, an
    // adversarial format string can be very expensive to walk.
    const MAX_FMT_CHARS: usize = 64 * 1024;
    let chars: Vec<char> = fmt_str.chars().take(MAX_FMT_CHARS).collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '%' && i + 1 < chars.len() {
            i += 1;
            let mut left_align = false;
            let mut zero_pad = false;
            while i < chars.len() && (chars[i] == '-' || chars[i] == '+' || chars[i] == '0') {
                match chars[i] {
                    '-' => left_align = true,
                    '0' => zero_pad = true,
                    _ => {}
                }
                i += 1;
            }
            let mut width: usize = 0;
            while i < chars.len() && chars[i].is_ascii_digit() {
                width = width * 10 + (chars[i] as usize - '0' as usize);
                i += 1;
            }
            if i >= chars.len() {
                break;
            }
            if left_align {
                zero_pad = false;
            }
            let apply_width = |s: String| -> String {
                if width == 0 || s.len() >= width {
                    s
                } else if left_align {
                    format!("{:<width$}", s, width = width)
                } else if zero_pad {
                    if let Some(stripped) = s.strip_prefix('-') {
                        format!("-{:0>width$}", stripped, width = width - 1)
                    } else {
                        format!("{:0>width$}", s, width = width)
                    }
                } else {
                    format!("{:>width$}", s, width = width)
                }
            };
            match chars[i] {
                's' => {
                    if arg_idx < format_args.len() {
                        let s = format_args[arg_idx].princ_to_string();
                        result.push_str(&apply_width(s));
                        arg_idx += 1;
                    }
                }
                'S' => {
                    if arg_idx < format_args.len() {
                        let s = format_args[arg_idx].prin1_to_string();
                        result.push_str(&apply_width(s));
                        arg_idx += 1;
                    }
                }
                'd' => {
                    if arg_idx < format_args.len() {
                        let s = match &format_args[arg_idx] {
                            LispObject::Integer(n) => n.to_string(),
                            LispObject::Float(f) => (*f as i64).to_string(),
                            _ => format_args[arg_idx].princ_to_string(),
                        };
                        result.push_str(&apply_width(s));
                        arg_idx += 1;
                    }
                }
                'f' => {
                    if arg_idx < format_args.len() {
                        let s = match &format_args[arg_idx] {
                            LispObject::Float(f) => format!("{:.6}", f),
                            LispObject::Integer(n) => format!("{:.6}", *n as f64),
                            _ => format_args[arg_idx].princ_to_string(),
                        };
                        result.push_str(&apply_width(s));
                        arg_idx += 1;
                    }
                }
                'c' => {
                    if arg_idx < format_args.len() {
                        if let LispObject::Integer(n) = &format_args[arg_idx] {
                            if let Some(ch) = char::from_u32(*n as u32) {
                                let s = ch.to_string();
                                result.push_str(&apply_width(s));
                            }
                        }
                        arg_idx += 1;
                    }
                }
                'x' => {
                    if arg_idx < format_args.len() {
                        if let LispObject::Integer(n) = &format_args[arg_idx] {
                            let s = format!("{:x}", n);
                            result.push_str(&apply_width(s));
                        }
                        arg_idx += 1;
                    }
                }
                'o' => {
                    if arg_idx < format_args.len() {
                        if let LispObject::Integer(n) = &format_args[arg_idx] {
                            let s = format!("{:o}", n);
                            result.push_str(&apply_width(s));
                        }
                        arg_idx += 1;
                    }
                }
                '%' => result.push('%'),
                _ => {
                    result.push('%');
                    result.push(chars[i]);
                }
            }
        } else {
            result.push(chars[i]);
        }
        i += 1;
    }
    Ok(obj_to_value(LispObject::string(&result)))
}

/// (load FILE &optional NOERROR NOMESSAGE NOSUFFIX MUST-SUFFIX)
/// Search `load-path` for FILE with .elc / .el suffixes, read and eval all forms.
fn after_load_key_matches(key: &LispObject, requested: &str, loaded_path: &str) -> bool {
    let key_name = key.as_string().cloned().or_else(|| key.as_symbol());
    let Some(key_name) = key_name else {
        return false;
    };
    let loaded = std::path::Path::new(loaded_path);
    let file_name = loaded.file_name().and_then(|s| s.to_str()).unwrap_or("");
    let stem = file_name
        .strip_suffix(".el.gz")
        .or_else(|| file_name.strip_suffix(".elc"))
        .or_else(|| file_name.strip_suffix(".el"))
        .unwrap_or(file_name);
    key_name == requested || key_name == loaded_path || key_name == file_name || key_name == stem
}

fn run_after_load_hooks(
    requested: &str,
    loaded_path: &str,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<()> {
    let sym = crate::obarray::intern("after-load-alist");
    let alist = state
        .global_env
        .read()
        .get_id(sym)
        .unwrap_or_else(LispObject::nil);
    let mut cur = alist;
    let mut remaining_rev = LispObject::nil();
    let mut forms_to_run = Vec::new();

    while let Some((entry, rest)) = cur.destructure_cons() {
        let (key, forms) = match entry.destructure_cons() {
            Some(pair) => pair,
            None => {
                remaining_rev = LispObject::cons(entry, remaining_rev);
                cur = rest;
                continue;
            }
        };
        if after_load_key_matches(&key, requested, loaded_path) {
            let mut form_list = forms;
            while let Some((form, next)) = form_list.destructure_cons() {
                forms_to_run.push(form);
                form_list = next;
            }
        } else {
            remaining_rev = LispObject::cons(LispObject::cons(key, forms), remaining_rev);
        }
        cur = rest;
    }

    let mut remaining = LispObject::nil();
    while let Some((entry, rest)) = remaining_rev.destructure_cons() {
        remaining = LispObject::cons(entry, remaining);
        remaining_rev = rest;
    }
    state.global_env.write().set_id(sym, remaining.clone());
    state.set_value_cell(sym, remaining);

    for form in forms_to_run {
        eval(obj_to_value(form), env, editor, macros, state)?;
    }
    Ok(())
}

pub(super) fn eval_load(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let file_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let file = value_to_obj(eval(obj_to_value(file_expr), env, editor, macros, state)?);
    let file_str = file
        .as_string()
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?
        .clone();

    // 4th arg (nosuffix): when non-nil, don't add .el / .elc suffixes
    let nosuffix = args_obj.nth(3).map(|v| !v.is_nil()).unwrap_or(false);

    // Prefer source only for generated/load-heavy features where the source
    // evaluator has narrow bootstrap shortcuts. Most stdlib requires still do
    // better through bytecode while the interpreter is incomplete.
    let prefer_source = matches!(
        file_str.as_str(),
        "cp51932" | "eucjp-ms" | "treesit" | "international/cp51932" | "international/eucjp-ms"
    );
    let suffixes: &[&str] = if nosuffix {
        &["", ".gz"]
    } else if prefer_source {
        &[".el", ".el.gz", ".elc", ""]
    } else {
        &[".elc", ".el", ".el.gz", ""]
    };

    // Gather load-path directories
    let load_path = state
        .global_env
        .read()
        .get("load-path")
        .unwrap_or(LispObject::nil());
    let mut load_dirs = Vec::new();
    let mut cur = load_path;
    while let Some((dir, rest)) = cur.destructure_cons() {
        if let Some(d) = dir.as_string() {
            load_dirs.push(d.clone());
        }
        cur = rest;
    }

    // Build candidate paths
    let mut paths_to_try = Vec::new();
    for suffix in suffixes {
        let full = format!("{}{}", file_str, suffix);
        // Try as absolute/relative path first
        paths_to_try.push(full.clone());
        // Then try in each load-path directory
        for d in &load_dirs {
            paths_to_try.push(format!("{}/{}", d, full));
        }
    }

    for path in &paths_to_try {
        let source = if path.ends_with(".gz") {
            // Decompress gzipped files via gunzip
            if std::path::Path::new(path).exists() {
                std::process::Command::new("gunzip")
                    .args(["-c", path])
                    .output()
                    .ok()
                    .and_then(|out| {
                        if out.status.success() {
                            String::from_utf8(out.stdout).ok()
                        } else {
                            None
                        }
                    })
            } else {
                None
            }
        } else if path.ends_with(".elc") {
            // .elc files are not necessarily valid UTF-8: read raw bytes and
            // map each byte to the corresponding Latin-1 char so that the
            // reader sees the exact byte values (shared-structure #N=/# refs,
            // bytecode strings, etc.) without UTF-8 reinterpretation.
            std::fs::read(path)
                .ok()
                .map(|bytes| bytes.iter().map(|&b| char::from(b)).collect())
        } else {
            std::fs::read_to_string(path).ok()
        };
        if let Some(source) = source {
            eprintln!("load: reading {path} ({} bytes)", source.len());
            let forms = crate::read_all(&source).map_err(|_| ElispError::FileError {
                operation: "load".into(),
                path: path.clone(),
                message: "read error".into(),
            })?;
            eprintln!("load: parsed {} forms from {path}", forms.len());
            let is_elc = path.ends_with(".elc");
            let forms_count = forms.len();
            let mut ok: usize = 0;
            // Phase 7: be tolerant of per-form errors during load.
            // Emacs's own behaviour is to propagate; we diverge because
            // our interpreter is incomplete (missing primitives, some
            // bytecode bugs) and most stdlib files are useful even when
            // a few forms fail. Errors are surfaced via stderr so
            // debugging still works.
            let load_start = std::time::Instant::now();
            let mut since_gc: usize = 0;
            for (i, form) in forms.into_iter().enumerate() {
                // Wall-clock safety: abort loading if a single file
                // takes too long. Increased from 3s to 30s to allow
                // legitimate require chains (cl-macs.el alone is 3500
                // lines of macro definitions).
                if load_start.elapsed().as_secs() >= 30 {
                    let ops = state.eval_ops.load(std::sync::atomic::Ordering::Relaxed);
                    let limit = state
                        .eval_ops_limit
                        .load(std::sync::atomic::Ordering::Relaxed);
                    eprintln!(
                        "load {path}: wall-clock timeout at form {i}/{forms_count} (ops={ops}, limit={limit})"
                    );
                    break;
                }
                // Periodic GC: the heap runs in Manual mode, so without
                // explicit collection large files (cl-macs.el, etc.) can
                // allocate hundreds of MB. Sweep every 200 forms.
                since_gc += 1;
                if since_gc >= 200 {
                    since_gc = 0;
                    let mut heap = state.heap.lock();
                    if heap.should_gc() {
                        heap.collect();
                    }
                }
                if let Err(e) = eval(obj_to_value(form), env, editor, macros, state) {
                    // Eval-ops-exceeded must propagate — never swallow it,
                    // or condition-case + eval_load creates an infinite loop.
                    if e.is_eval_ops_exceeded() {
                        return Err(e);
                    }
                    eprintln!("load {path}: form {i}: {e}");
                    // Early-abort for .elc: if the first 8 forms all
                    // failed, the bytecode VM almost certainly can't
                    // handle this file. Bail out fast instead of
                    // burning eval-ops on hundreds of doomed forms.
                    if is_elc && ok == 0 && i >= 7 {
                        eprintln!(
                            "load {path}: first {} bytecode forms failed, aborting early",
                            i + 1
                        );
                        break;
                    }
                } else {
                    ok += 1;
                }
            }
            run_after_load_hooks(&file_str, path, env, editor, macros, state)?;
            return Ok(Value::t());
        }
    }

    // 2nd arg (noerror): when non-nil, return nil instead of signaling
    let noerror = args_obj.nth(1).map(|v| !v.is_nil()).unwrap_or(false);
    if noerror {
        Ok(Value::nil())
    } else {
        Err(ElispError::FileError {
            operation: "load".into(),
            path: file_str,
            message: "file not found".into(),
        })
    }
}

/// Evaluate body forms in sequence, returning the result of the last one.
/// Used by save-excursion and save-restriction which wrap progn-style bodies.
pub(super) fn eval_progn_value(
    body: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    eval_progn(body, env, editor, macros, state)
}
