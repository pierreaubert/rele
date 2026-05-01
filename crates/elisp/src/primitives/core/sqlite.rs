//! Real sqlite primitives backed by the bundled `rusqlite` crate.
//! Connections are tracked in a process-wide registry; Lisp sees them
//! as `(sqlite . id)` cons cells.

use std::collections::HashMap;
use std::sync::LazyLock;

use parking_lot::Mutex;
use rusqlite::{params_from_iter, types::Value as SqlValue, Connection};

use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

#[derive(Default)]
struct Registry {
    conns: HashMap<usize, Connection>,
    next_id: usize,
}

static REGISTRY: LazyLock<Mutex<Registry>> =
    LazyLock::new(|| Mutex::new(Registry::default()));

fn make_handle(id: usize) -> LispObject {
    LispObject::cons(
        LispObject::symbol("sqlite"),
        LispObject::integer(id as i64),
    )
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
        "sqlite-select" => prim_sqlite_select(args),
        "sqlite-transaction" => prim_sqlite_simple(args, "BEGIN"),
        "sqlite-commit" => prim_sqlite_simple(args, "COMMIT"),
        "sqlite-rollback" => prim_sqlite_simple(args, "ROLLBACK"),
        "sqlite-pragma" => prim_sqlite_pragma(args),
        "sqlite-load-extension" => Ok(LispObject::nil()),
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
                SqlValue::Text(s.to_string())
            } else {
                SqlValue::Text(other.princ_to_string())
            }
        }
    }
}

fn collect_params(params: LispObject) -> Vec<SqlValue> {
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
        SqlValue::Text(s) => LispObject::string(s),
        SqlValue::Blob(b) => LispObject::string(&String::from_utf8_lossy(b)),
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
        let changed = conn
            .execute(&sql, params_from_iter(collected.iter()))
            .map_err(|e| ElispError::EvalError(format!("sqlite-execute: {e}")))?;
        Ok(LispObject::integer(changed as i64))
    })
}

fn prim_sqlite_select(args: &LispObject) -> ElispResult<LispObject> {
    let sql = args
        .nth(1)
        .and_then(|a| a.as_string().cloned())
        .ok_or_else(|| ElispError::WrongTypeArgument("string".into()))?;
    let params = args.nth(2).unwrap_or_else(LispObject::nil);
    let collected = collect_params(params);
    with_conn(args, |conn| {
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| ElispError::EvalError(format!("sqlite-select: {e}")))?;
        let column_count = stmt.column_count();
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
        let conn = prim_sqlite_open(&args(vec![LispObject::string(":memory:")]))
            .expect("open");
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
}
