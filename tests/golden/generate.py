#!/usr/bin/env python3
"""Write tests/golden/manifest.json from a built realism-gate binary.

This script is the provenance of the golden manifest: the manifest committed
in the workspace split (RG-A0) was produced by it from the pre-split binary at
commit 45ca25a, before any source file moved. It is not run by the test suite;
`tests/golden.rs` only reads the manifest.

The case list is derived from the committed fixture tree, so it does not depend
on which binary produced it:

  * every fixture directory: `check DIR --no-declaration` with `--format json`
    and `--format sarif`;
  * every source file under tests/fixtures: `check FILE --no-declaration
    --format json`;
  * the two declaration fixtures with their declaration file;
  * the three inline-allow fixtures with `--trust-inline-allows`;
  * the catalog commands (`rules`, `rules --format json`, `explain`).

Only stdout bytes and the exit code are compared. stderr is compared only when
it contains no absolute path (a refused in-repo declaration names the checkout
directory, so that case records stdout and the exit code only). If stdout ever
contains an absolute path the script stops: the JSON contract is
repository-relative, and a machine-specific path cannot be a golden value.

Usage:
    python tests/golden/generate.py --binary target/debug/realism-gate.exe
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "tests" / "fixtures"
SOURCE_SUFFIXES = (".sol", ".rs", ".ts", ".move")


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fixture_dirs() -> list[Path]:
    dirs = sorted({p.parent for p in FIXTURES.rglob("*") if p.is_file()})
    # Keep only directories that look like fixtures (contain a source file at
    # any depth); a directory of nothing but YAML or JSON is not a fixture.
    out = []
    for d in dirs:
        if any(f.suffix in SOURCE_SUFFIXES for f in d.rglob("*") if f.is_file()):
            out.append(d)
    return sorted(out, key=lambda p: p.as_posix())


def source_files() -> list[Path]:
    return sorted(
        (p for p in FIXTURES.rglob("*") if p.is_file() and p.suffix in SOURCE_SUFFIXES),
        key=lambda p: p.as_posix(),
    )


def cases() -> list[list[str]]:
    out: list[list[str]] = []
    decl = "tests/fixtures/declaration/realism.yaml"
    for d in fixture_dirs():
        rel = d.relative_to(ROOT).as_posix()
        for fmt in ("json", "sarif"):
            out.append(["check", rel, "--no-declaration", "--format", fmt])
        if rel in ("tests/fixtures/declaration", "tests/fixtures/rust_declaration"):
            for fmt in ("json", "sarif"):
                out.append(["check", rel, "--declaration", decl, "--format", fmt])
    for rel in (
        "tests/fixtures/suppress",
        "tests/fixtures/rust_suppress",
        "tests/fixtures/ts_suppress",
    ):
        for fmt in ("json", "sarif"):
            out.append(
                [
                    "check",
                    rel,
                    "--no-declaration",
                    "--trust-inline-allows",
                    "--format",
                    fmt,
                ]
            )
    for f in source_files():
        rel = f.relative_to(ROOT).as_posix()
        out.append(["check", rel, "--no-declaration", "--format", "json"])
    out.append(["rules"])
    out.append(["rules", "--format", "json"])
    out.append(["explain", "RG06"])
    out.append(["explain", "RG06", "--markdown"])
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", required=True, help="the realism-gate binary to run")
    parser.add_argument(
        "--commit",
        default="",
        help="the commit the binary was built from (recorded in the manifest)",
    )
    args = parser.parse_args()

    binary = Path(args.binary).resolve()
    binary_sha = sha256(binary.read_bytes())
    env = dict(os.environ)
    env["NO_COLOR"] = "1"

    root_str = str(ROOT)
    # The tool prints paths with forward slashes on every platform, including
    # Windows; check both spellings so an absolute path cannot slip through
    # the stderr comparison and poison the committed golden values.
    root_markers = (root_str.encode(), root_str.replace("\\", "/").encode())
    recorded = []
    for argv in cases():
        proc = subprocess.run(
            [str(binary), *argv],
            cwd=ROOT,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        if any(m in proc.stdout for m in root_markers):
            print(
                f"ABSOLUTE PATH IN STDOUT: {argv}; refusing to normalise it",
                file=sys.stderr,
            )
            return 2
        record = {
            "argv": argv,
            "exit": proc.returncode,
            "stdout_sha256": sha256(proc.stdout),
            "stdout_bytes": len(proc.stdout),
            "stderr_sha256": sha256(proc.stderr),
            "stderr_bytes": len(proc.stderr),
            "compare_stderr": not any(m in proc.stderr for m in root_markers),
        }
        recorded.append(record)

    manifest = {
        "description": (
            "Golden output of the realism-gate CLI over the committed fixture "
            "tree. Produced by tests/golden/generate.py from the pre-split "
            "binary; tests/golden.rs re-runs every case and compares stdout "
            "bytes and exit codes."
        ),
        "generator": "tests/golden/generate.py",
        "generated_from_commit": args.commit,
        "binary_sha256": binary_sha,
        "tool_version": "1.4.0",
        "case_count": len(recorded),
        "cases": recorded,
    }
    path = Path(__file__).resolve().parent / "manifest.json"
    path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    print(f"wrote {path} with {len(recorded)} cases (binary sha256 {binary_sha})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
