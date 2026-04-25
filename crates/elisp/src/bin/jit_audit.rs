//! Audit JIT opcode coverage over real bytecode loaded by bootstrap.
//!
//! Usage:
//!   cargo run -p rele-elisp --bin jit_audit

use std::collections::BTreeMap;

use rele_elisp::eval::bootstrap::{
    emacs_lisp_dir, ensure_stdlib_files, load_full_bootstrap, make_stdlib_interp,
};
use rele_elisp::jit::{bytecode_jit_coverage, opcode_name};
use rele_elisp::{BytecodeFunction, LispObject};

fn main() {
    let handle = std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run)
        .expect("spawn");
    handle.join().expect("join");
}

fn run() {
    if !ensure_stdlib_files() {
        eprintln!("Cannot find Emacs lisp dir. Set EMACS_LISP_DIR env.");
        std::process::exit(2);
    }

    let interp = make_stdlib_interp();
    eprintln!("Running full bootstrap...");
    load_full_bootstrap(&interp);
    eprintln!("Bootstrap complete.");

    let mut functions = interp.bytecode_functions();
    let installed_count = functions.len();
    functions.extend(scan_real_elc_bytecode());
    functions.sort_by(|(left, _), (right, _)| left.cmp(right));

    let mut total_opcodes = 0usize;
    let mut fully_supported = 0usize;
    let mut supported_samples = Vec::new();
    let mut unsupported_by_opcode: BTreeMap<u8, usize> = BTreeMap::new();
    let mut partial_functions = Vec::new();

    for (name, func) in &functions {
        let coverage = bytecode_jit_coverage(func);
        total_opcodes += coverage.opcode_count;
        if coverage.is_fully_supported() {
            fully_supported += 1;
            if supported_samples.len() < 10 {
                supported_samples.push((name.clone(), func.min_args(), func.max_args()));
            }
        } else {
            for unsupported in &coverage.unsupported {
                *unsupported_by_opcode.entry(unsupported.opcode).or_default() += 1;
            }
            partial_functions.push((
                name.clone(),
                coverage.opcode_count,
                coverage.unsupported.len(),
                coverage
                    .unsupported
                    .iter()
                    .map(|unsupported| unsupported.opcode)
                    .collect::<Vec<_>>(),
            ));
        }
    }

    partial_functions
        .sort_by(|left, right| right.2.cmp(&left.2).then_with(|| left.0.cmp(&right.0)));
    let mut unsupported_ranked: Vec<(u8, usize)> = unsupported_by_opcode.into_iter().collect();
    unsupported_ranked
        .sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    println!("=== JIT Coverage Audit ===");
    println!("  Bytecode functions: {}", functions.len());
    println!("  Installed cells:    {installed_count}");
    println!(
        "  .elc literals:      {}",
        functions.len().saturating_sub(installed_count)
    );
    println!("  Fully supported:    {fully_supported}");
    println!(
        "  Partially covered:  {}",
        functions.len().saturating_sub(fully_supported)
    );
    println!("  Opcode instances:   {total_opcodes}");

    println!();
    println!("=== Unsupported Opcode Histogram ===");
    if unsupported_ranked.is_empty() {
        println!("  none");
    } else {
        for (opcode, count) in unsupported_ranked.iter().take(20) {
            println!("  {:>3} {:<28} {}", opcode, opcode_name(*opcode), count);
        }
    }

    println!();
    println!("=== Supported Function Samples ===");
    if supported_samples.is_empty() {
        println!("  none");
    } else {
        for (name, min_args, max_args) in supported_samples {
            let max_args = if max_args == usize::MAX {
                "rest".to_string()
            } else {
                max_args.to_string()
            };
            println!("  {name}: min_args={min_args} max_args={max_args}");
        }
    }

    println!();
    println!("=== Top Partial Functions ===");
    if partial_functions.is_empty() {
        println!("  none");
    } else {
        for (name, opcode_count, unsupported_count, opcodes) in partial_functions.iter().take(20) {
            let unique = unique_opcode_names(opcodes);
            println!("  {name}: {unsupported_count}/{opcode_count} unsupported [{unique}]");
        }
    }
}

fn scan_real_elc_bytecode() -> Vec<(String, BytecodeFunction)> {
    let Some(emacs_dir) = emacs_lisp_dir() else {
        return Vec::new();
    };
    let candidates = [
        "international/charscript.elc",
        "international/emoji-zwj.elc",
        "textmodes/text-mode.elc",
        "emacs-lisp/bytecomp.elc",
        "emacs-lisp/cl-extra.elc",
        "emacs-lisp/cl-lib.elc",
        "emacs-lisp/cl-macs.elc",
        "emacs-lisp/cl-seq.elc",
        "emacs-lisp/pcase.elc",
        "emacs-lisp/subr-x.elc",
    ];

    let mut functions = Vec::new();
    for rel in candidates {
        let path = format!("{emacs_dir}/{rel}");
        let Some(source) = read_file_lossy(&path) else {
            continue;
        };
        let Ok(forms) = rele_elisp::read_all(&source) else {
            continue;
        };
        for (idx, form) in forms.iter().enumerate() {
            collect_bytecode_literals(form, &format!("{rel}#{idx}"), &mut functions);
        }
    }
    functions
}

fn collect_bytecode_literals(
    object: &LispObject,
    label: &str,
    out: &mut Vec<(String, BytecodeFunction)>,
) {
    match object {
        LispObject::BytecodeFn(func) => {
            let suffix = out.len();
            out.push((format!("{label}/bytecode-{suffix}"), func.clone()));
        }
        LispObject::Cons(cell) => {
            let (car, cdr) = cell.lock().clone();
            collect_bytecode_literals(&car, label, out);
            collect_bytecode_literals(&cdr, label, out);
        }
        LispObject::Vector(items) => {
            for item in items.lock().iter() {
                collect_bytecode_literals(item, label, out);
            }
        }
        LispObject::HashTable(table) => {
            for value in table.lock().data.values() {
                collect_bytecode_literals(value, label, out);
            }
        }
        _ => {}
    }
}

fn read_file_lossy(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().or_else(|| {
        std::fs::read(path)
            .ok()
            .map(|bytes| bytes.iter().map(|&b| char::from(b)).collect())
    })
}

fn unique_opcode_names(opcodes: &[u8]) -> String {
    let mut unique = opcodes.to_vec();
    unique.sort_unstable();
    unique.dedup();
    unique
        .into_iter()
        .map(|opcode| format!("{opcode}:{}", opcode_name(opcode)))
        .collect::<Vec<_>>()
        .join(", ")
}
