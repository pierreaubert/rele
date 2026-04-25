use super::InterpreterState;
use crate::object::LispObject;

pub(super) fn get_or_create_runtime_char_table(
    state: &InterpreterState,
    storage_symbol: &str,
    purpose: &str,
    with_upcase_slot: bool,
) -> LispObject {
    let id = crate::obarray::intern(storage_symbol);
    if let Some(table) = state.get_value_cell(id) {
        return table;
    }
    let table =
        crate::primitives::core::make_char_table(LispObject::symbol(purpose), LispObject::nil());
    if with_upcase_slot {
        let up = crate::primitives::core::make_char_table(
            LispObject::symbol(purpose),
            LispObject::nil(),
        );
        let _ =
            crate::primitives::core::char_table_set_extra_slot(&table, &LispObject::integer(0), up);
    }
    state.set_value_cell(id, table.clone());
    table
}

pub(super) fn is_generated_translation_table_let(args: &LispObject) -> bool {
    let (bindings, body) = args.clone().destructure();
    if !is_translation_table_map_binding(bindings) {
        return false;
    }
    form_contains_call(&body, "define-translation-table")
}

pub(super) fn register_generated_translation_table_let(
    args: &LispObject,
    state: &InterpreterState,
) {
    let (bindings, body) = args.clone().destructure();
    let Some(map) = translation_table_map_binding_value(bindings) else {
        return;
    };
    for name in translation_table_names(&body) {
        state
            .translation_tables
            .write()
            .insert(name.clone(), map.clone());
        state.set_value_cell(crate::obarray::intern(&name), map.clone());
    }
}

pub(super) fn is_cus_start_properties_let(args: &LispObject) -> bool {
    let (bindings, body) = args.clone().destructure();
    let binds_standard = let_binds_symbol(&bindings, "standard");
    let binds_quoter = let_binds_symbol(&bindings, "quoter");
    binds_standard && binds_quoter && form_contains_call(&body, "pcase-dolist")
}

fn let_binds_symbol(bindings: &LispObject, target: &str) -> bool {
    let mut cur = bindings.clone();
    while let Some((binding, rest)) = cur.destructure_cons() {
        let name = binding
            .first()
            .and_then(|obj| obj.as_symbol())
            .or_else(|| binding.as_symbol());
        if name.as_deref() == Some(target) {
            return true;
        }
        cur = rest;
    }
    false
}

fn is_translation_table_map_binding(bindings: LispObject) -> bool {
    let mut head = bindings;
    let mut count = 0;
    while !head.is_nil() {
        let (binding, rest) = match head.destructure_cons() {
            Some(v) => v,
            None => return false,
        };
        if !is_map_quote_binding(&binding) {
            return false;
        }
        count += 1;
        head = rest;
    }
    count == 1
}

fn translation_table_map_binding_value(bindings: LispObject) -> Option<LispObject> {
    let (binding, rest) = bindings.destructure_cons()?;
    if !rest.is_nil() {
        return None;
    }
    let (name, init) = binding.destructure_cons()?;
    if name.as_symbol().as_deref() != Some("map") {
        return None;
    }
    let init_expr = init.first()?;
    let (quote, quoted) = init_expr.destructure_cons()?;
    if quote.as_symbol().as_deref() != Some("quote") {
        return None;
    }
    quoted.first()
}

fn translation_table_names(form: &LispObject) -> Vec<String> {
    let mut out = Vec::new();
    collect_translation_table_names(form, &mut out);
    out
}

fn collect_translation_table_names(form: &LispObject, out: &mut Vec<String>) {
    let Some((head, rest)) = form.destructure_cons() else {
        return;
    };
    if head.as_symbol().as_deref() == Some("define-translation-table") {
        if let Some(name_form) = rest.first() {
            let name = name_form
                .as_quote_content()
                .and_then(|obj| obj.as_symbol())
                .or_else(|| name_form.as_symbol());
            if let Some(name) = name {
                out.push(name);
            }
        }
    }
    collect_translation_table_names(&head, out);
    collect_translation_table_names(&rest, out);
}

fn is_map_quote_binding(binding: &LispObject) -> bool {
    let (name, init) = match binding.destructure_cons() {
        Some(v) => v,
        None => return false,
    };
    if name.as_symbol().as_deref() != Some("map") {
        return false;
    }
    if !init.cdr().is_some_and(|tail| tail.is_nil()) {
        return false;
    }
    let init_expr = match init.first() {
        Some(v) => v,
        None => return false,
    };
    quoted_list_expr(&init_expr)
}

fn quoted_list_expr(form: &LispObject) -> bool {
    let (car, rest) = form.clone().destructure();
    if car.as_symbol().as_deref() != Some("quote") {
        return false;
    }
    matches!(rest.first(), Some(list) if list.is_cons() || list.is_nil())
}

fn form_contains_call(form: &LispObject, target: &str) -> bool {
    if !form.is_cons() {
        return false;
    }
    let (car, cdr) = match form.destructure_cons() {
        Some(v) => v,
        None => return false,
    };
    if car.as_symbol().as_deref() == Some(target) {
        return true;
    }

    form_contains_call(&car, target) || form_contains_call(&cdr, target)
}
