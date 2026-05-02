//! Real sqlite primitives backed by the bundled `rusqlite` crate.
//! Connections are tracked in a process-wide registry; Lisp sees them
//! as `(sqlite . id)` cons cells.

use std::collections::HashMap;
use std::sync::LazyLock;

use parking_lot::Mutex;
use rusqlite::{Connection, params_from_iter, types::Value as SqlValue};

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

#[derive(Default)]
struct Registry {
    conns: HashMap<usize, Connection>,
    next_id: usize,
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(|| Mutex::new(Registry::default()));

fn make_handle(id: usize) -> LispObject {
    LispObject::cons(LispObject::symbol("sqlite"), LispObject::integer(id as i64))
}

fn handle_id(obj: &LispObject) -> Option<usize> {
    let (car, cdr) = obj.destructure_cons()?;
    if car.as_symbol().as_deref() != Some("sqlite") {
        return None;
    }
    cdr.as_integer().map(|n| n as usize)
}

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for name in [
        "sqlite-available-p",
        "sqlite-open",
        "sqlite-close",
        "sqlite-execute",
        "sqlite-execute-batch",
        "sqlite-select",
        "sqlite-transaction",
        "sqlite-commit",
        "sqlite-rollback",
        "sqlitep",
        "sqlite-pragma",
        "sqlite-load-extension",
        "sqlite-version",
    ] {
        interp.define(name, LispObject::primitive(name));
    }
}

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    let result = match name {
        "sqlite-available-p" => Ok(LispObject::t()),
        "sqlite-version" => Ok(LispObject::string(rusqlite::version())),
        "sqlitep" => Ok(LispObject::from(
            args.first()
                .as_ref()
                .map(handle_id)
                .unwrap_or(None)
                .is_some(),
        )),
        "sqlite-open" => prim_sqlite_open(args),
        "sqlite-close" => prim_sqlite_close(args),
        "sqlite-execute" => prim_sqlite_execute(args),
        "sqlite-execute-batch" => prim_sqlite_execute_batch(args),
        "sqlite-select" => prim_sqlite_select(args),
        "sqlite-transaction" => prim_sqlite_simple(args, "BEGIN"),
        "sqlite-commit" => prim_sqlite_simple(args, "COMMIT"),
        "sqlite-rollback" => prim_sqlite_simple(args, "ROLLBACK"),
        "sqlite-pragma" => prim_sqlite_pragma(args),
        // Real Emacs signals when no extensions are loaded; without an
        // actual sqlite3_load_extension hookup we always signal so
        // (should-error (sqlite-load-extension ...)) holds.
        "sqlite-load-extension" => Err(ElispError::EvalError(
            "sqlite-load-extension: extensions not enabled in headless build".into(),
        )),
        _ => return None,
    };
    Some(result)
}

fn prim_sqlite_open(args: &LispObject) -> ElispResult<LispObject> {
    let path = args.first().and_then(|a| a.as_string().cloned());
    let conn = match path.as_deref() {
        None | Some("") => Connection::open_in_memory(),
        Some(p) => Connection::open(p),
    }
    .map_err(|e| ElispError::EvalError(format!("sqlite-open: {e}")))?;
    let mut reg = REGISTRY.lock();
    let id = reg.next_id;
    reg.next_id += 1;
    reg.conns.insert(id, conn);
    Ok(make_handle(id))
}

fn prim_sqlite_close(args: &LispObject) -> ElispResult<LispObject> {
    let Some(id) = args.first().and_then(|a| handle_id(&a)) else {
        return Ok(LispObject::nil());
    };
    REGISTRY.lock().conns.remove(&id);
    Ok(LispObject::t())
}

fn lisp_to_sql(value: &LispObject) -> SqlValue {
    match value {
        LispObject::Nil => SqlValue::Null,
        LispObject::Integer(n) => SqlValue::Integer(*n),
        LispObject::Float(f) => SqlValue::Real(*f),
        LispObject::T => SqlValue::Integer(1),
        other => {
            if let Some(s) = other.as_string() {
                let raw = crate::object::current_string_value(s);
                if string_has_binary_coding_property(other) {
                    SqlValue::Blob(string_bytes(&raw))
                } else {
                    SqlValue::Text(raw)
                }
            } else {
                SqlValue::Text(other.princ_to_string())
            }
        }
    }
}

fn string_bytes(s: &str) -> Vec<u8> {
    s.chars()
        .map(|ch| {
            let code = ch as u32;
            if (0xE000..=0xE0FF).contains(&code) {
                (code - 0xE000) as u8
            } else {
                code as u8
            }
        })
        .collect()
}

fn string_has_binary_coding_property(value: &LispObject) -> bool {
    let args = LispObject::cons(
        LispObject::integer(0),
        LispObject::cons(
            LispObject::symbol("coding-system"),
            LispObject::cons(value.clone(), LispObject::nil()),
        ),
    );
    crate::primitives_buffer::prim_get_text_property(&args)
        .ok()
        .and_then(|value| value.as_symbol())
        .as_deref()
        == Some("binary")
}

/// Lisp parameter lists for sqlite primitives accept either a list or
/// a vector of values. Walk both shapes into a flat Vec<SqlValue>.
fn collect_params(params: LispObject) -> Vec<SqlValue> {
    if let LispObject::Vector(items) = &params {
        return items.lock().iter().map(lisp_to_sql).collect();
    }
    let mut out = Vec::new();
    let mut cur = params;
    while let Some((head, tail)) = cur.destructure_cons() {
        out.push(lisp_to_sql(&head));
        cur = tail;
    }
    out
}

fn sql_to_lisp(value: &SqlValue) -> LispObject {
    match value {
        SqlValue::Null => LispObject::nil(),
        SqlValue::Integer(n) => LispObject::integer(*n),
        SqlValue::Real(f) => LispObject::Float(*f),
        SqlValue::Text(s) => LispObject::String(crate::object::string_with_multibyte_flag(s, true)),
        SqlValue::Blob(b) => {
            // BLOBs round-trip as unibyte strings — chars 0x00..=0xFF
            // map directly to bytes. Use the latin-1 view so we don't
            // mojibake binary data through utf-8.
            let s: String = b.iter().map(|byte| char::from(*byte)).collect();
            LispObject::String(crate::object::string_with_multibyte_flag(&s, false))
        }
    }
}

fn with_conn<F, R>(args: &LispObject, f: F) -> ElispResult<R>
where
    F: FnOnce(&mut Connection) -> ElispResult<R>,
{
    let Some(id) = args.first().and_then(|a| handle_id(&a)) else {
        return Err(ElispError::WrongTypeArgument("sqlite".into()));
    };
    let mut reg = REGISTRY.lock();
    let conn = reg
        .conns
        .get_mut(&id)
        .ok_or_else(|| ElispError::EvalError("sqlite: closed connection".into()))?;
    f(conn)
}

fn prim_sqlite_execute(args: &LispObject) -> ElispResult<LispObject> {
    let sql = args
        .nth(1)
        .and_then(|a| a.as_string().cloned())
        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let params = args.nth(2).unwrap_or_else(LispObject::nil);
    let collected = collect_params(params);
    with_conn(args, |conn| {
        // Always go through prepare+query. `Connection::execute` would
        // reject RETURNING-style statements with `ExecuteReturnedResults`,
        // and falling back to a second prepare-and-query would run the
        // INSERT twice — silently duplicating rows. Prepare once,
        // collect any rows it produces, then return either the rows
        // (RETURNING) or the change count (plain INSERT/UPDATE/DELETE).
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| ElispError::EvalError(format!("sqlite-execute: {e}")))?;
        let column_count = stmt.column_count();
        let mut rows_iter = stmt
            .query(params_from_iter(collected.iter()))
            .map_err(|e| ElispError::EvalError(format!("sqlite-execute: {e}")))?;
        let mut rows: Vec<Vec<SqlValue>> = Vec::new();
        while let Some(row) = rows_iter
            .next()
            .map_err(|e| ElispError::EvalError(format!("sqlite-execute: {e}")))?
        {
            let mut current = Vec::with_capacity(column_count);
            for i in 0..column_count {
                let value: SqlValue = row
                    .get(i)
                    .map_err(|e| ElispError::EvalError(format!("sqlite-execute: {e}")))?;
                current.push(value);
            }
            rows.push(current);
        }
        drop(rows_iter);
        drop(stmt);
        if column_count > 0 && !rows.is_empty() {
            let mut out = LispObject::nil();
            for row in rows.into_iter().rev() {
                let mut row_list = LispObject::nil();
                for col in row.into_iter().rev() {
                    row_list = LispObject::cons(sql_to_lisp(&col), row_list);
                }
                out = LispObject::cons(row_list, out);
            }
            Ok(out)
        } else {
            let changed = conn.changes();
            Ok(LispObject::integer(changed as i64))
        }
    })
}

/// `(sqlite-execute-batch DB SQL)` — run a multi-statement script.
fn prim_sqlite_execute_batch(args: &LispObject) -> ElispResult<LispObject> {
    let sql = args
        .nth(1)
        .and_then(|a| a.as_string().cloned())
        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    with_conn(args, |conn| {
        conn.execute_batch(&sql)
            .map_err(|e| ElispError::EvalError(format!("sqlite-execute-batch: {e}")))?;
        Ok(LispObject::t())
    })
}

fn prim_sqlite_select(args: &LispObject) -> ElispResult<LispObject> {
    let sql = args
        .nth(1)
        .and_then(|a| a.as_string().cloned())
        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let params = args.nth(2).unwrap_or_else(LispObject::nil);
    let format = args
        .nth(3)
        .and_then(|a| a.as_symbol())
        .map(|s| s.to_string());
    let with_columns = matches!(format.as_deref(), Some("full"));
    let collected = collect_params(params);
    with_conn(args, |conn| {
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| ElispError::EvalError(format!("sqlite-select: {e}")))?;
        let column_count = stmt.column_count();
        let column_names: Vec<String> = (0..column_count)
            .map(|i| stmt.column_name(i).unwrap_or("").to_string())
            .collect();
        let mut rows_iter = stmt
            .query(params_from_iter(collected.iter()))
            .map_err(|e| ElispError::EvalError(format!("sqlite-select: {e}")))?;
        let mut rows: Vec<Vec<SqlValue>> = Vec::new();
        while let Some(row) = rows_iter
            .next()
            .map_err(|e| ElispError::EvalError(format!("sqlite-select: {e}")))?
        {
            let mut current = Vec::with_capacity(column_count);
            for i in 0..column_count {
                let value: SqlValue = row
                    .get(i)
                    .map_err(|e| ElispError::EvalError(format!("sqlite-select: {e}")))?;
                current.push(value);
            }
            rows.push(current);
        }
        // Build (row1 row2 ...) where rowN is (col1 col2 ...).
        let mut out = LispObject::nil();
        for row in rows.into_iter().rev() {
            let mut row_list = LispObject::nil();
            for col in row.into_iter().rev() {
                row_list = LispObject::cons(sql_to_lisp(&col), row_list);
            }
            out = LispObject::cons(row_list, out);
        }
        if with_columns {
            let mut header = LispObject::nil();
            for name in column_names.into_iter().rev() {
                header = LispObject::cons(LispObject::string(&name), header);
            }
            out = LispObject::cons(header, out);
        }
        Ok(out)
    })
}

fn prim_sqlite_simple(args: &LispObject, sql: &str) -> ElispResult<LispObject> {
    with_conn(args, |conn| {
        conn.execute_batch(sql)
            .map_err(|e| ElispError::EvalError(format!("sqlite: {e}")))?;
        Ok(LispObject::t())
    })
}

fn prim_sqlite_pragma(args: &LispObject) -> ElispResult<LispObject> {
    let pragma = args
        .nth(1)
        .and_then(|a| a.as_string().cloned())
        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    with_conn(args, |conn| {
        let sql = format!("PRAGMA {}", pragma);
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| ElispError::EvalError(format!("sqlite-pragma: {e}")))?;
        let mut rows = stmt
            .query([])
            .map_err(|e| ElispError::EvalError(format!("sqlite-pragma: {e}")))?;
        if let Some(row) = rows
            .next()
            .map_err(|e| ElispError::EvalError(format!("sqlite-pragma: {e}")))?
        {
            let v: SqlValue = row
                .get(0)
                .map_err(|e| ElispError::EvalError(format!("sqlite-pragma: {e}")))?;
            return Ok(sql_to_lisp(&v));
        }
        Ok(LispObject::nil())
    })
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
    fn round_trip_in_memory() {
        let conn = prim_sqlite_open(&args(vec![LispObject::string(":memory:")])).expect("open");
        prim_sqlite_execute(&args(vec![
            conn.clone(),
            LispObject::string("CREATE TABLE t(id INTEGER, name TEXT)"),
            LispObject::nil(),
        ]))
        .expect("create");
        prim_sqlite_execute(&args(vec![
            conn.clone(),
            LispObject::string("INSERT INTO t VALUES (?, ?)"),
            args(vec![LispObject::integer(1), LispObject::string("alice")]),
        ]))
        .expect("insert");
        let rows = prim_sqlite_select(&args(vec![
            conn.clone(),
            LispObject::string("SELECT id, name FROM t"),
            LispObject::nil(),
        ]))
        .expect("select");
        assert_eq!(rows.princ_to_string(), "((1 \"alice\"))");
        prim_sqlite_close(&args(vec![conn])).expect("close");
    }

    #[test]
    fn select_full_format_includes_column_names() {
        let conn = prim_sqlite_open(&args(vec![LispObject::string(":memory:")])).expect("open");
        prim_sqlite_execute(&args(vec![
            conn.clone(),
            LispObject::string("CREATE TABLE t(col1 TEXT, col2 INTEGER)"),
            LispObject::nil(),
        ]))
        .expect("create");
        prim_sqlite_execute(&args(vec![
            conn.clone(),
            LispObject::string("INSERT INTO t VALUES ('foo', 2)"),
            LispObject::nil(),
        ]))
        .expect("insert");
        let rows = prim_sqlite_select(&args(vec![
            conn,
            LispObject::string("SELECT * FROM t"),
            LispObject::nil(),
            LispObject::symbol("full"),
        ]))
        .expect("select-full");
        assert_eq!(rows.princ_to_string(), "((\"col1\" \"col2\") (\"foo\" 2))");
    }
}
