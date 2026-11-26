#!/usr/bin/env python3
"""Score a human labelling of a v3-style packet against the AI readers' labels.

The human reads `<run>/human/HUMAN_PACKET.md`, writes a CSV with
`id,label,reason`, and only then runs this script. It reports:

- raw agreement and Cohen's kappa between the human labels and the resolved
  AI labels (A and B agree, or an adjudicator resolved the difference);
- the ids on which the human and the AI resolve disagree, as the adjudication
  queue (an `--adjudication` CSV with a written reason per id replaces the
  human label for the cells, never for kappa);
- per-rule Wilson 95% cells over the resolved human labels, with the same
  rules as `measure.py`: n < 5 is a count, more than 20% unclear is
  inconclusive, and the pooled cell is k/n with its interval.

The corpus vocabulary is enforced: a packet whose flags carry the other
corpus's languages is refused. Solana and Solidity are never pooled.

`--self-test` checks the kappa and Wilson arithmetic on a fixed in-memory
fixture and reads no corpus.

Usage:
  python eval/p2r_score_human.py --run-dir <run> --human labels/human.csv \
      --labels labels/ai.jsonl --version v1
  python eval/p2r_score_human.py --self-test
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import p2r_corpora  # noqa: E402

Z = 1.959964
CATEGORIES = ("true-positive", "false-positive", "unclear")
THRESHOLD = 0.70


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
    pe = sum((counts_a[c] / n) * (counts_b[c] / n) for c in CATEGORIES)
    k = None if pe >= 1.0 else round((po - pe) / (1 - pe), 4)
    return {"n": n, "raw_agreement": round(po, 4), "kappa": k}


def read_labels_csv(path: Path) -> dict[str, dict]:
    rows: dict[str, dict] = {}
    with open(path, encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle)
        if not reader.fieldnames or "id" not in reader.fieldnames or "label" not in reader.fieldnames:
            raise SystemExit(f"{path}: header must be id,label[,reason]")
        for row in reader:
            fid = (row.get("id") or "").strip()
            label = (row.get("label") or "").strip().lower()
            reason = (row.get("reason") or "").strip()
            if not fid:
                raise SystemExit(f"{path}: a row has no id")
            if label not in CATEGORIES:
                raise SystemExit(f"{path}: {fid}: label must be one of {CATEGORIES}")
            if fid in rows:
                raise SystemExit(f"{path}: duplicate id {fid}")
            rows[fid] = {"label": label, "reason": reason}
    return rows


def resolve_ai(label_rows: list[dict]) -> dict[str, dict]:
    by_flag: dict[str, dict[str, dict]] = defaultdict(dict)
    for row in label_rows:
        if row["label"] not in CATEGORIES:
            raise SystemExit(f"{row['id']}: unknown AI label {row['label']}")
        by_flag[row["id"]][row["reader"]] = row
    resolved: dict[str, dict] = {}
    for fid, readers in by_flag.items():
        a, b = readers.get("A"), readers.get("B")
        if a and b and a["label"] == b["label"]:
            resolved[fid] = {"label": a["label"], "basis": "agreed"}
        elif (adj := readers.get("adjudicator")):
            resolved[fid] = {"label": adj["label"], "basis": "adjudicated"}
        elif a or b:
            resolved[fid] = {"label": "unclear", "basis": "not-adjudicated"}
    return resolved


def self_test() -> int:
    # kappa: agreements TP/TP and FP/FP, one disagreement TP/FP.
    pairs = [
        ("true-positive", "true-positive"),
        ("true-positive", "false-positive"),
        ("false-positive", "false-positive"),
    ]
    got = kappa(pairs)
    # The reported values are rounded to four decimals.
    kappa_ok = (
        abs((got["raw_agreement"] or 0.0) - 2 / 3) < 5e-4
        and abs((got["kappa"] or 0.0) - 0.4) < 5e-4
    )
    # Wilson 95%: 7/10 is [0.3968, 0.8922] and 5/5 is [0.5655, 1.0].
    w1 = wilson(7, 10)
    w2 = wilson(5, 5)
    wilson_ok = w1 == [0.3968, 0.8922] and w2 == [0.5655, 1.0]
    # Cell rules: n<5 is a count; a 28.6% unclear cell is inconclusive.
    count_ok = cell(["true-positive", "false-positive"])["status"] == "count"
    unclear_ok = cell(["true-positive"] * 5 + ["unclear"] * 2)["status"] == "inconclusive"
    # A hand-made tiny packet: three flags, AI readers agree, one human
    # disagreement. The scorer path (resolve -> pairs -> kappa -> cells) must
    # reproduce kappa 0.4 and count the 3-flag pooled cell.
    packet = [
        {"id": "f1", "unit": "u1", "tier": "J", "language": "solidity", "file": "a.t.sol", "line": 1},
        {"id": "f2", "unit": "u1", "tier": "J", "language": "solidity", "file": "a.t.sol", "line": 2},
        {"id": "f3", "unit": "u1", "tier": "J", "language": "solidity", "file": "a.t.sol", "line": 3},
    ]
    ai = [
        {"id": "f1", "reader": "A", "label": "true-positive", "rule": "RG01"},
        {"id": "f1", "reader": "B", "label": "true-positive", "rule": "RG01"},
        {"id": "f2", "reader": "A", "label": "true-positive", "rule": "RG01"},
        {"id": "f2", "reader": "B", "label": "true-positive", "rule": "RG01"},
        {"id": "f3", "reader": "A", "label": "false-positive", "rule": "RG01"},
        {"id": "f3", "reader": "B", "label": "false-positive", "rule": "RG01"},
    ]
    human_tiny = {"f1": "true-positive", "f2": "false-positive", "f3": "false-positive"}
    resolved = resolve_ai(ai)
    tiny_pairs = [(resolved[r["id"]]["label"], human_tiny[r["id"]]) for r in packet]
    tiny = kappa(tiny_pairs)
    tiny_cell = cell([human_tiny[r["id"]] for r in packet])
    packet_ok = (
        tiny["kappa"] == 0.4
        and tiny["n"] == 3
        and tiny_cell["status"] == "count"
        and tiny_cell["tp"] == 1
        and tiny_cell["fp"] == 2
    )
    ok = kappa_ok and wilson_ok and count_ok and unclear_ok and packet_ok
    print(
        "self-test: kappa={:.4f} raw={:.4f} wilson7/10={} wilson5/5={} cells={} packet={} {}".format(
            got["kappa"] or 0.0,
            got["raw_agreement"] or 0.0,
            w1,
            w2,
            count_ok and unclear_ok,
            packet_ok,
            "ok" if ok else "FAIL",
        )
    )
    return 0 if ok else 1


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default="solana-v3", choices=p2r_corpora.names())
    ap.add_argument("--run-dir", default=None)
    ap.add_argument("--packet", default=None,
                    help="default <run-dir>/human/human_packet.jsonl")
    ap.add_argument("--human", default=None, help="the human CSV: id,label,reason")
    ap.add_argument("--flags", default=None,
                    help="default <run-dir>/flags/<version>.jsonl")
    ap.add_argument("--version", default=None, help="needed to default --flags")
    ap.add_argument("--labels", default=None, help="the AI readers' labels.jsonl")
    ap.add_argument("--adjudication", default=None,
                    help="CSV id,label,reason; only for human/AI disagreements")
    ap.add_argument("--out", default=None,
                    help="default <run-dir>/human/human_score.json")
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--self-test", action="store_true")
    args = ap.parse_args(argv[1:])
    if args.self_test:
        return self_test()
    if not args.run_dir or not args.human:
        ap.error("--run-dir and --human are required (or --self-test)")
    run_dir = Path(args.run_dir)
    packet_path = Path(args.packet) if args.packet else run_dir / "human" / "human_packet.jsonl"
    flags_path = Path(args.flags) if args.flags else (
        run_dir / "flags" / f"{args.version}.jsonl" if args.version else None
    )
    if flags_path is None or not flags_path.is_file():
        ap.error("--flags or --version is required to locate the run's flags file")
    if not args.labels or not Path(args.labels).is_file():
        ap.error("--labels (the AI readers' labels.jsonl) is required")
    corpus = p2r_corpora.get(args.corpus)

    packet = load_jsonl(packet_path)
    ids = [row["id"] for row in packet]
    if len(ids) != len(set(ids)):
        raise SystemExit(f"{packet_path}: duplicate flag ids")
    flags = {f["id"]: f for f in load_jsonl(flags_path)}
    p2r_corpora.check_languages(corpus, [flags[i] for i in ids if i in flags])
    missing_flags = sorted(set(ids) - set(flags))
    if missing_flags:
        raise SystemExit(f"{flags_path}: {len(missing_flags)} packet id(s) have no flag: {missing_flags[:5]}")

    human = read_labels_csv(Path(args.human))
    missing = sorted(set(ids) - set(human))
    if missing:
        print(f"missing {len(missing)} of {len(ids)} ids: {', '.join(missing[:20])}")
        return 2
    extra = sorted(set(human) - set(ids))
    if extra:
        raise SystemExit(f"ids not in the packet: {', '.join(extra)}")

    resolved = resolve_ai(load_jsonl(Path(args.labels)))
    pairs = [(resolved[i]["label"], human[i]["label"]) for i in ids if i in resolved]
    agreement = kappa(pairs)
    agreement["threshold"] = THRESHOLD
    agreement["kappa_ok"] = (
        agreement["kappa"] is not None and agreement["kappa"] >= THRESHOLD
    )

    disagreements = sorted(
        i for i in ids if i in resolved and resolved[i]["label"] != human[i]["label"]
    )
    final = {i: human[i]["label"] for i in ids}
    adjudicated = 0
    if args.adjudication:
        adj = read_labels_csv(Path(args.adjudication))
        outside = sorted(set(adj) - set(disagreements))
        if outside:
            raise SystemExit(
                f"--adjudication names {len(outside)} id(s) that are not human/AI "
                f"disagreements: {outside[:5]}"
            )
        for fid, row in adj.items():
            if not row["reason"]:
                raise SystemExit(f"--adjudication {fid}: a written reason is required")
            final[fid] = row["label"]
            adjudicated += 1

    cells: dict[str, dict[str, list[str]]] = defaultdict(lambda: defaultdict(list))
    for fid in ids:
        flag = flags[fid]
        cells[flag["rule"]][flag.get("language") or "other"].append(final[fid])
    rule_cells: dict[str, dict[str, dict]] = {}
    for rule in sorted(cells):
        rule_cells[rule] = {}
        for language in corpus.cell_languages:
            labels = cells[rule].get(language)
            if not labels:
                rule_cells[rule][language] = {
                    "status": "unmeasured",
                    "reason": "the rule raised no human-labelled flag in this language",
                }
                continue
            entry = cell(labels)
            entry["reason"] = None
            rule_cells[rule][language] = entry
    pooled = cell([final[i] for i in ids])

    output = {
        "schema": "realism-gate.human-score/v1",
        "corpus": corpus.name,
        "packet": len(ids),
        "labelled": len(human),
        "missing": [],
        "agreement": agreement,
        "adjudication": {
            "provided": bool(args.adjudication),
            "rows": adjudicated,
            "human_ai_disagreements": disagreements,
        },
        "cells": rule_cells,
        "pooled": pooled,
    }
    text = json.dumps(output, indent=2, sort_keys=True) + "\n"
    out = Path(args.out) if args.out else run_dir / "human" / "human_score.json"
    if args.check:
        if not out.is_file() or out.read_text(encoding="utf-8") != text:
            print(f"{out} does not reproduce", file=sys.stderr)
            return 1
        print(f"{len(ids)} flags — check ok")
        return 0
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8", newline="\n")
    print(
        f"{len(ids)} flags, kappa {agreement['kappa']}, "
        f"{len(disagreements)} disagreement(s), {len(rule_cells)} rules -> {out}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
