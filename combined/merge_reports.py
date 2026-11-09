#!/usr/bin/env python3
"""Merge a realism-gate SARIF file and a viability-gate JSON report into one
SARIF document and one Markdown summary.

viability-gate (`vg check --json`) does not emit SARIF. Each fired rule
becomes a SARIF result whose artifact is the finding file. realism-gate's
run is copied through. The document has two runs, so the rule tables do not
collide.

Usage:
  merge_reports.py RG.sarif VG.json OUT.sarif OUT.md
  merge_reports.py --self-test
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


def load_vg(path: Path) -> list[dict]:
    if not path.is_file() or path.stat().st_size == 0:
        return []
    data = json.loads(path.read_text(encoding="utf-8"))
    if isinstance(data, list):
        return data
    return [data]


def vg_level(verdict: str) -> str:
    if verdict == "hard":
        return "error"
    if verdict == "warn":
        return "warning"
    return "note"


def vg_run(reports: list[dict]) -> dict:
    rules = {}
    results = []
    for report in reports:
        subject = report.get("subject") or "finding.md"
        for finding in report.get("findings") or []:
            rule_id = str(finding.get("rule") or "VG")
            rules.setdefault(
                rule_id,
                {
                    "id": rule_id,
                    "name": finding.get("title") or rule_id,
                    "shortDescription": {"text": finding.get("title") or rule_id},
                    "fullDescription": {"text": finding.get("fix") or ""},
                    "help": {"text": finding.get("fix") or ""},
                },
            )
            evidence = "; ".join(finding.get("evidence") or [])[:400]
            results.append(
                {
                    "ruleId": rule_id,
                    "level": vg_level(str(finding.get("verdict") or "")),
                    "message": {
                        "text": f"{finding.get('title') or rule_id}: {evidence}".strip()
                    },
                    "locations": [
                        {
                            "physicalLocation": {
                                "artifactLocation": {"uri": subject},
                                "region": {"startLine": 1},
                            }
                        }
                    ],
                    "properties": {
                        "tool": "viability-gate",
                        "outcome": report.get("outcome"),
                        "verdict": finding.get("verdict"),
                    },
                }
            )
    return {
        "tool": {
            "driver": {
                "name": "viability-gate",
                "informationUri": "https://github.com/KhomDev/viability-gate",
                "rules": [rules[k] for k in sorted(rules)],
            }
        },
        "results": results,
    }


def merge(rg: dict, reports: list[dict]) -> dict:
    runs = list(rg.get("runs") or [])
    runs.append(vg_run(reports))
    return {
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": runs,
    }


def summary(rg: dict, reports: list[dict]) -> str:
    lines = ["# PoC and finding gates", ""]
    rg_results = []
    for run in rg.get("runs") or []:
        rg_results.extend(run.get("results") or [])
    active = [r for r in rg_results if not r.get("suppressions")]
    lines.append(f"realism-gate: {len(active)} active result(s).")
    counts: dict[str, int] = {}
    for r in active:
        counts[r.get("ruleId", "?")] = counts.get(r.get("ruleId", "?"), 0) + 1
    if counts:
        lines.append("")
        for rule in sorted(counts):
            lines.append(f"- {rule}: {counts[rule]}")
    lines.append("")
    if not reports:
        lines.append("viability-gate: no finding was checked.")
    for report in reports:
        outcome = report.get("outcome", "?")
        score = report.get("viability_score", "?")
        subject = report.get("subject") or "finding"
        lines.append(f"viability-gate `{subject}`: **{outcome}** (score {score}).")
        fired = report.get("findings") or []
        if not fired:
            lines.append("- no anti-pattern fired")
        for f in fired:
            lines.append(
                f"- {f.get('verdict', '?')} {f.get('rule')}: {f.get('title')}"
            )
        lines.append("")
    lines.append(
        "Advisory. realism-gate reads the test and does not run it. "
        "viability-gate scores the write-up. Neither decides a payout."
    )
    lines.append("")
    return "\n".join(lines)


def main(argv: list[str]) -> int:
    if argv[1:] == ["--self-test"]:
        return self_test()
    if len(argv) != 5:
        print(__doc__, file=sys.stderr)
        return 2
    rg_path, vg_path, out_sarif, out_md = map(Path, argv[1:])
    rg = json.loads(rg_path.read_text(encoding="utf-8")) if rg_path.is_file() else {"runs": []}
    reports = load_vg(vg_path)
    merged = merge(rg, reports)
    out_sarif.write_text(json.dumps(merged, indent=2) + "\n", encoding="utf-8")
    out_md.write_text(summary(rg, reports), encoding="utf-8")
    return 0


def self_test() -> int:
    rg = {
        "version": "2.1.0",
        "runs": [
            {
                "tool": {"driver": {"name": "realism-gate", "rules": [{"id": "RG15"}]}},
                "results": [
                    {
                        "ruleId": "RG15",
                        "level": "warning",
                        "message": {"text": "profit"},
                        "locations": [],
                    }
                ],
            }
        ],
    }
    vg = {
        "subject": "finding.md",
        "outcome": "KILL",
        "viability_score": 40,
        "findings": [
            {"rule": "P17", "title": "No attacker-win scenario", "verdict": "soft", "fix": "Name the attacker."}
        ],
    }
    merged = merge(rg, [vg])
    assert merged["version"] == "2.1.0"
    assert len(merged["runs"]) == 2
    assert merged["runs"][0]["tool"]["driver"]["name"] == "realism-gate"
    assert merged["runs"][1]["results"][0]["ruleId"] == "P17"
    text = summary(rg, [vg])
    assert "RG15" in text and "KILL" in text and "P17" in text
    assert "no finding" in summary(rg, [])
    print("ok")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
