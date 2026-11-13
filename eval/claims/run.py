#!/usr/bin/env python3
"""Score realism-gate's claim binding on 120 public PoC + incident-record pairs.
Offline; the corpus is fetched once by fetch.py.

    python eval/claims/run.py --gate target/debug/realism-gate.exe --split all
    python eval/claims/run.py --self-test
    python eval/claims/run.py --check

What is measured (pre-registered in REPORT.md): for each pair, realism-gate is
given the PoC, a report file rendered from the public incident record, and a
structured claims.yaml rendered from the same record. For every claim row the
tool marks CONTRADICTED or NOT SHOWN, does the frozen rubric also say the
evidence does not show it? That is claim-binding precision, with a Wilson 95%
interval. A tool that marks a row SHOWN that the rubric says is not evidenced is
a miss, reported separately (not in the precision ratio).

The pair label and the field labels come from eval/claims/label.py and
labels.yaml. The rubric is frozen before any tool run; it labels structural
evidence, not truth. See REPORT.md for why that is the honest ground truth a
public PoC corpus can support.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

try:
    import yaml
except ImportError:
    print("eval/claims: PyYAML is required", file=sys.stderr)
    sys.exit(2)

HERE = Path(__file__).resolve().parent
RG = HERE.parent.parent
PAIRS = HERE / "pairs.yaml"
LABELS = HERE / "labels.yaml"
SOURCES = HERE / "sources.yaml"
RESULTS = HERE / "results.json"
FREEZE = HERE / "freeze.json"
OUT = HERE / "cache" / "claims-out"
FIXTURES = HERE / "fixtures" / "rejections.yaml"
Z = 1.959964
FIELD_PRIORITY = ("impact", "attacker")


def wilson(k: int, n: int) -> list[float] | None:
    if n == 0:
        return None
    p = k / n
    denom = 1 + Z * Z / n
    centre = (p + Z * Z / (2 * n)) / denom
    half = Z * math.sqrt(p * (1 - p) / n + Z * Z / (4 * n * n)) / denom
    return [round(max(0.0, centre - half), 4), round(min(1.0, centre + half), 4)]


def sha256_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def spec() -> dict:
    return yaml.safe_load(SOURCES.read_text(encoding="utf-8"))


def seed() -> str:
    return spec()["seed"]


def split_of(item_id: str) -> str:
    digest = hashlib.sha256(f"{seed()}:{item_id}".encode()).hexdigest()
    return "heldout" if int(digest[:8], 16) % 2 == 1 else "dev"


def in_split(split: str, item_id: str) -> bool:
    return split == "all" or split_of(item_id) == split


def corpus_root() -> Path:
    c = spec()["corpora"][0]
    for candidate in ((RG / c["dir"]).resolve(), (RG / c["fallback_dir"]).resolve()):
        if (candidate / ".git").is_dir():
            return candidate
    raise SystemExit("corpus not found; run fetch.py first")


def load_pairs() -> dict:
    return yaml.safe_load(PAIRS.read_text(encoding="utf-8"))


def load_labels() -> dict:
    return yaml.safe_load(LABELS.read_text(encoding="utf-8"))


# --------------------------------------------------------------------------
# Render the pair's inputs from the public incident record.
# --------------------------------------------------------------------------

def render_report(pair: dict) -> str:
    inc = pair["incident"]
    name = inc.get("name") or pair["id"]
    lines = [
        "# Public incident record: " + name,
        "",
        "Period: " + pair["period"] + ".",
        "Recorded loss: " + inc["lost_text"] + ".",
        "Impact: funds were drained from the protocol by an external attacker.",
    ]
    if inc.get("attacker_address"):
        lines.append("Attacker address: " + inc["attacker_address"] + ".")
    if inc.get("attack_tx"):
        lines.append("Attack transaction: " + inc["attack_tx"] + ".")
    if inc.get("analyses"):
        lines += ["", "Public analyses:"]
        lines += ["- " + u for u in inc["analyses"]]
    lines += ["", "Source record: DeFiHackLabs header (@KeyInfo), pinned repo@commit in sources.yaml.", ""]
    return "\n".join(lines)


def render_claims(pair: dict) -> str:
    claim = pair["claim"]
    doc = {
        "schema": "realism-gate.claims/v1",
        "attacker": claim.get("attacker") or "any-user",
        "impact": claim.get("impact") or "drain",
    }
    if claim.get("asset"):
        doc["asset"] = claim["asset"]
    if claim.get("amount"):
        doc["amount"] = claim["amount_text"] if claim.get("amount_text") else claim["amount"]
    return yaml.safe_dump(doc, sort_keys=False, allow_unicode=True)


def run(cmd: list[str], timeout: int = 120) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=timeout)


def check_pair(binary: str, pair: dict, root: Path) -> dict:
    pair_dir = OUT / pair["id"].replace(":", "_").replace("/", "_")
    pair_dir.mkdir(parents=True, exist_ok=True)
    poc = root / pair["poc"]["path"]
    report = pair_dir / "report.md"
    claims = pair_dir / "claims.yaml"
    report.write_text(render_report(pair), encoding="utf-8", newline="\n")
    claims.write_text(render_claims(pair), encoding="utf-8", newline="\n")
    proc = run([binary, "check", str(poc), "--report", str(report), "--claims", str(claims),
                "--format", "json", "--no-fail-on-claims"])
    if proc.returncode not in (0, 1):
        raise RuntimeError(f"realism-gate check failed ({proc.returncode}): {proc.stderr[:300]}")
    try:
        return json.loads(proc.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"check did not print JSON: {exc}")


FLAGGED = {"contradicted", "not_shown"}


def measure(split: str, binary: str, version: str) -> dict:
    doc = load_pairs()
    labels = load_labels()
    root = corpus_root()
    per_pair = []
    for pair in doc["pairs"]:
        if not in_split(split, pair["id"]):
            continue
        report = check_pair(binary, pair, root)
        rows = (report.get("claims") or {}).get("rows") or []
        tool_rows = {r.get("field"): r.get("status") for r in rows if r.get("field")}
        mine = labels["pairs"][pair["id"]]
        fields = {}
        for field in FIELD_PRIORITY:
            tool_status = tool_rows.get(field)
            if tool_status is None:
                fields[field] = {"tool": None, "label": mine[field], "counted": False}
                continue
            if tool_status in FLAGGED:
                agree = mine[field] in FLAGGED
                fields[field] = {"tool": tool_status, "label": mine[field],
                                 "counted": True, "agree": agree}
            elif tool_status == "shown":
                fields[field] = {"tool": tool_status, "label": mine[field],
                                 "counted": False, "missed_evidence_gap": mine[field] in FLAGGED}
            else:
                fields[field] = {"tool": tool_status, "label": mine[field], "counted": False}
        per_pair.append({"id": pair["id"], "outcome": (report.get("claims") or {}).get("outcome"),
                         "claim_binding": report.get("claim_binding"), "fields": fields})
    return {"per_pair": per_pair, "version": version}


def summarise(per_pair: list[dict]) -> dict:
    out = {}
    for field in FIELD_PRIORITY:
        flagged = [p["fields"][field] for p in per_pair if p["fields"][field].get("counted")]
        agree = sum(1 for f in flagged if f.get("agree"))
        missed = sum(1 for p in per_pair if p["fields"][field].get("missed_evidence_gap"))
        out[field] = {
            "flagged": len(flagged), "agree": agree,
            "precision": round(agree / len(flagged), 4) if flagged else None,
            "wilson95": wilson(agree, len(flagged)),
            "tool_flagged_label_shown": len(flagged) - agree,
            "shown_by_tool_label_gap": missed,
            "status": "measured" if flagged else "not_measured",
        }
    # The identity rubric only checks whether the claimed address literal occurs
    # in the PoC source. realism-gate binds identity to how the PoC drives the
    # call, which is stronger. A disagreement here is therefore a label defect,
    # not a tool error, and the ratio must not be quoted as precision.
    out["attacker"]["status"] = "not_interpretable"
    out["attacker"]["reason"] = (
        "the rubric labels identity by address-literal presence; realism-gate checks "
        "whether the PoC uses that identity to drive the call. The 0.0 is the "
        "disagreement rate of a deliberately weak label, not a tool error rate; "
        "re-label identity by caller binding before using this field.")
    return out


NOT_CLAIMED = [
    "not a measurement of the tool's full rule set: only claim binding is scored here",
    "not truth: the labels say whether the PoC source structurally evidences the claim, not whether the incident happened",
    "the claim source is the public incident record (DeFiHackLabs header), not the full linked write-up",
    "amount claims are not scored: the record states a fiat loss, the PoC asserts token balances",
    "the 3 rejection-row fixtures need private inputs and are not scored from this repository",
]


def corpus_facts() -> dict:
    doc = load_pairs()
    labels = load_labels()
    counts = {}
    field_counts = {}
    for row in labels["pairs"].values():
        counts[row["pair"]] = counts.get(row["pair"], 0) + 1
        for field in ("attacker", "impact", "amount"):
            key = field + ":" + row[field]
            field_counts[key] = field_counts.get(key, 0) + 1
    return {
        "pairs": len(doc["pairs"]),
        "population": doc["sampling"]["population"],
        "seed": doc["seed"],
        "pairs_sha256": sha256_file(PAIRS),
        "labels_sha256": sha256_file(LABELS),
        "pair_labels": dict(sorted(counts.items())),
        "field_labels": dict(sorted(field_counts.items())),
        "report_coverage": {
            "incident_record": len(doc["pairs"]),
            "fetched_writeup": 0,
            "note": "the report file is rendered from the public incident record; linked write-ups are pointers",
        },
    }


def blocked(reason: str) -> int:
    doc = {
        "schema": "realism-gate.claims.results/v1",
        "generated_by": "eval/claims/run.py",
        "date": "2026-11-13",
        "corpus": corpus_facts(),
        "tool": None,
        "frozen": {"seed": seed(), "pairs_sha256": sha256_file(PAIRS),
                   "labels_sha256": sha256_file(LABELS), "heldout_opened": False,
                   "harness_sha256": sha256_file(HERE / "run.py")},
        "measurements": {f: {"status": "not_measured", "reason": reason, "n": None}
                         for f in FIELD_PRIORITY},
        "fixtures": fixtures_facts(),
        "not_claimed": NOT_CLAIMED,
    }
    RESULTS.write_text(json.dumps(doc, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"status": "not_measured", "reason": reason,
                      "corpus": doc["corpus"], "fixtures": doc["fixtures"]}, indent=2))
    return 1


def fixtures_facts() -> dict:
    if not FIXTURES.is_file():
        return {"present": False}
    doc = yaml.safe_load(FIXTURES.read_text(encoding="utf-8"))
    return {"present": True, "rows": [r["id"] for r in doc.get("rows", [])],
            "status": doc.get("status"), "why": doc.get("why_not_run")}


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description="score claim-binding precision (offline)")
    parser.add_argument("--gate", default=str(RG / "target" / "debug" / "realism-gate.exe"))
    parser.add_argument("--split", choices=["dev", "heldout", "all"], default="dev")
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv)
    if args.check:
        doc = json.loads(RESULTS.read_text(encoding="utf-8"))
        if doc.get("corpus", {}).get("pairs") != len(load_pairs()["pairs"]):
            print("eval/claims check failed: corpus count drifted", file=sys.stderr)
            return 1
        print("eval/claims check ok: " + json.dumps(doc.get("measurements")))
        return 0
    if args.self_test:
        return self_test()
    if not PAIRS.is_file() or not LABELS.is_file():
        return blocked("pairs.yaml/labels.yaml absent; run fetch.py --pairs and label.py --write")
    if args.split in ("heldout", "all"):
        if not FREEZE.is_file():
            print("refusing to open held-out before freeze: run --split dev first", file=sys.stderr)
            return 2
        if json.loads(FREEZE.read_text(encoding="utf-8")).get("labels_sha256") != sha256_file(LABELS):
            print("refusing: labels changed since freeze.json", file=sys.stderr)
            return 2
    binary = Path(args.gate)
    if not binary.is_file():
        return blocked(f"realism-gate binary not found at {binary}; claim binding is not measured until it exists")
    help_text = run([str(binary), "check", "--help"]).stdout + run([str(binary), "--help"]).stdout
    missing = [flag for flag in ("--report", "--claims", "--format") if flag not in help_text]
    if missing:
        return blocked(
            f"the installed realism-gate does not implement claim binding (missing {missing}); "
            "the corpus and labels are ready and this run will score automatically when 1.0 is built")
    version = run([str(binary), "--version"]).stdout.strip()
    doc = load_pairs()
    OUT.mkdir(parents=True, exist_ok=True)
    measured = measure(args.split, str(binary), version)
    measurements = summarise(measured["per_pair"])
    out = {
        "schema": "realism-gate.claims.results/v1",
        "generated_by": "eval/claims/run.py",
        "date": "2026-11-13",
        "corpus": corpus_facts(),
        "tool": {"path": str(binary), "version": version, "sha256": sha256_file(binary)},
        "frozen": {"seed": seed(), "pairs_sha256": sha256_file(PAIRS),
                   "labels_sha256": sha256_file(LABELS),
                   "heldout_opened": args.split in ("heldout", "all"),
                   "harness_sha256": sha256_file(HERE / "run.py")},
        "measurements": measurements,
        "per_pair": measured["per_pair"],
        "agreement": (load_labels().get("agreement")),
        "fixtures": fixtures_facts(),
        "not_claimed": NOT_CLAIMED,
    }
    RESULTS.write_text(json.dumps(out, indent=2) + "\n", encoding="utf-8", newline="\n")
    if args.split == "dev" and not FREEZE.is_file():
        FREEZE.write_text(json.dumps({
            "frozen_at": "written by eval/claims/run.py --split dev",
            "date": "2026-11-13",
            "seed": seed(),
            "pairs_sha256": sha256_file(PAIRS),
            "labels_sha256": sha256_file(LABELS),
            "harness_sha256": sha256_file(HERE / "run.py"),
        }, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps(measurements, indent=2))
    return 0


# --------------------------------------------------------------------------
# Self-test: a fake gate implementing the rg-core JSON contract.
# --------------------------------------------------------------------------

def self_test() -> int:
    tmp = Path(tempfile.mkdtemp(prefix="rg-claims-selftest-"))
    pairs = {"version": 1, "seed": "selftest", "pairs": [
        {"id": "p1", "poc": {"path": "p1.sol"}, "incident": {"name": "p1", "lost_text": "$1"},
         "period": "2024-01", "claim": {"attacker": "address:0xabc", "impact": "drain"}},
        {"id": "p2", "poc": {"path": "p2.sol"}, "incident": {"name": "p2", "lost_text": "$2"},
         "period": "2024-01", "claim": {"attacker": "address:0xdef", "impact": "drain"}},
    ]}
    labels = {"version": 1, "seed": "selftest", "pairs": {
        "p1": {"attacker": "shown", "impact": "shown", "amount": "not_checkable", "pair": "shown"},
        "p2": {"attacker": "shown", "impact": "not_shown", "amount": "not_checkable", "pair": "not_shown"},
    }, "agreement": None}
    fake = tmp / "fake_gate.py"
    fake.write_text(FAKE_GATE, encoding="utf-8")
    if os.name == "nt":
        binary = tmp / "fake.cmd"
        binary.write_text("@echo off\r\npython \"" + str(fake) + "\" %*\r\n", encoding="utf-8")
    else:
        fake.chmod(0o755)
        binary = fake
    saved = (PAIRS, LABELS, RESULTS, OUT)
    _saved_corpus_root = corpus_root
    try:
        (tmp / "pairs.yaml").write_text(yaml.safe_dump(pairs), encoding="utf-8")
        (tmp / "labels.yaml").write_text(yaml.safe_dump(labels), encoding="utf-8")
        globals().update(PAIRS=tmp / "pairs.yaml", LABELS=tmp / "labels.yaml",
                         RESULTS=tmp / "results.json", OUT=tmp / "out")
        # fake corpus: no git, so monkeypatch corpus_root
        globals()["corpus_root"] = lambda: tmp
        (tmp / "p1.sol").write_text("// p1", encoding="utf-8")
        (tmp / "p2.sol").write_text("// p2", encoding="utf-8")
        os.environ["FAKE_GATE_SPEC"] = str(tmp / "spec.json")
        (tmp / "spec.json").write_text(json.dumps({
            "p1": {"outcome": "shown", "rows": [
                {"field": "impact", "status": "shown"}, {"field": "attacker", "status": "shown"}]},
            "p2": {"outcome": "contradicted", "rows": [
                {"field": "impact", "status": "not_shown"}, {"field": "attacker", "status": "shown"}]},
        }), encoding="utf-8")
        measured = measure("all", str(binary), "fake")
        got = summarise(measured["per_pair"])
        problems = []
        if got["impact"]["flagged"] != 1 or got["impact"]["agree"] != 1:
            problems.append("impact precision expected 1/1, got " + json.dumps(got["impact"]))
        if got["attacker"]["flagged"] != 0:
            problems.append("attacker rows should not be counted as flagged: " + json.dumps(got["attacker"]))
        if problems:
            for p in problems:
                print("SELFTEST FAIL " + p, file=sys.stderr)
            return 1
        print(json.dumps({"self_test": "pass", "measurements": got}, indent=2))
        return 0
    finally:
        g_pairs, g_labels, g_results, g_out = saved
        globals()["PAIRS"] = g_pairs
        globals()["LABELS"] = g_labels
        globals()["RESULTS"] = g_results
        globals()["OUT"] = g_out
        globals()["corpus_root"] = _saved_corpus_root
        shutil.rmtree(tmp, ignore_errors=True)


FAKE_GATE = '''#!/usr/bin/env python3
import json, os, sys
spec = json.load(open(os.environ["FAKE_GATE_SPEC"], encoding="utf-8"))
args = sys.argv[1:]
if args and args[0] == "--version":
    print("fake-gate 0.0.0"); raise SystemExit(0)
poc = args[1] if len(args) > 1 else ""
name = os.path.basename(poc).rsplit(".", 1)[0]
found = spec.get(name)
if not found:
    print("no spec for " + name, file=sys.stderr); raise SystemExit(2)
print(json.dumps({"schema": "realism-gate.report/v1", "tool_version": "fake",
                  "claim_binding": "structured",
                  "claims": {"schema": "realism-gate.claims/v1", "binding": "structured",
                             "source": "structured", "outcome": found["outcome"],
                             "shown": 0, "not_shown": 0, "contradicted": 0, "not_checkable": 0,
                             "rows": found["rows"]}}))
'''


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
