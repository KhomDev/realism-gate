#!/usr/bin/env python3
"""The pinned corpora, and the hashes that make them checkable.

Each corpus in eval/corpus.json is pinned by commit and by the sha256 of
`git ls-tree -r <commit> -- <path>`. That listing names every file with its
blob hash, so the sha256 covers the content of every file, and it does not
depend on the line-ending settings of the checkout.

A corpus with `only_new_vs` counts only the .sol files that do not exist in
the named corpus: PoCs added after the rules were written.

Usage:
  corpus.py --field NAME.FIELD     print one field, e.g. later.commit
  corpus.py --names                print the corpus names
  corpus.py --files NAME           print the .sol files that count
  corpus.py --check                verify every checkout against the pins
  corpus.py --write                recompute the hashes (a deliberate change)
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
PINS = ROOT / "eval" / "corpus.json"


def pins() -> dict:
    return json.loads(PINS.read_text(encoding="utf-8"))


def directory(name: str) -> Path:
    return ROOT / pins()[name]["dir"]


def listing(name: str) -> str:
    c = pins()[name]
    return subprocess.run(
        ["git", "-C", str(ROOT / c["dir"]), "ls-tree", "-r", c["commit"], "--", c["path"]],
        check=True,
        capture_output=True,
    ).stdout.decode("utf-8")


def sol_files_in(text: str) -> list[str]:
    files = []
    for line in text.splitlines():
        path = line.split("\t", 1)[1]
        if path.endswith(".sol"):
            files.append(path)
    return sorted(files)


def files(name: str) -> list[str]:
    """The .sol files of a corpus that count."""
    mine = sol_files_in(listing(name))
    base = pins()[name].get("only_new_vs")
    if base is None:
        return mine
    old = set(sol_files_in(listing(base)))
    return [f for f in mine if f not in old]


def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def measure(name: str) -> dict:
    text = listing(name)
    got = {"listing_sha256": sha256(text), "sol_files": len(sol_files_in(text))}
    if pins()[name].get("only_new_vs"):
        new = files(name)
        got["new_sol_files"] = len(new)
        got["new_files_sha256"] = sha256("\n".join(new) + "\n")
    return got


def main(argv: list[str]) -> int:
    p = pins()
    if len(argv) == 3 and argv[1] == "--field":
        value = p
        for part in argv[2].split("."):
            value = value[part]
        print(value)
        return 0
    if argv[1:] == ["--names"]:
        print("\n".join(p))
        return 0
    if len(argv) == 3 and argv[1] == "--files":
        print("\n".join(files(argv[2])))
        return 0
    if argv[1:] == ["--write"]:
        for name in p:
            p[name].update(measure(name))
        PINS.write_text(json.dumps(p, indent=2) + "\n", encoding="utf-8", newline="\n")
        print(f"wrote {PINS.relative_to(ROOT).as_posix()}")
        return 0
    if argv[1:] == ["--check"]:
        bad = []
        for name in p:
            for field, value in measure(name).items():
                if p[name].get(field) != value:
                    bad.append(f"{name}.{field}: pinned {p[name].get(field)!r}, found {value!r}")
        for b in bad:
            print(f"MISMATCH {b}", file=sys.stderr)
        if bad:
            return 1
        counts = ", ".join(f"{n} {len(files(n))} files" for n in p)
        print(f"corpora match eval/corpus.json ({counts})")
        return 0
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv))
