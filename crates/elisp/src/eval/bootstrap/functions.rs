//! Auto-generated module
//!
//! 🤖 Generated with [SplitRS](https://github.com/cool-japan/splitrs)

use super::emacs_lisp_dir;

/// Staging directory for decompressed Emacs stdlib `.el` files.
///
/// Resolved at compile time to `<workspace-root>/tmp/elisp-stdlib` so
/// all scratch data stays under the repo's `tmp/` directory (which is
/// gitignored).
pub const STDLIB_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tmp/elisp-stdlib");
/// Decompress/copy the requested Emacs Lisp files into [`STDLIB_DIR`].
///
/// Each entry is relative to the Emacs `lisp/` directory and omits the
/// `.el` suffix, for example `emacs-lisp/cl-lib`. Existing staged files are
/// left untouched, so this is cheap to call from tests and audit binaries.
pub fn ensure_stdlib_files_for_dir(emacs_lisp_dir: &str, files: &[&str]) -> bool {
    for f in files {
        if !stage_stdlib_file(emacs_lisp_dir, f) {
            return false;
        }
    }
    true
}
/// Like [`ensure_stdlib_files_for_dir`], but discovers the Emacs lisp
/// directory from `EMACS_LISP_DIR` or common install locations.
pub fn ensure_stdlib_files_for(files: &[&str]) -> bool {
    let Some(emacs_lisp_dir) = emacs_lisp_dir() else {
        return false;
    };
    ensure_stdlib_files_for_dir(emacs_lisp_dir, files)
}
fn stage_stdlib_file(emacs_lisp_dir: &str, file: &str) -> bool {
    let dest = format!("{STDLIB_DIR}/{file}.el");
    if std::path::Path::new(&dest).exists() {
        return true;
    }
    if let Some(parent) = std::path::Path::new(&dest).parent() {
        if std::fs::create_dir_all(parent).is_err() {
            return false;
        }
    }
    let plain = format!("{emacs_lisp_dir}/{file}.el");
    let gz = format!("{emacs_lisp_dir}/{file}.el.gz");
    if std::path::Path::new(&plain).exists() {
        return std::fs::copy(&plain, &dest).is_ok();
    }
    if std::path::Path::new(&gz).exists() {
        if let Ok(out) = std::process::Command::new("gunzip")
            .args(["-c", &gz])
            .output()
        {
            return out.status.success() && std::fs::write(&dest, out.stdout).is_ok();
        }
    }
    true
}
