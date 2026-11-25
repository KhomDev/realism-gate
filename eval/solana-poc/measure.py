#!/usr/bin/env python3
"""The PoC study's numbers, from committed files only (no network, no binary):
RQ1 coverage, RQ2 precision per rule and language, RQ3 flag vs judged
outcome, RQ4 negative control, RQ5 comparison with the P1 test-suite
measurement, and the second-pass agreement.

Every proportion carries n and a Wilson 95% interval (z = 1.959964). A
rule/language cell with fewer than 5 labels is marked `headline: false` and
the Markdown prints it as a count only.

Usage:
  eval/solana-poc/measure.py            write results.json and RESULTS.md
  eval/solana-poc/measure.py --check    re-derive both and fail if the committed files differ
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from collections import Counter, defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent.parent
HERE = ROOT / "eval" / "solana-poc"
P1 = ROOT / "eval" / "solana" / "results.json"
Z = 1.959964
MIN_N = 5
SOLANA_RULES = ["RG01", "RG02", "RG03", "RG04", "RG05", "RG06", "RG08", "RG09", "RG10", "RG11"]
LANGS = ["rust", "typescript", "javascript"]
TP, FP, UNCLEAR = "true-positive", "false-positive", "unclear"

UNREADABLE_REASON = {
    "binary-main": "Rust binary with fn main (PoC framework or CTF client)",
    "ctf-solve-program": "on-chain attacker program, not a test",
    "rust-unit-test": "Rust unit test that drives no Solana harness",
    "script": "TS/JS script, not a test",
    "test-fragment": "fragment pasted in an issue (no test wrapper, or no imports)",
}


# ---------------------------------------------------------------- statistics

def wilson(k: int, n: int) -> list[float] | None:
    if n == 0:
        return None
    p = k / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def prop(k: int, n: int) -> dict:
    return {"k": k, "n": n, "rate": round(k / n, 4) if n else None, "wilson95": wilson(k, n)}


def fisher_two_sided(a: int, b: int, c: int, d: int) -> float:
    """Fisher's exact test for [[a, b], [c, d]], two-sided by summing the
    probabilities of all tables with the same margins that are no more likely
    than the observed one."""
    r1, r2, c1 = a + b, c + d, a + c
    n = r1 + r2

    def p_table(x: int) -> float:
        return math.comb(r1, x) * math.comb(r2, c1 - x) / math.comb(n, c1)

    lo, hi = max(0, c1 - r2), min(r1, c1)
    p_obs = p_table(a)
    total = sum(p_table(x) for x in range(lo, hi + 1) if p_table(x) <= p_obs * (1 + 1e-9))
    return round(min(1.0, total), 4)


def newcombe(k1: int, n1: int, k2: int, n2: int) -> dict:
    """Risk difference p1 - p2 with Newcombe's hybrid score interval (method 10)."""
    if n1 == 0 or n2 == 0:
        return {"difference": None, "ci95": None, "note": "a group is empty"}
    p1, p2 = k1 / n1, k2 / n2
    l1, u1 = wilson(k1, n1)
    l2, u2 = wilson(k2, n2)
    d = p1 - p2
    lo = d - math.sqrt((p1 - l1) ** 2 + (u2 - p2) ** 2)
    hi = d + math.sqrt((u1 - p1) ** 2 + (p2 - l2) ** 2)
    return {"difference": round(d, 4), "ci95": [round(max(-1.0, lo), 4), round(min(1.0, hi), 4)]}


def cohen_kappa(pairs: list[tuple[str, str]]) -> float | None:
    n = len(pairs)
    if n == 0:
        return None
    cats = sorted({x for p in pairs for x in p})
    po = sum(a == b for a, b in pairs) / n
    ca, cb = Counter(a for a, _ in pairs), Counter(b for _, b in pairs)
    pe = sum(ca[c] * cb[c] for c in cats) / (n * n)
    if pe == 1:
        return 1.0 if po == 1 else 0.0
    return round((po - pe) / (1 - pe), 4)


# ---------------------------------------------------------------- loading

def load_jsonl(path: Path) -> list[dict]:
    with open(path, encoding="utf-8") as fh:
        return [json.loads(line) for line in fh if line.strip()]


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def split_of(seed: str, flag: dict) -> str:
    h = hashlib.sha256(f"{seed}|{flag['source']}|{flag['id']}".encode("utf-8")).hexdigest()
    return "dev" if int(h[:8], 16) % 2 == 0 else "heldout"


def cell(labels: list[str]) -> dict:
    tp, fp, un = labels.count(TP), labels.count(FP), labels.count(UNCLEAR)
    n = tp + fp
    return {
        "labelled": len(labels), "tp": tp, "fp": fp, "unclear": un, "n": n,
        "precision": round(tp / n, 4) if n else None,
        "wilson95": wilson(tp, n),
        "status": "measured" if n else "unmeasured",
        "headline": n >= MIN_N,
    }


# ---------------------------------------------------------------- analysis

def build() -> dict:
    corpus = json.loads((HERE / "corpus.json").read_text(encoding="utf-8"))
    inv = json.loads((HERE / "inventory.json").read_text(encoding="utf-8"))
    seed = corpus["seed"]
    version = corpus["frozen_rule_version"]
    units = {u["id"]: u for u in inv["units"] if "duplicate_of" not in u}
    runs = {r["unit"]: r for r in load_jsonl(HERE / "runs" / f"v{version}.jsonl")}
    flags = load_jsonl(HERE / "flags" / f"v{version}.jsonl")
    sample = {r["id"]: r for r in load_jsonl(HERE / "sample.jsonl")}
    label_files = sorted((HERE / "labels").glob("*.jsonl"))
    labels: dict[str, dict] = {}
    for path in label_files:
        for row in load_jsonl(path):
            if row["id"] in labels:
                raise SystemExit(f"duplicate label {row['id']}")
            if row["id"] not in sample:
                raise SystemExit(f"label for an unsampled flag {row['id']}")
            if row["split"] != split_of(seed, sample[row["id"]]):
                raise SystemExit(f"label {row['id']} has the wrong split")
            if row["label"] not in (TP, FP, UNCLEAR):
                raise SystemExit(f"label {row['id']}: unknown label {row['label']}")
            labels[row["id"]] = row
    if set(units) != set(runs):
        raise SystemExit("runs and inventory disagree on the counted units")
    missing = sorted(set(sample) - set(labels))
    if missing:
        raise SystemExit(f"{len(missing)} sampled flags are unlabelled: {missing[:5]}")

    # ---- RQ1 coverage
    def unreadable_reason(uid: str) -> str:
        u, r = units[uid], runs[uid]
        if u["kind"] == "test-file":
            return "does not parse" if r["parse_errors"] else "test drives no harness the tool recognises"
        return UNREADABLE_REASON[u["kind"]]

    rq1: dict = {"by_tier": {}, "by_kind": {}, "unreadable_reasons": {}, "unreadable_units": []}
    for tier_name, tiers in [("J", {"J"}), ("X", {"X"}), ("J+X", {"J", "X"}), ("NC", {"NC"})]:
        ids = [u for u in units if units[u]["tier"] in tiers]
        k = sum(runs[u]["readable"] for u in ids)
        rq1["by_tier"][tier_name] = prop(k, len(ids)) | {
            "tests_checked": sum(runs[u]["tests_checked"] for u in ids)}
    for tier in ("J", "X"):
        by_kind = defaultdict(list)
        for u in units:
            if units[u]["tier"] == tier:
                by_kind[units[u]["kind"]].append(runs[u]["readable"])
        rq1["by_kind"][tier] = {k: prop(sum(v), len(v)) for k, v in sorted(by_kind.items())}
    reasons = Counter()
    for u in sorted(units):
        if units[u]["tier"] in ("J", "X") and not runs[u]["readable"]:
            why = unreadable_reason(u)
            reasons[(units[u]["tier"], why)] += 1
            rq1["unreadable_units"].append({"unit": u, "tier": units[u]["tier"], "kind": units[u]["kind"], "reason": why})
    for (tier, why), n in sorted(reasons.items()):
        rq1["unreadable_reasons"].setdefault(tier, {})[why] = n

    # ---- flags overview
    poc_flags = [f for f in flags if f["tier"] in ("J", "X")]
    nc_flags = [f for f in flags if f["tier"] == "NC"]
    raised = Counter((f["rule"], f["language"]) for f in poc_flags)

    # ---- RQ2 precision
    def table(rows: list[dict]) -> dict:
        by = defaultdict(list)
        for f in rows:
            by[(f["rule"], f["language"])].append(labels[f["id"]]["label"])
        out = {}
        for rule in SOLANA_RULES:
            out[rule] = {}
            for lang in LANGS:
                c = cell(by.get((rule, lang), []))
                if c["status"] == "unmeasured":
                    if raised.get((rule, lang), 0) == 0:
                        c["reason"] = "the rule raised no flag on a PoC in this language"
                    elif not by.get((rule, lang)):
                        c["reason"] = "flags raised, none in this split"
                    else:
                        c["reason"] = "only unclear labels"
                out[rule][lang] = c
        return out

    sampled = [sample[i] for i in sorted(sample)]
    heldout = [f for f in sampled if split_of(seed, f) == "heldout"]
    dev = [f for f in sampled if split_of(seed, f) == "dev"]

    def pooled(rows: list[dict]) -> dict:
        c = cell([labels[f["id"]]["label"] for f in rows])
        return c

    rq2 = {
        "heldout": table(heldout),
        "dev": table(dev),
        "all": table(sampled),
        "heldout_by_tier": {t: table([f for f in heldout if f["tier"] == t]) for t in ("J", "X")},
        "pooled": {"heldout": pooled(heldout), "dev": pooled(dev), "all": pooled(sampled)},
        "pooled_by_rule_heldout": {r: pooled([f for f in heldout if f["rule"] == r]) for r in SOLANA_RULES},
        "pooled_by_rule_all": {r: pooled([f for f in sampled if f["rule"] == r]) for r in SOLANA_RULES},
        "flags_per_source_heldout": dict(sorted(Counter(f["source"] for f in heldout).items())),
    }

    # ---- RQ3 judged outcome (exploratory)
    def rq3_row(ids: list[str]) -> dict:
        valid = [u for u in ids if units[u]["outcome"] == "valid"]
        invalid = [u for u in ids if units[u]["outcome"] == "invalid"]
        fl = lambda u: runs[u]["active_flags"] > 0  # noqa: E731
        a, b = sum(fl(u) for u in valid), sum(not fl(u) for u in valid)
        c, d = sum(fl(u) for u in invalid), sum(not fl(u) for u in invalid)
        return {
            "table": {"valid": {"flagged": a, "not_flagged": b}, "invalid": {"flagged": c, "not_flagged": d}},
            "flag_rate_valid": prop(a, a + b),
            "flag_rate_invalid": prop(c, c + d),
            "fisher_two_sided_p": fisher_two_sided(a, b, c, d) if (a + b) and (c + d) else None,
            "risk_difference_valid_minus_invalid": newcombe(a, a + b, c, c + d),
            "units": sorted(ids),
        }

    j_platform = [u for u in units if units[u]["tier"] == "J" and units[u]["outcome_source"] == "platform-report"]
    j_all = [u for u in units if units[u]["tier"] == "J"]
    rq3 = {
        "status": "exploratory: about ten units; no predictive claim is made",
        "primary_platform_report": rq3_row(j_platform),
        "readable_only": rq3_row([u for u in j_platform if runs[u]["readable"]]),
        "sensitivity_with_self_reported": rq3_row(j_all),
    }

    # ---- RQ4 negative control
    pairs = []
    for u in sorted(units):
        if units[u]["tier"] != "NC":
            continue
        x = units[u]["pair"]
        pairs.append({
            "exploit": x, "secure": u,
            "exploit_flags": runs[x]["active_flags"], "secure_flags": runs[u]["active_flags"],
            "exploit_tests": runs[x]["tests_checked"], "secure_tests": runs[u]["tests_checked"],
            "exploit_rules": dict(sorted(Counter(f["rule"] for f in flags if f["unit"] == x).items())),
            "secure_rules": dict(sorted(Counter(f["rule"] for f in flags if f["unit"] == u).items())),
        })
    ex_flagged = sum(p["exploit_flags"] > 0 for p in pairs)
    se_flagged = sum(p["secure_flags"] > 0 for p in pairs)
    ex_f, se_f = sum(p["exploit_flags"] for p in pairs), sum(p["secure_flags"] for p in pairs)
    ex_t, se_t = sum(p["exploit_tests"] for p in pairs), sum(p["secure_tests"] for p in pairs)
    rq4 = {
        "pairs": pairs,
        "exploit_units_flagged": prop(ex_flagged, len(pairs)),
        "secure_units_flagged": prop(se_flagged, len(pairs)),
        "exploit_flags_per_checked_test": round(ex_f / ex_t, 4) if ex_t else None,
        "secure_flags_per_checked_test": round(se_f / se_t, 4) if se_t else None,
        "exploit_flags": ex_f, "secure_flags": se_f, "exploit_tests": ex_t, "secure_tests": se_t,
        "expectation_held": (se_flagged / len(pairs) <= ex_flagged / len(pairs)) and
                            ((se_f / se_t if se_t else 0) <= (ex_f / ex_t if ex_t else 0)),
    }

    # ---- RQ5 comparison with P1
    p1 = json.loads(P1.read_text(encoding="utf-8"))
    rq5 = {"population_difference": (
        "P1: public test suites (feature tests of four repositories). This study: public PoCs "
        "(judged contest PoCs and exploit-by-construction tests). The populations are not pooled."),
        "cells": {}}
    for rule in SOLANA_RULES:
        for lang in LANGS:
            a = p1["heldout"].get(rule, {}).get(lang)
            b = rq2["heldout"][rule][lang]
            if (a and a.get("status") == "measured") or b["status"] == "measured":
                rq5["cells"][f"{rule}/{lang}"] = {
                    "p1_test_suites": ({"tp": a["tp"], "n": a["tp"] + a["fp"], "precision": a["precision"],
                                        "wilson95": a["wilson95"]} if a and a.get("status") == "measured" else None),
                    "poc": ({"tp": b["tp"], "n": b["n"], "precision": b["precision"], "wilson95": b["wilson95"],
                             "headline": b["headline"]} if b["status"] == "measured" else None),
                }
    p1_tp = sum(c["tp"] for r in p1["heldout"].values() for c in r.values() if c.get("status") == "measured")
    p1_n = sum(c["tp"] + c["fp"] for r in p1["heldout"].values() for c in r.values() if c.get("status") == "measured")
    rq5["pooled"] = {"p1_test_suites": prop(p1_tp, p1_n), "poc_heldout": prop(rq2["pooled"]["heldout"]["tp"],
                                                                               rq2["pooled"]["heldout"]["n"])}

    agreement = None
    if (HERE / "agreement.json").is_file():
        agreement = json.loads((HERE / "agreement.json").read_text(encoding="utf-8"))

    return {
        "schema": "realism-gate.solana-poc-results/v1",
        "date": "2026-11-25",
        "frozen_rule_commit": corpus["frozen_rule_commit"],
        "frozen_rule_version": version,
        "seed": seed,
        "inputs_sha256": {p.relative_to(ROOT).as_posix(): sha256_file(p) for p in
                          [HERE / "corpus.json", HERE / "inventory.json", HERE / "runs" / f"v{version}.jsonl",
                           HERE / "flags" / f"v{version}.jsonl", HERE / "sample.jsonl", *label_files]},
        "counts": {
            "units_counted": len(units),
            "units_by_tier": dict(sorted(Counter(u["tier"] for u in units.values()).items())),
            "duplicates": inv["counts"]["duplicates"],
            "excluded_candidates": inv["counts"]["excluded_candidates"],
            "flags": len(flags), "flags_on_pocs": len(poc_flags), "flags_on_negative_controls": len(nc_flags),
            "flags_by_rule_language": {f"{r}/{l}": n for (r, l), n in sorted(raised.items())},
            "sampled": len(sample), "heldout": len(heldout), "dev": len(dev), "labelled": len(labels),
            "units_with_flags": len({f["unit"] for f in poc_flags}),
        },
        "rq1_coverage": rq1,
        "rq2_precision": rq2,
        "rq3_outcome": rq3,
        "rq4_negative_control": rq4,
        "rq5_comparison": rq5,
        "agreement": agreement,
        "unmeasured_rules": [r for r in SOLANA_RULES if rq2["pooled_by_rule_all"][r]["status"] == "unmeasured"],
        "not_claimed": [
            "Not recall: a defect the rules do not flag is not counted.",
            "Not a property of later versions; one tree at one commit (1.0.4).",
            "Not predictive of judging outcomes; RQ3 is exploratory.",
            "Not independent human validation: one AI-assisted reader and one AI second pass.",
            "Flags are clustered within units and sources; the Wilson intervals assume independence and are too narrow for clustered data.",
        ],
    }


# ---------------------------------------------------------------- report

def pct(c: dict) -> str:
    if c.get("rate") is not None:
        lo, hi = c["wilson95"]
        return f"{c['k']}/{c['n']} = {c['rate'] * 100:.1f}% [{lo * 100:.1f}-{hi * 100:.1f}]"
    return f"{c['k']}/{c['n']}"


def prec(c: dict) -> str:
    if c["status"] != "measured":
        return "unmeasured"
    unclear = f", {c['unclear']} unclear" if c["unclear"] else ""
    if not c["headline"]:
        return f"{c['tp']}/{c['n']} (n<{MIN_N}, count only){unclear}"
    lo, hi = c["wilson95"]
    return f"{c['tp']}/{c['n']} = {c['precision'] * 100:.1f}% [{lo * 100:.1f}-{hi * 100:.1f}]{unclear}"


def markdown(doc: dict) -> str:
    c = doc["counts"]
    rq1, rq2, rq3, rq4, rq5 = (doc["rq1_coverage"], doc["rq2_precision"], doc["rq3_outcome"],
                               doc["rq4_negative_control"], doc["rq5_comparison"])
    L = [
        "# realism-gate on real Solana PoCs: results",
        "",
        "Generated by `measure.py` from committed files; `measure.py --check` re-derives it offline.",
        f"Frozen rule logic: realism-gate {doc['frozen_rule_version']} at `{doc['frozen_rule_commit']}`. "
        f"Seed `{doc['seed']}`. Preregistration: `PREREGISTRATION.md`.",
        "",
        f"Corpus: {c['units_counted']} counted units ({', '.join(f'{k} {v}' for k, v in c['units_by_tier'].items())}), "
        f"{c['duplicates']} duplicates removed, {c['excluded_candidates']} candidates excluded with a reason "
        "(`inventory.json`). J = judged contest PoCs, X = exploit-by-construction, NC = negative controls.",
        f"Flags: {c['flags']} active ({c['flags_on_pocs']} on PoCs, {c['flags_on_negative_controls']} on negative "
        f"controls). All {c['sampled']} PoC flags were sampled (no rule passed the 30-per-source cap) and all are "
        f"labelled: {c['heldout']} held-out, {c['dev']} dev.",
        "",
        "Every proportion is k/n with a Wilson 95% interval. Flags cluster within units and sources, so the "
        "intervals, which assume independence, are narrower than they should be.",
        "",
        "## RQ1: can the tool read the PoC at all?",
        "",
        "A unit is readable when the tool checked at least one test in it.",
        "",
        "| Tier | Readable units | Tests checked |",
        "|---|---|---:|",
    ]
    for t in ("J", "X", "J+X", "NC"):
        r = rq1["by_tier"][t]
        L.append(f"| {t} | {pct(r)} | {r['tests_checked']} |")
    L += ["", "By kind:", "", "| Tier | Kind | Readable |", "|---|---|---|"]
    for t, kinds in rq1["by_kind"].items():
        for k, r in kinds.items():
            L.append(f"| {t} | {k} | {r['k']}/{r['n']} |")
    L += ["", "Unreadable units by reason (counts):", "", "| Tier | Reason | Units |", "|---|---|---:|"]
    for t, reasons in rq1["unreadable_reasons"].items():
        for why, n in reasons.items():
            L.append(f"| {t} | {why} | {n} |")

    L += ["", "## RQ2: precision on PoC flags", "",
          f"Held-out split, J and X pooled. Cells with fewer than {MIN_N} labels are counts, not percentages.",
          "", "| Rule | Language | Held-out precision | Dev | All labelled |", "|---|---|---|---|---|"]
    for rule in rq2["heldout"]:
        for lang in rq2["heldout"][rule]:
            h, d, a = rq2["heldout"][rule][lang], rq2["dev"][rule][lang], rq2["all"][rule][lang]
            if a["status"] == "measured":
                L.append(f"| {rule} | {lang} | {prec(h)} | {prec(d)} | {prec(a)} |")
    p = rq2["pooled"]
    L += ["", f"Pooled over all rules: held-out {prec(p['heldout'])}; dev {prec(p['dev'])}; "
              f"all {prec(p['all'])}.", "",
          "Pooled by rule (held-out | all labelled):", "", "| Rule | Held-out | All |", "|---|---|---|"]
    for rule in rq2["pooled_by_rule_all"]:
        if rq2["pooled_by_rule_all"][rule]["status"] == "measured":
            L.append(f"| {rule} | {prec(rq2['pooled_by_rule_heldout'][rule])} | {prec(rq2['pooled_by_rule_all'][rule])} |")
    L += ["", "Unmeasured on PoCs (no flag raised in any language): " +
          (", ".join(doc["unmeasured_rules"]) or "none") + ". Unmeasured is not 100%.", "",
          "Held-out by tier:", "", "| Tier | Rule | Language | Precision |", "|---|---|---|---|"]
    for tier, tab in rq2["heldout_by_tier"].items():
        for rule in tab:
            for lang in tab[rule]:
                if tab[rule][lang]["status"] == "measured":
                    L.append(f"| {tier} | {rule} | {lang} | {prec(tab[rule][lang])} |")

    def rq3_lines(name: str, r: dict) -> list[str]:
        t = r["table"]
        rd = r["risk_difference_valid_minus_invalid"]
        rd_s = (f"{rd['difference']:+.3f} [{rd['ci95'][0]:+.3f}, {rd['ci95'][1]:+.3f}]"
                if rd.get("difference") is not None else "undefined (" + rd.get("note", "") + ")")
        return [f"**{name}** ({len(r['units'])} units)", "",
                "| Outcome | Flagged | Not flagged |", "|---|---:|---:|",
                f"| valid | {t['valid']['flagged']} | {t['valid']['not_flagged']} |",
                f"| invalid | {t['invalid']['flagged']} | {t['invalid']['not_flagged']} |", "",
                f"Fisher two-sided p = {r['fisher_two_sided_p']}; risk difference (valid minus invalid) {rd_s}.", ""]

    L += ["", "## RQ3: does a flag say anything about the judged outcome? (exploratory)", "",
          "A unit is flagged when it has at least one active flag. With this n no predictive claim is made.", ""]
    L += rq3_lines("Primary: platform-published outcomes", rq3["primary_platform_report"])
    L += rq3_lines("Readable units only", rq3["readable_only"])
    L += rq3_lines("Sensitivity: adding self-reported outcomes", rq3["sensitivity_with_self_reported"])

    L += ["## RQ4: negative control (exploit vs patched counterpart)", "",
          "| Exploit unit | Flags | Tests | Secure unit | Flags | Tests |", "|---|---:|---:|---|---:|---:|"]
    for pr in rq4["pairs"]:
        L.append(f"| {pr['exploit']} | {pr['exploit_flags']} | {pr['exploit_tests']} | {pr['secure']} | "
                 f"{pr['secure_flags']} | {pr['secure_tests']} |")
    L += ["", f"Units flagged: exploit {pct(rq4['exploit_units_flagged'])}, secure "
              f"{pct(rq4['secure_units_flagged'])}. Flags per checked test: exploit "
              f"{rq4['exploit_flags']}/{rq4['exploit_tests']} = {rq4['exploit_flags_per_checked_test']}, secure "
              f"{rq4['secure_flags']}/{rq4['secure_tests']} = {rq4['secure_flags_per_checked_test']}. "
              f"Expectation (secure side not above exploit side) held: {rq4['expectation_held']}.", ""]

    L += ["## RQ5: PoCs vs the P1 test-suite measurement", "", rq5["population_difference"], "",
          "| Rule/language | P1 test suites (held-out) | PoCs (held-out) |", "|---|---|---|"]

    def p1s(x: dict | None) -> str:
        if not x:
            return "unmeasured"
        lo, hi = x["wilson95"]
        return f"{x['tp']}/{x['n']} = {x['precision'] * 100:.1f}% [{lo * 100:.1f}-{hi * 100:.1f}]"

    def pocs(x: dict | None) -> str:
        if not x:
            return "unmeasured"
        if not x["headline"]:
            return f"{x['tp']}/{x['n']} (count only)"
        return p1s(x)

    for key, v in rq5["cells"].items():
        L.append(f"| {key} | {p1s(v['p1_test_suites'])} | {pocs(v['poc'])} |")
    L += ["", f"Pooled over rules: P1 {pct(rq5['pooled']['p1_test_suites'])}; PoCs "
              f"{pct(rq5['pooled']['poc_heldout'])}.", ""]

    ag = doc.get("agreement")
    if ag:
        raw, fin = ag["raw"], ag["final"]
        L += ["## Agreement", "",
              f"A blinded second pass ({ag['reader']}) on a seeded {ag['sample']}-flag sample across both splits: "
              f"raw agreement {pct(raw)}, Cohen's kappa {raw['kappa']}; after adjudication {pct(fin)}, "
              f"kappa {fin['kappa']}. Disagreements and their resolution are in `agreement.json`.", ""]
    L += ["## What this cannot claim", ""] + [f"- {x}" for x in doc["not_claimed"]] + [""]
    return "\n".join(L)


def main(argv: list[str]) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--check", action="store_true")
    args = p.parse_args(argv[1:])
    doc = build()
    js = json.dumps(doc, indent=2, ensure_ascii=False) + "\n"
    md = markdown(doc)
    out_js, out_md = HERE / "results.json", HERE / "RESULTS.md"
    if args.check:
        bad = [str(f.relative_to(ROOT)) for f, text in ((out_js, js), (out_md, md))
               if not f.is_file() or f.read_text(encoding="utf-8") != text]
        if bad:
            print(f"out of date: {', '.join(bad)}", file=sys.stderr)
            return 1
        print("results.json and RESULTS.md reproduce from the committed inputs")
        return 0
    out_js.write_text(js, encoding="utf-8", newline="\n")
    out_md.write_text(md, encoding="utf-8", newline="\n")
    print(f"wrote {out_js.relative_to(ROOT)} and {out_md.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
