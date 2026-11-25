#!/usr/bin/env python3
"""Turn the committed held-out labels into `measured.json` for the severity floor.

Reads only committed files: `labels/heldout.jsonl` (split == heldout) and
`flags/v1.3.0.jsonl`, joins a label to its flag by `id`, and writes one cell
per rule and language. It never touches the network and never reads a flag the
label file does not cover, so re-running it cannot change a label.

Population string: public Solana PoCs, held-out split of the sealed v2 corpus.
Only cells with n = tp + fp >= 5 are rates; `tests/precision_floor.rs` applies
the same rule.
"""

from __future__ import annotations

import argparse
import json
import math
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
Z = 1.96
POPULATION = (
    "public Solana PoCs (held-out split; eval/solana-poc-v2): judged contest "
    "PoCs and exploit-by-construction tests. Cells with n<5 are counts only. "
    "The corpus and the round are described in eval/solana-poc-v2/RESULTS.md."
)
SOURCE = "eval/solana-poc-v2/labels/heldout.jsonl + flags/v1.3.0.jsonl"
FROZEN_RULE_COMMIT = "fcba51d"
FROZEN_VERSION = "1.3.0"
LANGUAGES = ("rust", "typescript", "javascript", "solidity")


def wilson(tp: int, n: int) -> list[float] | None:
    if n == 0:
        return None
    p = tp / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def load_jsonl(path: Path) -> list[dict]:
    with open(path, encoding="utf-8") as fh:
        return [json.loads(line) for line in fh if line.strip()]


def cell(labels: list[str]) -> dict:
    tp = labels.count("true-positive")
    fp = labels.count("false-positive")
    unclear = labels.count("unclear")
    n = tp + fp
    return {
        "labelled": len(labels),
        "tp": tp,
        "fp": fp,
        "unclear": unclear,
        "precision": round(tp / n, 4) if n else None,
        "wilson95": wilson(tp, n),
        "status": "measured" if n else "unmeasured",
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--labels", default=str(HERE / "labels/heldout.jsonl"))
    ap.add_argument("--flags", default=str(HERE / "flags/v1.3.0.jsonl"))
    ap.add_argument("--out", default=str(HERE / "measured.json"))
    args = ap.parse_args()

    labels = [r for r in load_jsonl(Path(args.labels)) if r.get("split") == "heldout"]
    flags = {f["id"]: f for f in load_jsonl(Path(args.flags))}

    by_rule_lang: dict[str, dict[str, list[str]]] = defaultdict(lambda: defaultdict(list))
    for row in labels:
        flag = flags.get(row["id"])
        if flag is None:
            raise SystemExit(f"label {row['id']} has no flag in the 1.3.0 run")
        if flag["rule"] != row["rule"]:
            raise SystemExit(f"label {row['id']} names {row['rule']}, flag is {flag['rule']}")
        by_rule_lang[row["rule"]][flag["language"]].append(row["label"])

    rules: dict[str, dict[str, dict]] = {}
    for rule in sorted(by_rule_lang):
        rules[rule] = {}
        for language in LANGUAGES:
            got = by_rule_lang[rule].get(language)
            if not got:
                rules[rule][language] = {
                    "status": "unmeasured",
                    "reason": "the rule raised no labelled flag on a PoC in this language",
                }
                continue
            entry = cell(got)
            entry["split"] = "heldout"
            entry["population"] = "public Solana PoCs (held-out split; eval/solana-poc-v2)"
            rules[rule][language] = entry

    doc = {
        "schema": "realism-gate.measured/v1",
        "date": "2026-11-25",
        "population": POPULATION,
        "source": SOURCE,
        "frozen_rule_commit": FROZEN_RULE_COMMIT,
        "frozen_version": FROZEN_VERSION,
        "rules": rules,
    }
    out = Path(args.out)
    out.write_text(json.dumps(doc, indent=2, sort_keys=False) + "\n", encoding="utf-8", newline="\n")
    print(f"wrote {out} ({len(labels)} held-out labels, {len(rules)} rules)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
