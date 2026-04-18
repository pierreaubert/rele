#!/usr/bin/env bash
# Diff two emacs-test-results.jsonl runs.
# Reports tests that newly pass, newly fail, newly error, or disappeared.
#
# Usage: diff-emacs-results.sh OLD.jsonl NEW.jsonl

set -euo pipefail

if [ $# -ne 2 ]; then
  echo "Usage: $0 OLD.jsonl NEW.jsonl" >&2
  exit 1
fi

OLD="$1"
NEW="$2"

if ! command -v jq >/dev/null 2>&1; then
  echo "Requires jq" >&2
  exit 1
fi

# Each test is keyed by "FILE::TEST". Build {key: result} maps via jq.
to_map() {
  jq -r '"\(.file)::\(.test) \(.result)"' "$1" | sort
}

old_map=$(to_map "$OLD")
new_map=$(to_map "$NEW")

# Join on key. comm/awk approach.
old_passes=$(echo "$old_map" | awk '$2=="pass" {print $1}')
new_passes=$(echo "$new_map" | awk '$2=="pass" {print $1}')

newly_passing=$(comm -13 <(echo "$old_passes") <(echo "$new_passes"))
newly_failing=$(comm -23 <(echo "$old_passes") <(echo "$new_passes"))

n_new_pass=$(echo "$newly_passing" | grep -c '^.' || echo 0)
n_lost=$(echo "$newly_failing" | grep -c '^.' || echo 0)

old_total=$(wc -l < "$OLD")
new_total=$(wc -l < "$NEW")
old_pass=$(echo "$old_passes" | grep -c '^.' || echo 0)
new_pass=$(echo "$new_passes" | grep -c '^.' || echo 0)

echo "=== ERT compatibility diff ==="
echo "OLD ($OLD): $old_pass pass / $old_total total"
echo "NEW ($NEW): $new_pass pass / $new_total total"
echo
echo "Newly passing ($n_new_pass):"
echo "$newly_passing" | head -30
[ "$n_new_pass" -gt 30 ] && echo "  ... ($((n_new_pass - 30)) more)"
echo
echo "Newly failing/errored ($n_lost):"
echo "$newly_failing" | head -30
[ "$n_lost" -gt 30 ] && echo "  ... ($((n_lost - 30)) more)"
