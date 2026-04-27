#!/bin/bash
# Refresh the ERT baseline: build the worker, run it on every file in
# tractable.list (one file per worker invocation so a crash on file N
# doesn't lose results for files >N), and print a summary table.
#
# Usage:
#   ./refresh.sh                    # run all files in tractable.list
#   ./refresh.sh path/to/foo.el     # run a single file (no tractable.list)
#
# Outputs:
#   tmp/ert-baseline.jsonl          # raw worker results (one ERT result per line)
#   stdout                          # per-file summary + top failure patterns

set -u
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../../.." && pwd)"
WORKER="$REPO_ROOT/target/release/emacs_test_worker"
BASELINE="$REPO_ROOT/tmp/ert-baseline.jsonl"
PER_TEST_MS="${PER_TEST_MS:-2000}"
PER_FILE_TIMEOUT="${PER_FILE_TIMEOUT:-60}"

mkdir -p "$REPO_ROOT/tmp"

# Build worker if missing or out-of-date.
if ! cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" \
        -p rele-elisp --bin emacs_test_worker 2>/dev/null; then
  echo "build failed" >&2
  exit 1
fi

if [ "$#" -gt 0 ]; then
  FILES=("$@")
else
  if [ ! -f "$SCRIPT_DIR/tractable.list" ]; then
    echo "tractable.list not found in $SCRIPT_DIR" >&2
    exit 1
  fi
  mapfile -t FILES < <(grep -v '^\s*#' "$SCRIPT_DIR/tractable.list" | grep -v '^\s*$')
fi

> "$BASELINE"
for f in "${FILES[@]}"; do
  if [ ! -f "$f" ]; then
    echo "skip (missing): $f" >&2
    continue
  fi
  echo "$f" | timeout "$PER_FILE_TIMEOUT" "$WORKER" \
      --per-test-ms "$PER_TEST_MS" 2>/dev/null \
    | grep -v '^__DONE__$' >> "$BASELINE" || true
done
echo "all-done" >> "$BASELINE"

python3 "$SCRIPT_DIR/summarize.py" "$BASELINE"
