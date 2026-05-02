#!/usr/bin/env bash
# Diff two emacs-test-results.jsonl runs.
# Reports tests that newly pass, newly fail, newly error, or disappeared,
# plus a per-file pass-rate summary table (markdown).
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

count_lines() { local n; n=$(echo "$1" | grep -c '^.' || true); echo "$n"; }

n_new_pass=$(count_lines "$newly_passing")
n_lost=$(count_lines "$newly_failing")

old_total=$(wc -l < "$OLD" | tr -d ' ')
new_total=$(wc -l < "$NEW" | tr -d ' ')
old_pass=$(count_lines "$old_passes")
new_pass=$(count_lines "$new_passes")

echo "=== ERT compatibility diff ==="
echo "OLD ($OLD): $old_pass pass / $old_total total"
echo "NEW ($NEW): $new_pass pass / $new_total total"
echo

# --- Per-file summary table (markdown) ---
echo "=== Per-file pass-rate table ==="
echo

# Collect all files from both runs.
old_files=$(jq -r '.file' "$OLD" | sort -u)
new_files=$(jq -r '.file' "$NEW" | sort -u)
all_files=$(printf '%s\n%s\n' "$old_files" "$new_files" | sort -u | grep '^.')

echo "| file | before pass | after pass | delta |"
echo "|------|------------|------------|-------|"

while IFS= read -r f; do
  bp=$(echo "$old_map" | awk -v f="$f" '$1 ~ "^"f"::" && $2=="pass" {n++} END{print n+0}')
  ap=$(echo "$new_map" | awk -v f="$f" '$1 ~ "^"f"::" && $2=="pass" {n++} END{print n+0}')
  d=$((ap - bp))
  if [ "$d" -gt 0 ]; then
    sign="+$d"
  elif [ "$d" -eq 0 ]; then
    sign="$d"
  else
    sign="$d"
  fi
  echo "| $f | $bp | $ap | $sign |"
done <<< "$all_files"

echo
echo "Newly passing ($n_new_pass):"
if [ "$n_new_pass" -gt 0 ]; then echo "$newly_passing" | head -30; fi
if [ "$n_new_pass" -gt 30 ]; then echo "  ... ($((n_new_pass - 30)) more)"; fi
echo
echo "Newly failing/errored ($n_lost):"
if [ "$n_lost" -gt 0 ]; then echo "$newly_failing" | head -30; fi
if [ "$n_lost" -gt 30 ]; then echo "  ... ($((n_lost - 30)) more)"; fi
