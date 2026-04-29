#![allow(clippy::disallowed_methods)]
use crate::error::{ElispError, ElispResult};
use crate::object::LispObject;

pub fn call(name: &str, args: &LispObject) -> Option<ElispResult<LispObject>> {
    match name {
        "make-directory-internal" => Some(prim_make_directory_internal(args)),
        "directory-name-p" => Some(prim_directory_name_p(args)),
        "file-accessible-directory-p" => Some(prim_file_accessible_directory_p(args)),
        "file-name-as-directory" => Some(prim_file_name_as_directory(args)),
        "rename-file" => Some(prim_rename_file(args)),
        "copy-file" => Some(prim_copy_file(args)),
        "delete-directory" => Some(prim_delete_directory(args)),
        "file-modes" => Some(prim_file_modes(args)),
        "file-newer-than-file-p" => Some(prim_file_newer_than_file_p(args)),
        "file-symlink-p" => Some(prim_file_symlink_p(args)),
        "file-regular-p" => Some(prim_file_regular_p(args)),
        "file-readable-p" => Some(prim_file_readable_p(args)),
        "file-writable-p" => Some(prim_file_writable_p(args)),
        "file-executable-p" => Some(prim_file_executable_p(args)),
        _ => None,
    }
}

pub fn add_primitives(interp: &mut crate::eval::Interpreter) {
    for &name in FILE_OPS_PRIMITIVE_NAMES {
        interp.define(name, LispObject::primitive(name));
    }
}

pub const FILE_OPS_PRIMITIVE_NAMES: &[&str] = &[
    "make-directory-internal",
    "directory-name-p",
    "file-accessible-directory-p",
    "file-name-as-directory",
    "rename-file",
    "copy-file",
    "delete-directory",
    "file-modes",
    "set-file-modes",
    "file-newer-than-file-p",
    "file-symlink-p",
    "file-regular-p",
    "file-readable-p",
    "file-writable-p",
    "file-executable-p",
];

/// Extract the Nth argument as a `String`, or return WrongTypeArgument.
fn str_arg(args: &LispObject, n: usize) -> ElispResult<String> {
    match args.nth(n) {
        Some(LispObject::String(s)) => Ok(s),
        _ => Err(ElispError::WrongTypeArgument("string".to_string())),
    }
}

fn prim_make_directory_internal(args: &LispObject) -> ElispResult<LispObject> {
    let name = args
        .first()
        .and_then(|a| match a {
            LispObject::String(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    std::fs::create_dir(&name)
        .map_err(|e| ElispError::EvalError(format!("make-directory-internal: {e}")))?;
    Ok(LispObject::nil())
}

fn prim_directory_name_p(args: &LispObject) -> ElispResult<LispObject> {
    let s = args.first().unwrap_or_else(LispObject::nil);
    if let LispObject::String(s) = s {
        Ok(LispObject::from(s.ends_with('/')))
    } else {
        Err(ElispError::WrongTypeArgument("string".to_string()))
    }
}

fn prim_file_accessible_directory_p(args: &LispObject) -> ElispResult<LispObject> {
    let s = args
        .first()
        .and_then(|a| match a {
            LispObject::String(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    Ok(LispObject::from(
        std::path::Path::new(&s).is_dir() && std::fs::metadata(&s).is_ok(),
    ))
}

fn prim_file_name_as_directory(args: &LispObject) -> ElispResult<LispObject> {
    let s = args
        .first()
        .and_then(|a| match a {
            LispObject::String(s) => Some(s),
            _ => None,
        })
        .ok_or_else(|| ElispError::WrongTypeArgument("string".to_string()))?;
    if s.ends_with('/') {
        Ok(LispObject::string(&s))
    } else {
        Ok(LispObject::string(&format!("{s}/")))
    }
}

fn prim_rename_file(args: &LispObject) -> ElispResult<LispObject> {
    let from = str_arg(args, 0)?;
    let to = str_arg(args, 1)?;
    std::fs::rename(&from, &to).map_err(|e| ElispError::EvalError(format!("rename-file: {e}")))?;
    Ok(LispObject::nil())
}

fn prim_copy_file(args: &LispObject) -> ElispResult<LispObject> {
    let from = str_arg(args, 0)?;
    let to = str_arg(args, 1)?;
    std::fs::copy(&from, &to).map_err(|e| ElispError::EvalError(format!("copy-file: {e}")))?;
    Ok(LispObject::nil())
}

fn prim_delete_directory(args: &LispObject) -> ElispResult<LispObject> {
    let dir = str_arg(args, 0)?;
    let recursive = args.nth(1).is_some_and(|a| !a.is_nil());
    let result = if recursive {
        std::fs::remove_dir_all(&dir)
    } else {
        std::fs::remove_dir(&dir)
    };
    result.map_err(|e| ElispError::EvalError(format!("delete-directory: {e}")))?;
    Ok(LispObject::nil())
}

fn prim_file_modes(args: &LispObject) -> ElispResult<LispObject> {
    let path = str_arg(args, 0)?;
    match std::fs::metadata(&path) {
        Ok(meta) => {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                Ok(LispObject::integer(
                    (meta.permissions().mode() & 0o777) as i64,
                ))
            }
            #[cfg(not(unix))]
            {
                let ro = meta.permissions().readonly();
                Ok(LispObject::integer(if ro { 0o444 } else { 0o644 }))
            }
        }
        Err(_) => Ok(LispObject::nil()),
    }
}

fn prim_file_newer_than_file_p(args: &LispObject) -> ElispResult<LispObject> {
    let a = str_arg(args, 0)?;
    let b = str_arg(args, 1)?;
    let mtime = |p: &str| std::fs::metadata(p).and_then(|m| m.modified()).ok();
    match (mtime(&a), mtime(&b)) {
        (Some(ma), None) => Ok(LispObject::from(ma.elapsed().is_ok())),
        (Some(ma), Some(mb)) => Ok(LispObject::from(ma > mb)),
        _ => Ok(LispObject::nil()),
    }
}

fn prim_file_symlink_p(args: &LispObject) -> ElispResult<LispObject> {
    let path = str_arg(args, 0)?;
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_symlink() => match std::fs::read_link(&path) {
            Ok(target) => Ok(LispObject::string(&target.to_string_lossy())),
            Err(_) => Ok(LispObject::t()),
        },
        _ => Ok(LispObject::nil()),
    }
}

fn prim_file_regular_p(args: &LispObject) -> ElispResult<LispObject> {
    let path = str_arg(args, 0)?;
    Ok(LispObject::from(
        std::fs::metadata(&path).is_ok_and(|m| m.is_file()),
    ))
}

fn prim_file_readable_p(args: &LispObject) -> ElispResult<LispObject> {
    let path = str_arg(args, 0)?;
    Ok(LispObject::from(std::fs::metadata(&path).is_ok()))
}

fn prim_file_writable_p(args: &LispObject) -> ElispResult<LispObject> {
    let path = str_arg(args, 0)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(LispObject::from(
            std::fs::metadata(&path).is_ok_and(|m| m.permissions().mode() & 0o200 != 0),
        ))
    }
    #[cfg(not(unix))]
    {
        Ok(LispObject::from(
            std::fs::metadata(&path).is_ok_and(|m| !m.permissions().readonly()),
        ))
    }
}

fn prim_file_executable_p(args: &LispObject) -> ElispResult<LispObject> {
    let path = str_arg(args, 0)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        Ok(LispObject::from(
            std::fs::metadata(&path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0),
        ))
    }
    #[cfg(not(unix))]
    {
        Ok(LispObject::from(std::fs::metadata(&path).is_ok()))
    }
}
