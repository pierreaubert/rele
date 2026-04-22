#!/usr/bin/env python3
"""Triage Emacs ERT test results from the rele-elisp test harness.

Reads the JSONL output from `test_emacs_all_files_run` and produces:
  1. Per-file summary (pass/fail/error/skip/timeout)
  2. Error classification (void-function, wrong-type, timeout, etc.)
  3. Missing primitive ranking (which void-functions block the most tests)
  4. Priority list: files sorted by tests-unlockable-if-we-fix-the-blocker

Usage:
  python3 scripts/triage_ert.py [path/to/emacs-test-results.jsonl]

Default path: crates/elisp/target/emacs-test-results.jsonl
"""

import json
import sys
import re
from collections import Counter, defaultdict
from pathlib import Path


def load_results(path: str) -> list[dict]:
    results = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                results.append(json.loads(line))
            except json.JSONDecodeError:
                pass
    return results


def classify_error(detail: str) -> tuple[str, str]:
    """Return (category, specifics) for an error detail string."""
    if m := re.match(r"void function: (.+)", detail):
        return ("void-function", m.group(1))
    if m := re.match(r"void variable: (.+)", detail):
        return ("void-variable", m.group(1))
    if m := re.match(r"wrong type argument: (.+)", detail):
        return ("wrong-type-argument", m.group(1))
    if m := re.match(r"wrong number of arguments", detail):
        return ("wrong-number-of-arguments", "")
    if "needs eval dispatch" in detail:
        return ("missing-dispatch", detail)
    if m := re.match(r"signal (.+)", detail):
        return ("signal", m.group(1))
    if detail == "":
        return ("unknown", "")
    return ("other", detail)


def shorten_file(path: str) -> str:
    """Shorten absolute paths to relative test/... form."""
    for prefix in ["/tmp/emacs-src/", "/Volumes/home_ext1/Src/emacs/"]:
        if path.startswith(prefix):
            return path[len(prefix):]
    return path


def main():
    jsonl_path = sys.argv[1] if len(sys.argv) > 1 else "crates/elisp/target/emacs-test-results.jsonl"
    if not Path(jsonl_path).exists():
        print(f"Error: {jsonl_path} not found", file=sys.stderr)
        print("Run: cargo test -p rele-elisp --release -- --ignored test_emacs_all_files_run --nocapture", file=sys.stderr)
        sys.exit(1)

    results = load_results(jsonl_path)
    if not results:
        print("No results found", file=sys.stderr)
        sys.exit(1)

    # --- Aggregate stats ---
    result_counts = Counter(r["result"] for r in results)
    print("=" * 60)
    print("OVERALL RESULT DISTRIBUTION")
    print("=" * 60)
    total = len(results)
    for status, count in sorted(result_counts.items(), key=lambda x: -x[1]):
        pct = count * 100 / total
        print(f"  {status:12s}: {count:5d} ({pct:5.1f}%)")
    print(f"  {'TOTAL':12s}: {total:5d}")
    print()

    # --- Per-file analysis ---
    files: dict[str, list[dict]] = defaultdict(list)
    for r in results:
        files[shorten_file(r["file"])].append(r)

    file_level_timeouts = []
    file_level_crashes = []
    files_with_tests = {}

    for fname, file_results in sorted(files.items()):
        if len(file_results) == 1 and file_results[0]["test"] == "<file>":
            r = file_results[0]
            if r["result"] == "timeout":
                file_level_timeouts.append(fname)
            elif r["result"] == "crash":
                file_level_crashes.append(fname)
            else:
                files_with_tests[fname] = file_results
        else:
            files_with_tests[fname] = file_results

    print("=" * 60)
    print("FILE-LEVEL OUTCOMES")
    print("=" * 60)
    print(f"  Files with individual test results: {len(files_with_tests)}")
    print(f"  Files that timed out (no tests ran): {len(file_level_timeouts)}")
    print(f"  Files that crashed:                  {len(file_level_crashes)}")
    print()

    # --- Error classification ---
    errors = [r for r in results if r["result"] == "error"]
    categories: dict[str, list[tuple[str, str]]] = defaultdict(list)
    for r in errors:
        cat, spec = classify_error(r["detail"])
        categories[cat].append((spec, shorten_file(r["file"])))

    print("=" * 60)
    print("ERROR CLASSIFICATION")
    print("=" * 60)
    for cat, items in sorted(categories.items(), key=lambda x: -len(x[1])):
        print(f"\n  {cat} ({len(items)} occurrences):")
        specifics = Counter(spec for spec, _ in items)
        for spec, count in specifics.most_common(15):
            print(f"    [{count:3d}] {spec}")
    print()

    # --- Missing primitive ranking ---
    void_funcs = Counter()
    void_func_files: dict[str, set[str]] = defaultdict(set)
    for r in results:
        if r["result"] == "error":
            cat, spec = classify_error(r["detail"])
            if cat == "void-function" and spec != "nil":
                void_funcs[spec] += 1
                void_func_files[spec].add(shorten_file(r["file"]))

    if void_funcs:
        print("=" * 60)
        print("MISSING PRIMITIVES (by test impact)")
        print("=" * 60)
        for func, count in void_funcs.most_common(30):
            n_files = len(void_func_files[func])
            print(f"  {func:40s}: {count:3d} tests, {n_files:2d} files")
        print()

    # --- Files with test results: pass/fail breakdown ---
    print("=" * 60)
    print("FILES WITH INDIVIDUAL TEST RESULTS")
    print("=" * 60)
    for fname, file_results in sorted(files_with_tests.items()):
        test_results = [r for r in file_results if r["test"] != "<file>"]
        if not test_results:
            continue
        counts = Counter(r["result"] for r in test_results)
        parts = []
        for status in ["pass", "fail", "error", "skip", "timeout", "panic"]:
            if counts.get(status, 0) > 0:
                parts.append(f"{counts[status]} {status}")
        total_tests = len(test_results)
        pass_rate = counts.get("pass", 0) * 100 / total_tests if total_tests > 0 else 0
        print(f"  {fname}")
        print(f"    {', '.join(parts)} (of {total_tests}, {pass_rate:.0f}% pass)")

        # Show first few failures
        failures = [r for r in test_results if r["result"] in ("fail", "error")]
        for r in failures[:3]:
            print(f"      {r['result']:5s} {r['test']}: {r['detail'][:70]}")
        if len(failures) > 3:
            print(f"      ... and {len(failures) - 3} more")
    print()

    # --- Timed-out files (sorted by probable test count) ---
    print("=" * 60)
    print(f"FILE-LEVEL TIMEOUTS ({len(file_level_timeouts)} files)")
    print("=" * 60)
    # Group by directory
    by_dir: dict[str, list[str]] = defaultdict(list)
    for f in file_level_timeouts:
        parts = f.split("/")
        if len(parts) >= 2:
            d = "/".join(parts[:2])
        else:
            d = "other"
        by_dir[d].append(f)
    for d, flist in sorted(by_dir.items()):
        print(f"\n  {d}/ ({len(flist)} files):")
        for f in sorted(flist)[:10]:
            print(f"    {f}")
        if len(flist) > 10:
            print(f"    ... and {len(flist) - 10} more")


if __name__ == "__main__":
    main()
