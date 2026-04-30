#!/usr/bin/env python3
"""Summarise an ERT baseline JSONL file.

Reads the JSONL produced by emacs_test_worker (one record per ERT result,
plus __SUMMARY__ marker lines) and prints two tables to stdout:

  1. Per-file pass/fail/error/skip counts and pass-rate.
  2. Top failure patterns ranked by how many tests share the bucket
     (the leverage targets — fix once, unblock many).

Usage:
  python3 summarize.py <baseline.jsonl>
"""

import json
import sys
from collections import Counter, defaultdict
from pathlib import Path


def categorise(detail: str) -> str:
    """Bucket a failure detail string into a coarse pattern."""
    if not detail:
        return "EMPTY_DETAIL"
    if detail.startswith('("did not signal")'):
        return 'DID_NOT_SIGNAL'
    if detail.startswith('void function:'):
        return f'VOID_FN: {detail.replace("void function:", "").strip()}'
    if detail.startswith('void variable:'):
        return f'VOID_VAR: {detail.replace("void variable:", "").strip()}'
    if 'wrong type argument: expected number' in detail:
        return 'WRONG_TYPE_NUMBER'
    if 'wrong type argument: expected integer' in detail:
        return 'WRONG_TYPE_INTEGER'
    if 'wrong type argument: expected string' in detail:
        return 'WRONG_TYPE_STRING'
    if 'wrong type argument: expected character' in detail:
        return 'WRONG_TYPE_CHARACTER'
    if 'wrong type argument: expected marker' in detail:
        return 'WRONG_TYPE_MARKER'
    if 'wrong number of arguments' in detail:
        return 'WRONG_N_ARGS'
    if 'reader error' in detail:
        return f'READER: {detail[:60]}'
    if detail.startswith('signal '):
        return f'SIGNAL: {detail[:60]}'
    # For asserts, keep only the leading shape.
    return f'ASSERT: {detail[:80]}'


def parse_stub_hits(field: str) -> list[tuple[str, int]]:
    hits = []
    for item in field.split(';'):
        if not item:
            continue
        name, sep, count_s = item.rpartition('=')
        if not sep:
            continue
        try:
            count = int(count_s)
        except ValueError:
            continue
        hits.append((name, count))
    return hits


def load_stub_buckets() -> dict[str, str]:
    script_dir = Path(__file__).resolve().parent
    root = script_dir.parent.parent.parent
    inventory = root / "tmp" / "elisp-stub-inventory.tsv"
    buckets: dict[str, str] = {}
    if not inventory.exists():
        return buckets
    with inventory.open(encoding="utf-8") as f:
        header = f.readline().rstrip('\n').split('\t')
        try:
            bucket_i = header.index('bucket')
            name_i = header.index('name')
        except ValueError:
            return buckets
        for line in f:
            parts = line.rstrip('\n').split('\t')
            if len(parts) <= max(bucket_i, name_i):
                continue
            buckets.setdefault(parts[name_i], parts[bucket_i])
    return buckets


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__)
        return 2
    path = sys.argv[1]
    stub_buckets = load_stub_buckets()

    by_file = defaultdict(Counter)
    patterns = Counter()
    pattern_examples = {}
    stub_hits = Counter()
    stub_tests = defaultdict(set)
    stub_bad_tests = defaultdict(set)
    stub_examples = {}
    total = Counter()

    with open(path) as f:
        for line in f:
            line = line.strip()
            if not line.startswith('{'):
                continue
            try:
                r = json.loads(line)
            except json.JSONDecodeError:
                continue
            fname = r.get('file', '').split('/')[-1]
            res = r.get('result', '?')
            by_file[fname][res] += 1
            total[res] += 1
            if res in ('fail', 'error'):
                bucket = categorise(r.get('detail', ''))
                patterns[bucket] += 1
                pattern_examples.setdefault(
                    bucket,
                    f"{fname}::{r.get('test', '')}",
                )
            test_key = f"{fname}::{r.get('test', '')}"
            for stub, count in parse_stub_hits(r.get('stubs', '')):
                stub_hits[stub] += count
                stub_tests[stub].add(test_key)
                stub_examples.setdefault(stub, test_key)
                if res in ('fail', 'error', 'panic', 'timeout'):
                    stub_bad_tests[stub].add(test_key)

    print("=" * 72)
    print("PER-FILE RESULTS")
    print("=" * 72)
    print(f"{'file':<28} {'pass':>5} {'fail':>5} {'err':>5} {'skip':>5} {'pct':>5}")
    print('-' * 72)

    sorted_files = sorted(by_file.items())
    for fname, cnt in sorted_files:
        p = cnt.get('pass', 0)
        f_ = cnt.get('fail', 0)
        e = cnt.get('error', 0)
        s = cnt.get('skip', 0)
        n = p + f_ + e + s
        pct = 100 * p / n if n else 0
        print(f"{fname:<28} {p:>5} {f_:>5} {e:>5} {s:>5} {pct:>4.0f}%")

    n = sum(total.values())
    pct = 100 * total['pass'] / n if n else 0
    print('-' * 72)
    print(
        f"{'TOTAL':<28} {total['pass']:>5} {total['fail']:>5} "
        f"{total['error']:>5} {total['skip']:>5} {pct:>4.0f}%",
    )

    print()
    print("=" * 72)
    print("TOP FAILURE PATTERNS (leverage targets — fix once, unblock many)")
    print("=" * 72)
    for bucket, count in patterns.most_common(20):
        example = pattern_examples.get(bucket, '')
        print(f"  {count:>4}  {bucket}")
        print(f"        e.g. {example}")

    print()
    print("=" * 72)
    print("TOP RUNTIME STUB HITS (ranked by failing/erroring tests)")
    print("=" * 72)
    ranked_stubs = sorted(
        stub_hits,
        key=lambda stub: (
            len(stub_bad_tests[stub]),
            len(stub_tests[stub]),
            stub_hits[stub],
            stub,
        ),
        reverse=True,
    )
    if not ranked_stubs:
        print("  none recorded")
    for stub in ranked_stubs[:20]:
        bucket_name = stub.split('->', 1)[0]
        bucket = stub_buckets.get(bucket_name, 'unknown')
        print(
            f"  {stub_hits[stub]:>4} hits  "
            f"{len(stub_bad_tests[stub]):>3} bad-tests  "
            f"{len(stub_tests[stub]):>3} tests  [{bucket}] {stub}"
        )
        print(f"        e.g. {stub_examples.get(stub, '')}")

    return 0


if __name__ == '__main__':
    sys.exit(main())
