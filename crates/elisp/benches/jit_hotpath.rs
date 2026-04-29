#![allow(clippy::disallowed_methods)]
//! JIT safety/performance microbenchmarks.
//!
//! Run with:
//! `cargo bench -p rele-elisp --features jit --bench jit_hotpath`
//!
//! The benchmark compares the same bytecode function through the VM-only
//! interpreter path and the eager-compiled JIT path. It is intentionally small:
//! Milestone 7 needs a stable measured gate before we expand opcode coverage.

use criterion::{Criterion, black_box, criterion_group, criterion_main};
#[cfg(feature = "jit")]
use rele_elisp::{
    BytecodeFunction, Interpreter, LispObject, add_primitives, jit::bytecode_jit_coverage, read,
};

#[cfg(feature = "jit")]
fn bytecode_add_two_args() -> BytecodeFunction {
    BytecodeFunction {
        argdesc: 0x0202,
        bytecode: vec![0x01, 0x01, 0x5c, 0x87],
        constants: vec![],
        maxdepth: 4,
        docstring: None,
        interactive: None,
    }
}

#[cfg(feature = "jit")]
fn bytecode_constant2() -> BytecodeFunction {
    let mut constants = vec![LispObject::nil(); 70];
    constants[68] = LispObject::Integer(1234);
    BytecodeFunction {
        argdesc: 0x0000,
        bytecode: vec![0x81, 0x44, 0x00, 0x87],
        constants,
        maxdepth: 1,
        docstring: None,
        interactive: None,
    }
}

#[cfg(feature = "jit")]
fn bytecode_stack_ref1_add1() -> BytecodeFunction {
    BytecodeFunction {
        argdesc: 0x0101,
        bytecode: vec![0x06, 0x00, 0x54, 0x87],
        constants: vec![],
        maxdepth: 2,
        docstring: None,
        interactive: None,
    }
}

#[cfg(feature = "jit")]
fn interpreter_with_bytecode(name: &str, func: BytecodeFunction) -> Interpreter {
    let mut interp = Interpreter::new();
    add_primitives(&mut interp);
    interp.define(name, LispObject::BytecodeFn(func));
    interp
}

#[cfg(feature = "jit")]
fn real_supported_zero_arg_bytecode() -> Option<BytecodeFunction> {
    let emacs_dir = rele_elisp::eval::bootstrap::emacs_lisp_dir()?;
    for rel in [
        "emacs-lisp/bytecomp.elc",
        "emacs-lisp/cl-lib.elc",
        "emacs-lisp/cl-macs.elc",
        "textmodes/text-mode.elc",
    ] {
        let path = format!("{emacs_dir}/{rel}");
        let Some(source) = read_file_lossy(&path) else {
            continue;
        };
        let Ok(forms) = rele_elisp::read_all(&source) else {
            continue;
        };
        for form in forms {
            if let Some(func) = find_supported_zero_arg_bytecode(&form) {
                return Some(func);
            }
        }
    }
    None
}

#[cfg(feature = "jit")]
fn find_supported_zero_arg_bytecode(object: &LispObject) -> Option<BytecodeFunction> {
    match object {
        LispObject::BytecodeFn(func)
            if func.min_args() == 0
                && func.max_args() == 0
                && bytecode_jit_coverage(func).is_fully_supported() =>
        {
            Some(func.clone())
        }
        LispObject::Cons(cell) => {
            let (car, cdr) = cell.lock().clone();
            find_supported_zero_arg_bytecode(&car)
                .or_else(|| find_supported_zero_arg_bytecode(&cdr))
        }
        LispObject::Vector(items) => items
            .lock()
            .iter()
            .find_map(find_supported_zero_arg_bytecode),
        LispObject::HashTable(table) => table
            .lock()
            .data
            .values()
            .find_map(find_supported_zero_arg_bytecode),
        _ => None,
    }
}

#[cfg(feature = "jit")]
fn read_file_lossy(path: &str) -> Option<String> {
    std::fs::read_to_string(path).ok().or_else(|| {
        std::fs::read(path)
            .ok()
            .map(|bytes| bytes.iter().map(|&b| char::from(b)).collect())
    })
}

#[cfg(feature = "jit")]
fn bench_jit_hotpath(c: &mut Criterion) {
    let vm = interpreter_with_bytecode("bench-vm-plus", bytecode_add_two_args());
    *vm.state.profiler.write() = rele_elisp::jit::Profiler::new(u64::MAX);
    let vm_form = read("(bench-vm-plus 20 22)").unwrap();

    let jit = interpreter_with_bytecode("bench-jit-plus", bytecode_add_two_args());
    jit.jit_compile("bench-jit-plus").unwrap();
    let jit_form = read("(bench-jit-plus 20 22)").unwrap();

    let constant2_vm = interpreter_with_bytecode("bench-vm-constant2", bytecode_constant2());
    *constant2_vm.state.profiler.write() = rele_elisp::jit::Profiler::new(u64::MAX);
    let constant2_vm_form = read("(bench-vm-constant2)").unwrap();

    let constant2_jit = interpreter_with_bytecode("bench-jit-constant2", bytecode_constant2());
    constant2_jit.jit_compile("bench-jit-constant2").unwrap();
    let constant2_jit_form = read("(bench-jit-constant2)").unwrap();

    let stack_ref1_vm =
        interpreter_with_bytecode("bench-vm-stack-ref1", bytecode_stack_ref1_add1());
    *stack_ref1_vm.state.profiler.write() = rele_elisp::jit::Profiler::new(u64::MAX);
    let stack_ref1_vm_form = read("(bench-vm-stack-ref1 41)").unwrap();

    let stack_ref1_jit =
        interpreter_with_bytecode("bench-jit-stack-ref1", bytecode_stack_ref1_add1());
    stack_ref1_jit.jit_compile("bench-jit-stack-ref1").unwrap();
    let stack_ref1_jit_form = read("(bench-jit-stack-ref1 41)").unwrap();

    let real_bytecode = real_supported_zero_arg_bytecode();
    let real_vm = real_bytecode
        .clone()
        .map(|func| interpreter_with_bytecode("bench-vm-real-elc", func));
    if let Some(vm) = &real_vm {
        *vm.state.profiler.write() = rele_elisp::jit::Profiler::new(u64::MAX);
    }
    let real_vm_form = read("(bench-vm-real-elc)").unwrap();
    let real_jit = real_bytecode.map(|func| interpreter_with_bytecode("bench-jit-real-elc", func));
    if let Some(jit) = &real_jit {
        jit.jit_compile("bench-jit-real-elc").unwrap();
    }
    let real_jit_form = read("(bench-jit-real-elc)").unwrap();

    let mut group = c.benchmark_group("jit_hotpath");
    group.bench_function("vm_bytecode_add", |b| {
        b.iter(|| black_box(vm.eval(black_box(vm_form.clone())).unwrap()))
    });
    group.bench_function("jit_bytecode_add", |b| {
        b.iter(|| black_box(jit.eval(black_box(jit_form.clone())).unwrap()))
    });
    group.bench_function("vm_constant2", |b| {
        b.iter(|| {
            black_box(
                constant2_vm
                    .eval(black_box(constant2_vm_form.clone()))
                    .unwrap(),
            )
        })
    });
    group.bench_function("jit_constant2", |b| {
        b.iter(|| {
            black_box(
                constant2_jit
                    .eval(black_box(constant2_jit_form.clone()))
                    .unwrap(),
            )
        })
    });
    group.bench_function("vm_stack_ref1", |b| {
        b.iter(|| {
            black_box(
                stack_ref1_vm
                    .eval(black_box(stack_ref1_vm_form.clone()))
                    .unwrap(),
            )
        })
    });
    group.bench_function("jit_stack_ref1", |b| {
        b.iter(|| {
            black_box(
                stack_ref1_jit
                    .eval(black_box(stack_ref1_jit_form.clone()))
                    .unwrap(),
            )
        })
    });
    if let (Some(real_vm), Some(real_jit)) = (real_vm, real_jit) {
        group.bench_function("vm_real_elc_zero_arg", |b| {
            b.iter(|| black_box(real_vm.eval(black_box(real_vm_form.clone())).unwrap()))
        });
        group.bench_function("jit_real_elc_zero_arg", |b| {
            b.iter(|| black_box(real_jit.eval(black_box(real_jit_form.clone())).unwrap()))
        });
    }
    group.finish();
}

#[cfg(not(feature = "jit"))]
fn bench_jit_hotpath(c: &mut Criterion) {
    c.bench_function("jit_hotpath/disabled_without_feature", |b| {
        b.iter(|| black_box(0))
    });
}

criterion_group!(benches, bench_jit_hotpath);
criterion_main!(benches);
