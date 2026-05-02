// Error and exception handling forms: catch, throw, condition-case, signal, unwind-protect.

use super::SyncRefCell as RwLock;
use crate::EditorCallbacks;
use crate::error::{ElispError, ElispResult, SignalData, ThrowData};
use crate::object::LispObject;
use crate::value::{Value, obj_to_value, value_to_obj};
use std::cell::{Cell, RefCell};
use std::sync::Arc;

use super::builtins::eval_format;
use super::functions::call_function;
use super::{Environment, InterpreterState, MacroTable, eval, eval_progn};

#[derive(Clone)]
struct HandlerBindEntry {
    conditions: LispObject,
    handler: LispObject,
}

#[derive(Clone)]
struct HandlerBindFrame {
    entries: Vec<HandlerBindEntry>,
    env: Arc<RwLock<Environment>>,
}

thread_local! {
    static HANDLER_BIND_STACK: RefCell<Vec<HandlerBindFrame>> = const { RefCell::new(Vec::new()) };
    static MUTE_FLOOR: Cell<usize> = const { Cell::new(usize::MAX) };
    /// Stack of `MUTE_FLOOR` values to restore. Pushed when a
    /// handler-bind handler exits via a non-local exit (so the mute
    /// stays in effect while the error propagates up the Rust stack);
    /// popped by the catching `condition-case`.
    static MUTE_RESTORE_STACK: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    static IN_DISPATCH: Cell<bool> = const { Cell::new(false) };
}

/// Walk the `handler-bind` stack top-down (skipping muted frames) and
/// invoke any matching handler within the dynamic extent of the signal
/// point. Returns `Ok(())` if no handler claimed the signal (caller
/// should propagate normally so `condition-case` can catch it), or
/// `Err(e)` if a handler did a non-local exit.
pub(super) fn dispatch_signal(
    err: &ElispError,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<()> {
    if IN_DISPATCH.with(Cell::get) {
        return Ok(());
    }
    let signal = err.to_signal();
    let err_value = if let ElispError::Signal(sig) = &signal {
        LispObject::cons(sig.symbol.clone(), sig.data.clone())
    } else {
        return Ok(());
    };
    let snapshot: Vec<(usize, HandlerBindFrame)> =
        HANDLER_BIND_STACK.with(|s| s.borrow().iter().cloned().enumerate().collect());
    let mute_floor = MUTE_FLOOR.with(Cell::get);
    'frames: for (idx, frame) in snapshot.iter().rev() {
        if *idx >= mute_floor {
            continue;
        }
        for entry in &frame.entries {
            if !condition_matches(err, &entry.conditions, state) {
                continue;
            }
            let prev_floor = mute_floor;
            MUTE_FLOOR.with(|m| m.set(*idx));
            let args = LispObject::cons(err_value.clone(), LispObject::nil());
            let res = call_function(
                obj_to_value(entry.handler.clone()),
                obj_to_value(args),
                &frame.env,
                editor,
                macros,
                state,
            );
            match res {
                Ok(_) => {
                    MUTE_FLOOR.with(|m| m.set(prev_floor));
                    continue 'frames;
                }
                Err(e) => {
                    MUTE_RESTORE_STACK.with(|s| s.borrow_mut().push(prev_floor));
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

fn condition_case_is_muted(cc_frame_idx: usize) -> bool {
    cc_frame_idx > MUTE_FLOOR.with(Cell::get)
}

fn release_mute_on_catch() {
    let saved = MUTE_RESTORE_STACK.with(|s| s.borrow_mut().pop());
    if let Some(prev) = saved {
        MUTE_FLOOR.with(|m| m.set(prev));
    }
}

pub(super) fn eval_catch(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let tag_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let body = args_obj.rest().unwrap_or(LispObject::nil());

    let tag = value_to_obj(eval(obj_to_value(tag_expr), env, editor, macros, state)?);

    match eval_progn(obj_to_value(body), env, editor, macros, state) {
        Ok(value) => Ok(value),
        Err(ElispError::Throw(throw_data)) => {
            if tag == throw_data.tag {
                // A throw can be the non-local exit from a
                // `handler-bind` handler — release the mute that was
                // installed while the handler ran.
                release_mute_on_catch();
                Ok(obj_to_value(throw_data.value))
            } else {
                Err(ElispError::Throw(throw_data))
            }
        }
        Err(e) => Err(e),
    }
}
pub(super) fn eval_throw(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let tag_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let value_expr = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let tag = value_to_obj(eval(obj_to_value(tag_expr), env, editor, macros, state)?);
    let value = value_to_obj(eval(obj_to_value(value_expr), env, editor, macros, state)?);

    Err(ElispError::Throw(Box::new(ThrowData { tag, value })))
}
/// Match an error against a condition-case clause condition.
/// Walks the `error-conditions` plist to honour `define-error` parents.
/// Accepts a single symbol or a list of symbols.
fn condition_matches(
    err: &ElispError,
    condition: &LispObject,
    state: &InterpreterState,
) -> bool {
    // Cheap path: direct symbol equality (covers built-in error types).
    if err.matches_condition(condition) {
        return true;
    }
    // Get the symbol of the actual error.
    let signal = err.to_signal();
    let err_sym = match &signal {
        ElispError::Signal(sig) => match sig.symbol.as_symbol() {
            Some(s) => s,
            None => return false,
        },
        _ => return false,
    };
    let err_id = crate::obarray::intern(&err_sym);
    let conds_id = crate::obarray::intern("error-conditions");
    let conditions = state.get_plist(err_id, conds_id);
    // condition can be a single symbol or a list of symbols.
    let mut targets = Vec::new();
    if condition.as_symbol().is_some() {
        targets.push(condition.clone());
    } else if matches!(condition, LispObject::Cons(_)) {
        let mut cur = condition.clone();
        while let Some((c, rest)) = cur.destructure_cons() {
            targets.push(c);
            cur = rest;
        }
    }
    let mut cur = conditions;
    while let Some((parent, rest)) = cur.destructure_cons() {
        for t in &targets {
            if &parent == t {
                return true;
            }
            // 'error matches any error class.
            if t.as_symbol().as_deref() == Some("error") {
                return true;
            }
        }
        cur = rest;
    }
    false
}

pub(super) fn eval_condition_case(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let var = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let bodyform = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let rest = args_obj
        .rest()
        .and_then(|r| r.rest())
        .unwrap_or(LispObject::nil());

    let cc_frame_idx = HANDLER_BIND_STACK.with(|s| s.borrow().len());

    match eval(obj_to_value(bodyform), env, editor, macros, state) {
        Ok(value) => Ok(value),
        Err(ElispError::TailEval(v)) => Err(ElispError::TailEval(v)),
        Err(ref err @ ElispError::Throw(..)) => Err(err.clone()),
        Err(ref err @ ElispError::StackOverflow) => Err(err.clone()),
        Err(ref err) if err.is_eval_ops_exceeded() => Err(err.clone()),
        Err(err) => {
            // A condition-case installed *inside* an active
            // handler-bind dispatch is muted: the handler-bind's
            // signal is not allowed to be caught by the inner
            // condition-case.
            if condition_case_is_muted(cc_frame_idx) {
                return Err(err);
            }
            let mut handlers = rest;
            while let Some((handler, more)) = handlers.destructure_cons() {
                let condition = handler.first().unwrap_or(LispObject::nil());
                let handler_body = handler.rest().unwrap_or(LispObject::nil());

                if condition_matches(&err, &condition, state) {
                    // Catching ends any active handler-bind dispatch
                    // mute that was propagating with this error.
                    release_mute_on_catch();
                    let parent_env = Arc::new(env.read().clone());
                    let handler_env = Arc::new(RwLock::new(Environment::with_parent(parent_env)));

                    if !var.is_nil()
                        && let Some(var_name) = var.as_symbol()
                    {
                        let signal = err.to_signal();
                        let err_value = if let ElispError::Signal(sig) = signal {
                            LispObject::cons(sig.symbol, sig.data)
                        } else {
                            LispObject::nil()
                        };
                        handler_env.write().define(&var_name, err_value);
                    }

                    return eval_progn(
                        obj_to_value(handler_body),
                        &handler_env,
                        editor,
                        macros,
                        state,
                    );
                }
                handlers = more;
            }
            Err(err)
        }
    }
}
pub(super) fn eval_signal(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let symbol_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let data_expr = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;

    let symbol = value_to_obj(eval(obj_to_value(symbol_expr), env, editor, macros, state)?);
    let data = value_to_obj(eval(obj_to_value(data_expr), env, editor, macros, state)?);

    let err = ElispError::Signal(Box::new(SignalData { symbol, data }));
    dispatch_signal(&err, editor, macros, state)?;
    Err(err)
}
pub(super) fn eval_error_fn(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    // Handle (error nil) — signal with nil data, matching Emacs behavior.
    let args_obj = value_to_obj(args);
    let first = args_obj.first().unwrap_or(LispObject::nil());
    let first_val = value_to_obj(eval(obj_to_value(first), env, editor, macros, state)?);
    if first_val.is_nil() {
        let err = ElispError::Signal(Box::new(SignalData {
            symbol: LispObject::symbol("error"),
            data: LispObject::nil(),
        }));
        dispatch_signal(&err, editor, macros, state)?;
        return Err(err);
    }
    let formatted = value_to_obj(eval_format(args, env, editor, macros, state)?);
    let msg_str = formatted.princ_to_string();

    let err = ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("error"),
        data: LispObject::cons(LispObject::string(&msg_str), LispObject::nil()),
    }));
    dispatch_signal(&err, editor, macros, state)?;
    Err(err)
}
pub(super) fn eval_user_error_fn(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let formatted = value_to_obj(eval_format(args, env, editor, macros, state)?);
    let msg_str = formatted.princ_to_string();

    let err = ElispError::Signal(Box::new(SignalData {
        symbol: LispObject::symbol("user-error"),
        data: LispObject::cons(LispObject::string(&msg_str), LispObject::nil()),
    }));
    dispatch_signal(&err, editor, macros, state)?;
    Err(err)
}
pub(super) fn eval_unwind_protect(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let bodyform = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let cleanup_forms = args_obj.rest().unwrap_or(LispObject::nil());

    // Must fully resolve any tail-call bouncing before running cleanup.
    let body_result = eval(obj_to_value(bodyform), env, editor, macros, state);

    // Cleanup forms always run, even on error. Ignore their result.
    let _ = eval_progn(obj_to_value(cleanup_forms), env, editor, macros, state);

    body_result
}

pub(super) fn eval_handler_bind(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let bindings = args_obj.first().unwrap_or(LispObject::nil());
    let body = args_obj.rest().unwrap_or(LispObject::nil());

    let mut entries: Vec<HandlerBindEntry> = Vec::new();
    let mut cur = bindings;
    while let Some((binding, rest)) = cur.destructure_cons() {
        let conditions = binding.first().unwrap_or(LispObject::nil());
        let handler_expr = binding.nth(1).unwrap_or(LispObject::nil());
        let handler =
            value_to_obj(eval(obj_to_value(handler_expr), env, editor, macros, state)?);
        entries.push(HandlerBindEntry {
            conditions,
            handler,
        });
        cur = rest;
    }

    let frame = HandlerBindFrame {
        entries,
        env: env.clone(),
    };
    HANDLER_BIND_STACK.with(|s| s.borrow_mut().push(frame));
    let result = eval_progn(obj_to_value(body), env, editor, macros, state);
    HANDLER_BIND_STACK.with(|s| {
        s.borrow_mut().pop();
    });
    result
}

pub(super) fn eval_handler_bind_1(
    args: Value,
    env: &Arc<RwLock<Environment>>,
    editor: &Arc<RwLock<Option<Box<dyn EditorCallbacks>>>>,
    macros: &MacroTable,
    state: &InterpreterState,
) -> ElispResult<Value> {
    let args_obj = value_to_obj(args);
    let body_fn_expr = args_obj.first().ok_or(ElispError::WrongNumberOfArguments)?;
    let condition_expr = args_obj.nth(1).ok_or(ElispError::WrongNumberOfArguments)?;
    let handler_expr = args_obj.nth(2).ok_or(ElispError::WrongNumberOfArguments)?;

    let body_fn = eval(obj_to_value(body_fn_expr), env, editor, macros, state)?;
    let conditions =
        value_to_obj(eval(obj_to_value(condition_expr), env, editor, macros, state)?);
    let handler = value_to_obj(eval(obj_to_value(handler_expr), env, editor, macros, state)?);

    let frame = HandlerBindFrame {
        entries: vec![HandlerBindEntry {
            conditions,
            handler,
        }],
        env: env.clone(),
    };
    HANDLER_BIND_STACK.with(|s| s.borrow_mut().push(frame));
    let result = call_function(
        body_fn,
        obj_to_value(LispObject::nil()),
        env,
        editor,
        macros,
        state,
    );
    HANDLER_BIND_STACK.with(|s| {
        s.borrow_mut().pop();
    });
    result
}
