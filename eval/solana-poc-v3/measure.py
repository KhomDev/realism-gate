#!/usr/bin/env python3
"""Turn two-reader labels into the measured cells and the round results.

Inputs are committed files only: a run's `sample.jsonl` (the flags selected for
labelling; each row is a full flag) and a `labels.jsonl` whose rows are
{id, reader, label, rule, reason} with reader A, B or "adjudicator". Outputs
`measured.json` (the severity-floor table) and `results.json` (cells,
agreement, kappa, source-clustered bootstrap).

Rules fixed by the preregistration:
- labels are true-positive / false-positive / unclear; unclear never counts as
  TP or FP, and a cell with >20% unclear is inconclusive;
- n < 5 is a count, never a rate;
- Wilson 95% (z = 1.959964) per cell; pooled cells also get a source-clustered
  bootstrap (10,000 draws, committed seed);
- no pooling across languages, corpora or binaries: the caller passes one
  run, and `--corpus` fixes the language vocabulary that run may carry.

`--corpus` defaults to `solana-v3`; `solidity-v1` uses the same statistics
with the Solidity language vocabulary. A flags file carrying a language from
the other corpus is refused (never pool Solana with Solidity).

Usage:
  python eval/solana-poc-v3/measure.py --flags <run>/flags.jsonl --labels <labels.jsonl> \
      --measured measured.json --results results.json
  python eval/solana-poc-v3/measure.py ... --check
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import random
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent

sys.path.insert(0, str(ROOT / "eval"))
import p2r_corpora  # noqa: E402

Z = 1.959964
SEED = "p2r-bootstrap-2026-11-25"
DRAWS = 10_000
LABELS = ("true-positive", "false-positive", "unclear")


def load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def wilson(tp: int, n: int) -> list[float] | None:
    if n == 0:
        return None
    p = tp / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def cell(labels: list[str]) -> dict:
    tp = labels.count("true-positive")
    fp = labels.count("false-positive")
    unclear = labels.count("unclear")
    n = tp + fp
    labelled = len(labels)
    status = "measured"
    if n == 0:
        status = "unmeasured"
    elif n < 5:
        status = "count"
    elif labelled and unclear / labelled > 0.20:
        status = "inconclusive"
    return {
        "labelled": labelled,
        "tp": tp,
        "fp": fp,
        "unclear": unclear,
        "n": n,
        "precision": round(tp / n, 4) if n else None,
        "wilson95": wilson(tp, n),
        "status": status,
    }


def kappa(pairs: list[tuple[str, str]]) -> dict:
    if not pairs:
        return {"n": 0, "raw_agreement": None, "kappa": None}
    agree = sum(1 for a, b in pairs if a == b)
    n = len(pairs)
    po = agree / n
    counts_a: dict[str, int] = defaultdict(int)
    counts_b: dict[str, int] = defaultdict(int)
    for a, b in pairs:
        counts_a[a] += 1
        counts_b[b] += 1
    pe = sum((counts_a[c] / n) * (counts_b[c] / n) for c in LABELS)
    k = None if pe >= 1.0 else round((po - pe) / (1 - pe), 4)
    return {"n": n, "raw_agreement": round(po, 4), "kappa": k}


def bootstrap(rows: list[dict], draws: int, seed: str) -> dict | None:
    """Source-clustered percentile interval for pooled precision."""
    clusters: dict[str, list[int]] = defaultdict(lambda: [0, 0])  # source -> [tp, n]
    for r in rows:
        if r["label"] in ("true-positive", "false-positive"):
            c = clusters[r["flag"]["source"]]
            c[1] += 1
            c[0] += 1 if r["label"] == "true-positive" else 0
    if len(clusters) < 2:
        return None
    keys = sorted(clusters)
    rng = random.Random(int(hashlib.sha256(seed.encode()).hexdigest()[:16], 16))
    rates = []
    for _ in range(draws):
        tp = n = 0
        for _ in range(len(keys)):
            key = keys[rng.randrange(len(keys))]
            tp += clusters[key][0]
            n += clusters[key][1]
        if n:
            rates.append(tp / n)
    if not rates:
        return None
    rates.sort()
    lo = rates[int(0.025 * (len(rates) - 1))]
    hi = rates[int(0.975 * (len(rates) - 1))]
    return {"draws": draws, "seed": seed, "clusters": len(keys),
            "point": round(sum(clusters[k][0] for k in keys) / max(1, sum(clusters[k][1] for k in keys)), 4),
            "interval": [round(lo, 4), round(hi, 4)]}


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default="solana-v3", choices=p2r_corpora.names())
    ap.add_argument("--flags", required=True)
    ap.add_argument("--labels", required=True)
    ap.add_argument("--measured", required=True)
    ap.add_argument("--results", required=True)
    ap.add_argument("--tiers", default=None,
                    help="comma-separated tiers to measure (e.g. J,X). Amendment 2: J+X is the "
                         "verdict cell, J alone is descriptive, A and AJ are separate cells and "
                         "are never pooled with J or X. Default: every flag (pre-Amendment-2 "
                         "behaviour, kept so committed outputs reproduce).")
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    corpus = p2r_corpora.get(args.corpus)
    flags = {f["id"]: f for f in load_jsonl(Path(args.flags))}
    tiers = None
    if args.tiers:
        tiers = sorted({t.strip().upper() for t in args.tiers.split(",") if t.strip()})
        if {"A", "AJ"} & set(tiers) and {"J", "X", "N"} & set(tiers):
            raise SystemExit("Amendment 2: tiers A and AJ are never pooled with J, X or N")
        flags = {k: f for k, f in flags.items() if (f.get("tier") or "").upper() in tiers}
    p2r_corpora.check_languages(corpus, list(flags.values()))
    languages = corpus.cell_languages
    label_rows = load_jsonl(Path(args.labels))
    if tiers is not None:
        label_rows = [r for r in label_rows if r["id"] in flags]

    by_flag: dict[str, dict[str, dict]] = defaultdict(dict)
    for row in label_rows:
        fid, reader = row["id"], row["reader"]
        if fid not in flags:
            raise SystemExit(f"label {fid} has no flag")
        if flags[fid]["rule"] != row["rule"]:
            raise SystemExit(f"label {fid} names {row['rule']}, flag is {flags[fid]['rule']}")
        if row["label"] not in LABELS:
            raise SystemExit(f"label {fid} has unknown label {row['label']}")
        by_flag[fid][reader] = row

    pairs = []
    resolved: dict[str, dict] = {}
    unlabelled = []
    for fid, f in flags.items():
        readers = by_flag.get(fid, {})
        a, b = readers.get("A"), readers.get("B")
        if a and b:
            pairs.append((a["label"], b["label"]))
        if a and b and a["label"] == b["label"]:
            label, source = a["label"], "agreed"
        elif (adj := readers.get("adjudicator")):
            label, source = adj["label"], "adjudicated"
        elif a or b:
            label, source = "unclear", "not-adjudicated"
        else:
            unlabelled.append(fid)
            continue
        resolved[fid] = {"label": label, "basis": source, "flag": f}

    cells: dict[str, dict[str, list[str]]] = defaultdict(lambda: defaultdict(list))
    for fid, r in resolved.items():
        cells[r["flag"]["rule"]][r["flag"]["language"]].append(fid)

    measured_rules: dict[str, dict[str, dict]] = {}
    for rule in sorted(cells):
        measured_rules[rule] = {}
        for language in languages:
            ids = cells[rule].get(language)
            if not ids:
                measured_rules[rule][language] = {
                    "status": "unmeasured",
                    "reason": "the rule raised no labelled flag in this language",
                }
                continue
            c = cell([resolved[i]["label"] for i in ids])
            c["reason"] = None
            measured_rules[rule][language] = c

    def pooled(rows: list[dict]) -> dict:
        c = cell([r["label"] for r in rows])
        c["bootstrap"] = bootstrap(rows, DRAWS, SEED)
        return c

    all_rows = list(resolved.values())
    above = [r for r in all_rows if r["flag"].get("severity") not in (None, "note")]
    results = {
        "schema": "realism-gate.p2r-results/v1",
        "flags": len(flags),
        "labelled_flags": len(resolved),
        "unlabelled_flags": len(unlabelled),
        "agreement": kappa(pairs),
        "adjudication": {
            "agreed": sum(1 for r in all_rows if r["basis"] == "agreed"),
            "adjudicated": sum(1 for r in all_rows if r["basis"] == "adjudicated"),
            "not_adjudicated": sum(1 for r in all_rows if r["basis"] == "not-adjudicated"),
        },
        "cells": measured_rules,
        "pooled": {"all": pooled(all_rows), "above_note": pooled(above)},
    }
    if tiers is not None:
        results["tiers"] = tiers
    measured = {
        "schema": "realism-gate.measured/v1",
        "population": "see the round preregistration",
        "rules": measured_rules,
    }
    outputs = {
        Path(args.results): json.dumps(results, indent=2, sort_keys=True) + "\n",
        Path(args.measured): json.dumps(measured, indent=2, sort_keys=True) + "\n",
    }
    if args.check:
        bad = 0
        for path, text in outputs.items():
            if not path.is_file() or path.read_text(encoding="utf-8") != text:
                print(f"{path} does not reproduce", file=sys.stderr)
                bad += 1
        print(f"{len(flags)} flags, {len(resolved)} labelled, "
              f"kappa {results['agreement']['kappa']}, {len(measured_rules)} rules"
              + (" — check ok" if not bad else f" — {bad} mismatch(es)"))
        return 1 if bad else 0
    for path, text in outputs.items():
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding="utf-8", newline="\n")
    print(f"{len(flags)} flags, {len(resolved)} labelled, "
          f"kappa {results['agreement']['kappa']}, {len(measured_rules)} rules")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
