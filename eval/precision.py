#!/usr/bin/env python3
"""Per-rule precision of two realism-gate versions on the labelled flags of
every pinned corpus, with Wilson 95% intervals. Standard library only.

Inputs, all committed:
  eval/flags/<version>-<corpus>.jsonl   every flag a version raises on a corpus
  eval/labels/v0.2.0.jsonl              one label per labelled flag id

What counts as labelled for a version on a corpus is every flag of that
version whose id has a label. The script refuses to run if a rule raised 30
flags or fewer on a corpus and any of them is unlabelled: those rules are
labelled in full, so a gap means a label is missing, not a sampling choice.

precision = TP / (TP + FP). `unclear` labels are left out of the ratio and
reported next to it; `precision_if_unclear_wrong` counts them as false
positives, which is the pessimistic bound.

Ruling R3: a rule whose v0.2.0 precision is below 0.80 on any corpus where it
has a TP or FP label may not ship at severity error. With --rules RULES.json
(the output of `realism-gate rules --format json` for v0.2.0) the script
fails if one does.

Usage:
  precision.py OUT.json OUT.md [--rules RULES.json]
"""

from __future__ import annotations

import json
import math
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
EVAL = ROOT / "eval"
sys.path.insert(0, str(EVAL))
import corpus  # noqa: E402

VERSIONS = ("v0.1.0", "v0.2.0")
CAP = 30
FLOOR = 0.80
Z = 1.959964


def wilson(k: int, n: int) -> tuple[float, float] | None:
    if n == 0:
        return None
    p = k / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return (round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4))


def load(path: Path) -> list[dict]:
    with open(path, encoding="utf-8") as fh:
        return [json.loads(line) for line in fh if line.strip()]


def tally(labels: list[str]) -> dict:
    tp = labels.count("true-positive")
    fp = labels.count("false-positive")
    unclear = labels.count("unclear")
    return {
        "labelled": len(labels),
        "tp": tp,
        "fp": fp,
        "unclear": unclear,
        "precision": round(tp / (tp + fp), 4) if tp + fp else None,
        "wilson95": wilson(tp, tp + fp),
        "precision_if_unclear_wrong": round(tp / len(labels), 4) if labels else None,
    }


def main(argv: list[str]) -> int:
    if len(argv) not in (3, 5):
        print(__doc__, file=sys.stderr)
        return 2
    labels = {l["id"]: l for l in load(EVAL / "labels" / "v0.2.0.jsonl")}
    names = list(corpus.pins())
    result: dict = {"floor": FLOOR, "corpora": {}, "pooled": {}}
    missing = []
    pooled: dict = defaultdict(lambda: defaultdict(list))
    for name in names:
        flags = {v: load(EVAL / "flags" / f"{v}-{name}.jsonl") for v in VERSIONS}
        ids = {v: {f["id"] for f in flags[v]} for v in VERSIONS}
        rules = sorted({f["rule"] for v in VERSIONS for f in flags[v]})
        per_rule = {}
        for rule in rules:
            row = {}
            for v in VERSIONS:
                mine = [f for f in flags[v] if f["rule"] == rule]
                if len(mine) <= CAP:
                    missing += [f"{name} {v} {rule} {f['id']}" for f in mine if f["id"] not in labels]
                got = [labels[f["id"]]["label"] for f in mine if f["id"] in labels]
                pooled[rule][v] += got
                row[v] = {"flags": len(mine), **tally(got)}
            gone = ids["v0.1.0"] - ids["v0.2.0"]
            removed = [labels[i]["label"] for i in gone if i in labels and labels[i]["rule"] == rule]
            added = [i for i in ids["v0.2.0"] - ids["v0.1.0"] if any(f["id"] == i and f["rule"] == rule for f in flags["v0.2.0"])]
            row["removed"] = {"tp": removed.count("true-positive"), "fp": removed.count("false-positive"), "unclear": removed.count("unclear")}
            row["added"] = len(added)
            per_rule[rule] = row
        result["corpora"][name] = {"files": len(corpus.files(name)), "rules": per_rule}
    if missing:
        for m in missing:
            print(f"UNLABELLED {m}", file=sys.stderr)
        return 1
    for rule in sorted(pooled):
        result["pooled"][rule] = {v: tally(pooled[rule][v]) for v in VERSIONS}

    below = sorted({
        rule
        for name in names
        for rule, row in result["corpora"][name]["rules"].items()
        if row["v0.2.0"]["precision"] is not None and row["v0.2.0"]["precision"] < FLOOR
    })
    result["below_floor_v0.2.0"] = below

    status = 0
    if len(argv) == 5 and argv[3] == "--rules":
        catalog = {r["id"]: r["severity"] for r in json.loads(Path(argv[4]).read_text(encoding="utf-8"))}
        result["severity_v0.2.0"] = catalog
        for rule in below:
            if catalog.get(rule) == "error":
                print(f"R3 VIOLATION {rule} is below {FLOOR} and ships as error", file=sys.stderr)
                status = 1

    Path(argv[1]).write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8", newline="\n")
    Path(argv[2]).write_text(markdown(result), encoding="utf-8", newline="\n")
    print(f"wrote {argv[1]} and {argv[2]}; below the floor in v0.2.0: {below or 'none'}")
    return status


def cell(t: dict) -> str:
    if t["labelled"] == 0:
        return "-"
    p = "n/a" if t["precision"] is None else f"{t['precision'] * 100:.0f}%"
    ci = "" if t["wilson95"] is None else f" [{t['wilson95'][0] * 100:.0f}-{t['wilson95'][1] * 100:.0f}]"
    unclear = f", {t['unclear']} unclear" if t["unclear"] else ""
    return f"{t['tp']}/{t['tp'] + t['fp']} = {p}{ci}{unclear}"


def markdown(result: dict) -> str:
    out = [
        "# Precision per rule, v0.1.0 and v0.2.0",
        "",
        "Generated by `eval/precision.py`. Cells are TP/(TP+FP) = precision [Wilson 95% interval, %].",
        "`flags` is how many flags the version raised on the corpus; `removed` counts labelled",
        "v0.1.0 flags that v0.2.0 no longer raises, by label.",
        "",
    ]
    for name, data in result["corpora"].items():
        out += [f"## {name} ({data['files']} files)", "",
                "| Rule | flags 0.1.0 | precision 0.1.0 | flags 0.2.0 | precision 0.2.0 | removed TP / FP | added |",
                "|---|---:|---|---:|---|---|---:|"]
        for rule, row in data["rules"].items():
            a, b = row["v0.1.0"], row["v0.2.0"]
            out.append(
                f"| {rule} | {a['flags']} | {cell(a)} | {b['flags']} | {cell(b)} | "
                f"{row['removed']['tp']} / {row['removed']['fp']} | {row['added']} |"
            )
        out.append("")
    out += ["## Pooled over all corpora", "", "| Rule | 0.1.0 | 0.2.0 |", "|---|---|---|"]
    for rule, row in result["pooled"].items():
        out.append(f"| {rule} | {cell(row['v0.1.0'])} | {cell(row['v0.2.0'])} |")
    out += ["", f"Below the {result['floor']:.0%} floor in v0.2.0 on some corpus: "
            f"{', '.join(result['below_floor_v0.2.0']) or 'none'}.", ""]
    return "\n".join(out)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
