#!/usr/bin/env python3
"""Repository hygiene checks for realism-gate. Standard library only.

  A. HOST PATHS        No tracked file may contain an absolute home-directory
                       path (C:/Users/<name>, /Users/<name>, /home/<name>).
                       Fixtures, docs and examples are read by strangers.

  B. PRIVATE STRINGS   No tracked file may contain a private-workspace marker.
                       The markers are assembled from fragments below so this
                       file does not contain the literals it forbids.

  C. NO TARGETS        Fixtures, examples and rule docs name no bug-bounty
                       platform. They are synthetic by design.

  D. ENCODING          No tracked text file may carry a mojibake sequence (a
                       UTF-8 character re-saved through cp1252).

Exit code 0 = clean, 1 = at least one violation, 2 = the check could not run.
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

HOST_PATH = re.compile(
    r"(?i)(?:\b[a-z]:[\\/]+users[\\/]+[^\\/\s\"'`<>]+)|(?:/users/[a-z0-9._-]+/)|(?:/home/[a-z0-9._-]+/)"
)

# Placeholders that document the pattern rather than leak a path.
HOST_PATH_ALLOWED = ("<name>", "<user>", "runner/")


def private_markers() -> list[str]:
    return [
        "KNOW" + "LEDGE/",
        "_private" + "-tools",
        "RESULTS" + "/_REJECTION",
        "submission" + "_guides/",
        "engage" + "ments/",
        "audit" + "mem",
        "deepseek" + "-harness",
        "d3f" + "al",
    ]


PLATFORMS = re.compile(
    r"(?i)\b(hacken" + r"proof|imm" + r"unefi|can" + r"tina\.xyz|sher" + r"lock\.xyz|code" + r"4rena)\b"
)
NO_TARGET_DIRS = ("tests/fixtures/", "examples/", "docs/rules/")

MOJIBAKE = re.compile("\u00e2\u20ac|\u00c3\u00a9|\u00c3\u00a8|\u00e2\u2020\u2019|\u00c2\u00b7")

BINARY_SUFFIXES = {".png", ".jpg", ".jpeg", ".gif", ".ico", ".zip", ".gz"}


def tracked_files() -> list[Path]:
    try:
        out = subprocess.run(
            ["git", "ls-files", "-z"], cwd=ROOT, check=True, capture_output=True
        ).stdout
    except (OSError, subprocess.CalledProcessError) as exc:
        print(f"cannot list tracked files: {exc}", file=sys.stderr)
        sys.exit(2)
    return [ROOT / p for p in out.decode("utf-8").split("\0") if p]


def main() -> int:
    problems: list[str] = []
    markers = private_markers()
    for path in tracked_files():
        rel = path.relative_to(ROOT).as_posix()
        if path.suffix.lower() in BINARY_SUFFIXES or not path.is_file():
            continue
        if rel == "scripts/check_hygiene.py":
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        for n, line in enumerate(text.splitlines(), 1):
            for m in HOST_PATH.finditer(line):
                if not any(a in m.group(0) for a in HOST_PATH_ALLOWED):
                    problems.append(f"{rel}:{n}: host path `{m.group(0)}`")
            for marker in markers:
                if marker in line:
                    problems.append(f"{rel}:{n}: private-workspace marker")
            if rel.startswith(NO_TARGET_DIRS) and PLATFORMS.search(line):
                problems.append(f"{rel}:{n}: names a platform")
            if MOJIBAKE.search(line):
                problems.append(f"{rel}:{n}: mojibake")
    for p in problems:
        print(f"FAIL  {p}")
    if problems:
        return 1
    print("ok    no host paths, private markers, platform names or mojibake")
    return 0


if __name__ == "__main__":
    sys.exit(main())
