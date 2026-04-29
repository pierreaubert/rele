#!/usr/bin/env python3
"""Refresh the ERT baseline.

Builds the `emacs_test_worker`, runs it once per file, writes
`tmp/ert-baseline.jsonl`, and prints the same summary table as the old
shell driver.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import sys
from pathlib import Path


def repo_root(script_dir: Path) -> Path:
    return script_dir.parent.parent.parent


def tractable_files(script_dir: Path) -> list[str]:
    path = script_dir / "tractable.list"
    if not path.exists():
        print(f"tractable.list not found in {script_dir}", file=sys.stderr)
        raise SystemExit(1)
    files: list[str] = []
    for line in path.read_text().splitlines():
        stripped = line.strip()
        if stripped and not stripped.startswith("#"):
            files.append(stripped)
    return files


def build_worker(root: Path) -> Path:
    target_dir = Path(os.environ.get("CARGO_TARGET_DIR", root / "target"))
    if not target_dir.is_absolute():
        target_dir = root / target_dir
    worker = target_dir / "release" / "emacs_test_worker"
    cargo = os.environ.get("CARGO") or shutil.which("cargo")
    if cargo is None:
        rustup_cargo = Path.home() / ".cargo" / "bin" / "cargo"
        if rustup_cargo.exists():
            cargo = str(rustup_cargo)
    if cargo is None:
        print("build failed: cargo not found", file=sys.stderr)
        raise SystemExit(1)
    result = subprocess.run(
        [
            cargo,
            "build",
            "--release",
            "--manifest-path",
            str(root / "Cargo.toml"),
            "-p",
            "rele-elisp",
            "--bin",
            "emacs_test_worker",
        ],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    if result.returncode != 0:
        print("build failed", file=sys.stderr)
        raise SystemExit(1)
    return worker


def worker_output(worker: Path, file_name: str, per_test_ms: str, timeout_s: float) -> str:
    proc = subprocess.Popen(
        [str(worker), "--per-test-ms", per_test_ms],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    try:
        stdout, _ = proc.communicate(f"{file_name}\n", timeout=timeout_s)
    except subprocess.TimeoutExpired as exc:
        proc.kill()
        stdout, _ = proc.communicate()
        if exc.stdout:
            stdout = f"{exc.stdout}{stdout}"
    return "\n".join(
        line for line in stdout.splitlines() if line.strip() != "__DONE__"
    )


def main(argv: list[str]) -> int:
    script_dir = Path(__file__).resolve().parent
    root = repo_root(script_dir)
    worker = build_worker(root)
    baseline = root / "tmp" / "ert-baseline.jsonl"
    baseline.parent.mkdir(parents=True, exist_ok=True)

    per_test_ms = os.environ.get("PER_TEST_MS", "2000")
    per_file_timeout = float(os.environ.get("PER_FILE_TIMEOUT", "60"))
    files = argv if argv else tractable_files(script_dir)

    with baseline.open("w", encoding="utf-8") as out:
        for file_name in files:
            if not Path(file_name).is_file():
                print(f"skip (missing): {file_name}", file=sys.stderr)
                continue
            text = worker_output(worker, file_name, per_test_ms, per_file_timeout)
            if text:
                out.write(text)
                out.write("\n")
        out.write("all-done\n")

    return subprocess.run(
        ["python3", str(script_dir / "summarize.py"), str(baseline)],
        check=False,
    ).returncode


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
