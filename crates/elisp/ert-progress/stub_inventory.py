#!/usr/bin/env python3
"""Write an inventory of silent/stubbed Elisp primitives.

The report is intentionally source-derived rather than hand-curated.
It gives the ERT work a complete backlog of bootstrap aliases and
core stub-module primitives before individual tests reveal them.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class StubRecord:
    bucket: str
    status: str
    name: str
    dispatch: str
    target: str
    file: str
    line: int
    note: str


def repo_root(script_dir: Path) -> Path:
    return script_dir.parent.parent.parent


def line_number(text: str, offset: int) -> int:
    return text.count("\n", 0, offset) + 1


def rel(path: Path, root: Path) -> str:
    return str(path.relative_to(root))


def string_literals(text: str) -> list[str]:
    return re.findall(r'"((?:\\.|[^"\\])*)"', text)


BUCKETS = {
    "editing/regions",
    "category/case-tables",
    "window/display",
    "keymap/help",
    "other",
}

EDITING_EXACT = {
    "delete-and-extract-region",
    "field-beginning",
    "field-string-no-properties",
    "gap-position",
    "gap-size",
    "insert-and-inherit",
    "insert-byte",
    "minibuffer-prompt-end",
    "next-single-char-property-change",
    "position-bytes",
    "transpose-regions",
    "upcase-initials-region",
}

EDITING_PARTS = (
    "buffer-substring",
    "char-property",
    "delete-region",
    "insert-buffer-substring",
    "marker",
    "narrow-to-region",
    "point",
    "pos-property",
    "property-change",
    "text-properties",
    "text-property",
)

CATEGORY_CASE_PARTS = (
    "case-syntax",
    "case-table",
    "category",
    "char-table",
    "charset",
    "syntax-table",
)

CATEGORY_CASE_EXACT = {
    "char-syntax",
    "copy-syntax-table",
    "modify-syntax-entry",
    "set-syntax-table",
    "standard-syntax-table",
    "string-to-syntax",
    "syntax-after",
    "syntax-class-to-char",
    "syntax-table-p",
}

WINDOW_DISPLAY_PARTS = (
    "bidi-",
    "color-",
    "display-",
    "face",
    "font-",
    "frame",
    "fringe",
    "image",
    "mode-line",
    "pixel",
    "redisplay",
    "terminal",
    "tty-",
    "window",
    "x-display",
)

WINDOW_DISPLAY_EXACT = {
    "force-mode-line-update",
}

KEYMAP_HELP_PARTS = (
    "command-key",
    "describe-",
    "documentation",
    "help",
    "kbd",
    "key-",
    "keymap",
    "keyboard",
    "lookup-key",
    "substitute-command-keys",
    "where-is",
)


def bucket_for(name: str) -> str:
    if name in EDITING_EXACT or any(part in name for part in EDITING_PARTS):
        return "editing/regions"
    if name in CATEGORY_CASE_EXACT or any(part in name for part in CATEGORY_CASE_PARTS):
        return "category/case-tables"
    if name in WINDOW_DISPLAY_EXACT or any(part in name for part in WINDOW_DISPLAY_PARTS):
        return "window/display"
    if any(part in name for part in KEYMAP_HELP_PARTS):
        return "keymap/help"
    return "other"


def make_record(
    *,
    status: str,
    name: str,
    dispatch: str,
    target: str,
    file: str,
    line: int,
    note: str,
) -> StubRecord:
    bucket = bucket_for(name)
    assert bucket in BUCKETS
    return StubRecord(
        bucket=bucket,
        status=status,
        name=name,
        dispatch=dispatch,
        target=target,
        file=file,
        line=line,
        note=note,
    )


def add_bootstrap_aliases(root: Path, records: list[StubRecord]) -> None:
    bootstrap_dir = root / "crates/elisp/src/eval/bootstrap"
    pattern = re.compile(
        r'interp\.define\(\s*"([^"]+)"\s*,\s*'
        r'LispObject::primitive\(\s*"([^"]+)"\s*\)\s*\)',
        re.DOTALL,
    )
    for path in sorted(bootstrap_dir.glob("*.rs")):
        text = path.read_text(encoding="utf-8")
        for match in pattern.finditer(text):
            name, target = match.groups()
            if target not in {"ignore", "identity"}:
                continue
            records.append(
                make_record(
                    status="runtime-missing"
                    if target == "ignore"
                    else "compat-identity",
                    name=name,
                    dispatch="bootstrap-alias",
                    target=target,
                    file=rel(path, root),
                    line=line_number(text, match.start()),
                    note="silent primitive alias",
                )
            )


def extract_array_after(text: str, marker: str) -> tuple[int, str] | None:
    marker_offset = text.find(marker)
    if marker_offset < 0:
        return None
    start = text.find("[", marker_offset)
    end_candidates = [idx for idx in (text.find("];", start), text.find("] {", start)) if idx >= 0]
    if start < 0 or not end_candidates:
        return None
    end = min(end_candidates)
    return start, text[start : end + 1]


def add_core_aliases(root: Path, records: list[StubRecord]) -> None:
    path = root / "crates/elisp/src/primitives/core.rs"
    text = path.read_text(encoding="utf-8")
    alias_array = extract_array_after(text, "let alias_to_ignore = [")
    if alias_array is not None:
        offset, block = alias_array
        for name in string_literals(block):
            records.append(
                    make_record(
                        status="runtime-missing",
                        name=name,
                    dispatch="core-alias",
                    target="ignore",
                    file=rel(path, root),
                    line=line_number(text, offset + block.find(f'"{name}"')),
                    note="registered as primitive(\"ignore\")",
                )
            )

    phase_array = extract_array_after(text, "// Phase-1 C-level primitive stubs")
    if phase_array is not None:
        offset, block = phase_array
        for name in string_literals(block):
            records.append(
                    make_record(
                        status="needs-classification",
                        name=name,
                    dispatch="phase1-registration",
                    target="stubs::call",
                    file=rel(path, root),
                    line=line_number(text, offset + block.find(f'"{name}"')),
                    note="registered through Phase-1 stub list",
                )
            )


def classify_stub_behavior(rhs: str) -> str:
    if "Ok(LispObject::nil())" in rhs:
        return "nil"
    if "Err(" in rhs:
        return "signals"
    if "args.first()" in rhs or "args.nth(" in rhs:
        return "argument-derived"
    if "LispObject::integer" in rhs:
        return "constant-integer"
    if "LispObject::string" in rhs:
        return "constant-string"
    if "LispObject::symbol" in rhs:
        return "constant-symbol"
    return "helper-or-compound"


def add_stub_module_matches(root: Path, records: list[StubRecord]) -> None:
    path = root / "crates/elisp/src/primitives/core/stubs.rs"
    lines = path.read_text(encoding="utf-8").splitlines()
    pending: list[str] = []
    pending_start = 0
    for idx, line in enumerate(lines, start=1):
        stripped = line.strip()
        if stripped.startswith("//") or not (pending or '"' in stripped):
            continue
        if not pending:
            pending_start = idx
        pending.append(stripped)
        if "=>" not in stripped:
            continue
        arm = " ".join(pending)
        pending = []
        lhs, rhs = arm.split("=>", 1)
        names = string_literals(lhs)
        if not names:
            continue
        behavior = classify_stub_behavior(rhs)
        for name in names:
            records.append(
                make_record(
                    status="needs-classification",
                    name=name,
                    dispatch="stub-module",
                    target=behavior,
                    file=rel(path, root),
                    line=pending_start,
                    note="matched in stubs::call",
                )
            )


def write_tsv(path: Path, records: list[StubRecord]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8") as out:
        out.write("bucket\tstatus\tname\tdispatch\ttarget\tfile\tline\tnote\n")
        for r in sorted(
            records,
            key=lambda item: (item.bucket, item.status, item.name, item.dispatch),
        ):
            out.write(
                f"{r.bucket}\t{r.status}\t{r.name}\t{r.dispatch}\t"
                f"{r.target}\t{r.file}\t{r.line}\t{r.note}\n"
            )


def record_key(record: StubRecord) -> tuple[str, str, str, str]:
    return (record.name, record.dispatch, record.target, record.status)


def read_baseline_keys(path: Path) -> set[tuple[str, str, str, str]]:
    keys: set[tuple[str, str, str, str]] = set()
    with path.open(encoding="utf-8") as f:
        header = f.readline().rstrip("\n").split("\t")
        try:
            name_i = header.index("name")
            dispatch_i = header.index("dispatch")
            target_i = header.index("target")
            status_i = header.index("status")
        except ValueError as exc:
            raise SystemExit(f"{path}: invalid inventory baseline header") from exc
        for line in f:
            parts = line.rstrip("\n").split("\t")
            if len(parts) <= max(name_i, dispatch_i, target_i, status_i):
                continue
            keys.add(
                (
                    parts[name_i],
                    parts[dispatch_i],
                    parts[target_i],
                    parts[status_i],
                )
            )
    return keys


def check_against_baseline(records: list[StubRecord], baseline: Path) -> int:
    if not baseline.exists():
        print(f"stub inventory baseline missing: {baseline}", file=sys.stderr)
        return 2
    baseline_keys = read_baseline_keys(baseline)
    current_by_key = {record_key(record): record for record in records}
    new_keys = sorted(set(current_by_key) - baseline_keys)
    if not new_keys:
        removed = len(baseline_keys - set(current_by_key))
        if removed:
            print(f"stub inventory gate passed; {removed} baseline stubs removed")
        else:
            print("stub inventory gate passed")
        return 0
    print("stub inventory gate failed: new silent/runtime stubs were added", file=sys.stderr)
    for key in new_keys[:50]:
        record = current_by_key[key]
        print(
            f"  {record.bucket}\t{record.status}\t{record.name}\t"
            f"{record.dispatch}\t{record.target}\t{record.file}:{record.line}",
            file=sys.stderr,
        )
    if len(new_keys) > 50:
        print(f"  ... and {len(new_keys) - 50} more", file=sys.stderr)
    return 1


def main(argv: list[str]) -> int:
    script_dir = Path(__file__).resolve().parent
    root = repo_root(script_dir)
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=root / "tmp/elisp-stub-inventory.tsv",
        help="TSV output path",
    )
    parser.add_argument(
        "--baseline",
        type=Path,
        default=script_dir / "stub_inventory_baseline.tsv",
        help="baseline TSV used by --check",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="fail if the current source adds stubs not present in the baseline",
    )
    parser.add_argument("--quiet", action="store_true")
    args = parser.parse_args(argv)

    output = args.output
    if not output.is_absolute():
        output = root / output

    records: list[StubRecord] = []
    add_bootstrap_aliases(root, records)
    add_core_aliases(root, records)
    add_stub_module_matches(root, records)
    if args.check:
        baseline = args.baseline
        if not baseline.is_absolute():
            baseline = root / baseline
        return check_against_baseline(records, baseline)
    write_tsv(output, records)

    if not args.quiet:
        by_status: dict[str, int] = {}
        by_bucket: dict[str, int] = {}
        for record in records:
            by_status[record.status] = by_status.get(record.status, 0) + 1
            by_bucket[record.bucket] = by_bucket.get(record.bucket, 0) + 1
        summary = ", ".join(
            f"{status}={count}" for status, count in sorted(by_status.items())
        )
        bucket_summary = ", ".join(
            f"{bucket}={count}" for bucket, count in sorted(by_bucket.items())
        )
        print(f"wrote {len(records)} records to {output}")
        print(summary)
        print(bucket_summary)
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
