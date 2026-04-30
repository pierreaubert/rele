#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
cd "$repo_root"

run_gate() {
  local label="$1"
  shift
  printf '\n==> %s\n' "$label"
  "$@"
}

run_gate "compile/default tests (no run)" \
  cargo test -p rele-elisp --no-run

run_gate "stub inventory gate" \
  python3 crates/elisp/ert-progress/stub_inventory.py --check

run_gate "runtime/default tests" \
  cargo test -p rele-elisp

run_gate "compile/JIT visibility (no run)" \
  cargo test -p rele-elisp --features jit --no-run

run_gate "bootstrap/load audit" \
  cargo run -p rele-elisp --bin load_audit

run_gate "bootstrap/require audit" \
  cargo run -p rele-elisp --bin require_audit
