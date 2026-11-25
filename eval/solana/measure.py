#!/usr/bin/env python3
"""Per-rule precision of the Rust and Anchor TypeScript rules on the held-out
split of the pinned public test-suite corpus, with Wilson 95% intervals.

The population is public test suites, not judged PoCs. The report and every
consumer must say so; see PREREGISTRATION.md.

Usage:
  eval/solana/measure.py [--out-json eval/solana/results.json]
                         [--out-md eval/solana/RESULTS.md]
                         [--out-measured eval/solana/measured.json]
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solana"
Z = 1.959964


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def wilson(tp: int, n: int) -> list[float] | None:
    if n == 0:
        return None
    p = tp / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def split_of(seed: str, flag: dict) -> str:
    h = hashlib.sha256(f"{seed}|{flag['corpus']}|{flag['id']}".encode("utf-8")).hexdigest()
    return "dev" if int(h[:8], 16) % 2 == 0 else "heldout"


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


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--out-json", default=str(HERE / "results.json"))
    p.add_argument("--out-md", default=str(HERE / "RESULTS.md"))
    p.add_argument("--out-measured", default=str(HERE / "measured.json"))
    args = p.parse_args(argv[1:])
    pins = json.loads((HERE / "corpus.json").read_text(encoding="utf-8"))
    seed = pins["seed"]
    flags: list[dict] = []
    for corpus in sorted(pins["corpora"]):
        path = HERE / "flags" / f"v{pins['frozen_rule_version']}-{corpus}.jsonl"
        flags.extend(load_jsonl(path))
    sampled: dict[str, dict] = {}
    for corpus in sorted(pins["corpora"]):
        for row in load_jsonl(HERE / "samples" / f"{corpus}.jsonl"):
            sampled[row["id"]] = row
    labels: dict[str, dict] = {}
    label_files = sorted((HERE / "labels").glob("*.jsonl"))
    for path in label_files:
        for row in load_jsonl(path):
            if row["id"] in labels:
                raise SystemExit(f"duplicate label for {row['id']}")
            labels[row["id"]] = row
    unknown = sorted(set(labels) - set(sampled))
    if unknown:
        raise SystemExit(f"labels for flags that are not sampled: {unknown[:5]}")
    wrong_split = [
        row["id"] for row in labels.values()
        if row.get("split") != split_of(seed, sampled[row["id"]])
    ]
    if wrong_split:
        raise SystemExit(f"labels with a split that does not match the seed: {wrong_split[:5]}")

    heldout = [f for f in sampled.values() if split_of(seed, f) == "heldout"]
    missing = sorted(f["id"] for f in heldout if f["id"] not in labels)
    if missing:
        raise SystemExit(
            f"{len(missing)} held-out flags are unlabelled: {missing[:5]}"
        )
    heldout_labelled = [f for f in heldout if f["id"] in labels]
    dev = [f for f in sampled.values() if split_of(seed, f) == "dev"]
    dev_labelled = [f for f in dev if f["id"] in labels]

    by_rule: dict[str, dict[str, list[str]]] = defaultdict(lambda: defaultdict(list))
    for f in heldout_labelled:
        by_rule[f["rule"]][f["language"]].append(labels[f["id"]]["label"])
    rules = sorted({f["rule"] for f in flags})
    languages = sorted({f["language"] for f in flags})
    table = {}
    for rule in rules:
        table[rule] = {}
        for lang in languages:
            labels_for_cell = by_rule.get(rule, {}).get(lang, [])
            c = cell(labels_for_cell)
            if c["status"] == "unmeasured":
                if not any(f["rule"] == rule and f["language"] == lang for f in heldout):
                    c["reason"] = "no sampled flag on the held-out split"
                else:
                    c["reason"] = "sampled but not labelled"
            table[rule][lang] = c

    doc = {
        "schema": "realism-gate.solana-precision/v1",
        "date": "2026-11-24",
        "frozen_rule_commit": pins["frozen_rule_commit"],
        "frozen_rule_version": pins["frozen_rule_version"],
        "seed": seed,
        "population": pins["population"],
        "split": pins["split"],
        "corpora": pins["corpora"],
        "labels": {
            "files": [str(p.relative_to(ROOT)).replace("\\", "/") for p in label_files],
            "sha256": {p.name: sha256_file(p) for p in label_files},
        },
        "counts": {
            "flags": len(flags),
            "sampled": len(sampled),
            "heldout_sampled": len(heldout),
            "heldout_labelled": len(heldout_labelled),
            "dev_sampled": len(dev),
            "dev_labelled": len(dev_labelled),
        },
        "heldout": table,
        "agreement": (
            json.loads((HERE / "agreement.json").read_text(encoding="utf-8"))
            if (HERE / "agreement.json").is_file()
            else None
        ),
        "not_claimed": [
            "This is not PoC precision: the corpus is public test suites, not judged PoCs or incident reproductions.",
            "It is not recall: a rule that raised nothing is unmeasured, not 100%.",
            "It is not a property of future versions; it is one tree at one commit.",
            "It does not establish that a rule is safe to ship at error severity.",
        ],
    }
    Path(args.out_json).write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8", newline="\n")
    Path(args.out_measured).write_text(
        json.dumps(measured_view(doc), indent=2) + "\n", encoding="utf-8", newline="\n"
    )
    Path(args.out_md).write_text(markdown(doc), encoding="utf-8", newline="\n")
    print(f"wrote {args.out_json}, {args.out_md}, {args.out_measured}")
    return 0


def solidity_cells() -> dict[str, dict]:
    """Fresh-corpus Solidity cells from the existing precision study.

    Only the fresh corpus is out-of-sample; the in-sample and later numbers stay
    in docs/precision.md and are deliberately not promoted into the catalog.
    """
    path = ROOT / "eval" / "results" / "precision.json"
    if not path.is_file():
        return {}
    data = json.loads(path.read_text(encoding="utf-8"))
    fresh = data.get("v0.6.0", {}).get("fresh", {})
    out = {}
    for rule, row in fresh.items():
        out[rule] = {
            "labelled": row["labelled"],
            "tp": row["tp"],
            "fp": row["fp"],
            "unclear": row["unclear"],
            "precision": row["precision"],
            "wilson95": row["wilson95"],
            "status": "measured",
            "split": "fresh",
            "population": "Solidity public PoCs (DeFiVulnLabs)",
        }
    return out


POC_RESULTS = ROOT / "eval" / "solana-poc" / "results.json"
POC_POPULATION = "public Solana PoCs (held-out split; eval/solana-poc)"
COUNT_ONLY_BELOW = 5


def measured_view(doc: dict) -> dict:
    """The compact view the `rules`/`explain` implementation reads.

    Since 2026-11-25 the Rust/TypeScript/JavaScript cells come from the held-out
    split of the Solana PoC study (`eval/solana-poc/results.json`), not from the
    test-suite measurement in this directory, which stays in RESULTS.md. A cell
    with fewer than five labels is marked `count_only`, as the PoC
    preregistration requires.
    """
    solidity = solidity_cells()
    poc = json.loads(POC_RESULTS.read_text(encoding="utf-8"))
    rules = {}
    for rule in [f"RG{i:02}" for i in range(1, 21)]:
        cells = {}
        for lang, cell in poc["rq2_precision"]["heldout"].get(rule, {}).items():
            if cell.get("status") == "measured":
                out = {k: cell[k] for k in ("labelled", "tp", "fp", "unclear", "precision", "wilson95")}
                out["status"] = "measured"
                out["split"] = "heldout"
                out["population"] = POC_POPULATION
                if cell["tp"] + cell["fp"] < COUNT_ONLY_BELOW:
                    out["count_only"] = True
            else:
                out = {"status": "unmeasured", "reason": cell.get("reason") or "no held-out PoC label"}
            cells[lang] = out
        sol = dict(solidity.get(rule, {
            "status": "unmeasured",
            "reason": "no fresh Solidity labels; in-sample numbers are in docs/precision.md",
        }))
        # The same n<5 rule applies to every language, not only the PoC cells.
        if sol.get("status") == "measured" and sol["tp"] + sol["fp"] < COUNT_ONLY_BELOW:
            sol["count_only"] = True
        cells["solidity"] = sol
        cells["move"] = {
            "status": "unmeasured",
            "reason": "no labelled Move corpus",
        }
        rules[rule] = cells
    return {
        "schema": "realism-gate.measured/v1",
        "date": poc["date"],
        "population": POC_POPULATION + ": judged contest PoCs and exploit-by-construction tests. "
        "Cells with n<5 (any language) are counts only. The test-suite measurement is eval/solana/RESULTS.md.",
        "source": "eval/solana-poc/results.json",
        "frozen_rule_commit": poc["frozen_rule_commit"],
        "rules": rules,
    }


def fmt(c: dict) -> str:
    if c["status"] != "measured":
        return "unmeasured"
    pct = c["precision"] * 100
    lo, hi = c["wilson95"]
    unclear = f", {c['unclear']} unclear" if c["unclear"] else ""
    return f"{c['tp']}/{c['tp'] + c['fp']} = {pct:.1f}% [{lo * 100:.1f}-{hi * 100:.1f}]{unclear}"


def markdown(doc: dict) -> str:
    lines = [
        "# Rust and Anchor TypeScript rule precision",
        "",
        "**Population: public test suites, not judged PoCs.** The four corpora are pinned public",
        "Rust and Anchor TypeScript test suites. This page originally said the intended PoC corpus",
        "does not exist publicly in a form this tool reads; that was wrong (the search stopped after",
        "three repositories). The PoC measurement is `eval/solana-poc/RESULTS.md` (2026-11-25).",
        "Every number below is precision on public test suites. It is not PoC precision, and it",
        "must not be quoted as such.",
        "",
        f"Frozen rule logic: realism-gate {doc['frozen_rule_version']} at "
        f"`{doc['frozen_rule_commit']}`. Seed: `{doc['seed']}`. "
        "The headline is the held-out split; each flag was assigned by "
        "`sha256(seed|corpus|flag_id)` before any label was written.",
        "",
        f"Counts: {doc['counts']['flags']} flags, {doc['counts']['sampled']} sampled "
        f"(at most 30 per rule per corpus), {doc['counts']['heldout_labelled']} held-out labels. "
        f"{doc['counts']['dev_sampled']} dev-sampled flags are unlabelled; the held-out table is "
        "the measurement.",
        "",
        "## Held-out precision",
        "",
        "| Rule | Language | Precision | n | Unclear |",
        "|---|---|---|---:|---:|",
    ]
    for rule, langs in sorted(doc["heldout"].items()):
        for lang, c in sorted(langs.items()):
            if c["status"] != "measured":
                continue
            lines.append(f"| {rule} | {lang} | {fmt(c)} | {c['tp'] + c['fp']} | {c['unclear']} |")
    lines += ["", "## Unmeasured", "",
              "A rule with no held-out label is not 100%. It is unmeasured, with the reason.", "",
              "| Rule | Language | Reason |", "|---|---|---|"]
    for rule, langs in sorted(doc["heldout"].items()):
        for lang, c in sorted(langs.items()):
            if c["status"] != "measured":
                lines.append(f"| {rule} | {lang} | {c.get('reason', '')} |")
    if doc.get("agreement"):
        raw = doc["agreement"]["raw"]
        final = doc["agreement"]["final"]
        lines += [
            "",
            "## Agreement",
            "",
            f"A blinded second pass on a seeded {doc['agreement']['sample']}-flag sample: "
            f"raw {raw['agree']}/{raw['n']} = {raw['rate'] * 100:.1f}% "
            f"[{raw['wilson95'][0] * 100:.1f}-{raw['wilson95'][1] * 100:.1f}]; "
            f"after the one adjudication {final['agree']}/{final['n']} = "
            f"{final['rate'] * 100:.1f}% "
            f"[{final['wilson95'][0] * 100:.1f}-{final['wilson95'][1] * 100:.1f}]. "
            "The disagreement is recorded in `agreement.json`; it changed the RG04 cell.",
        ]
    lines += [
        "",
        "## What this cannot claim",
        "",
        "- It is not PoC precision; the population is public test suites.",
        "- It is not recall; a rule that raised nothing is unmeasured.",
        "- It is not a property of future versions; it is one tree at one commit.",
        "- It does not establish that a rule is safe to ship at error severity.",
        "",
    ]
    return "\n".join(lines)


if __name__ == "__main__":
    sys.exit(main(sys.argv))
