#!/usr/bin/env python3
"""Build the ~50-flag DEV practice pack for external labellers.

Input: the blind human packets that `eval/p2r_packet.py` writes for DEV runs
(`<run>/human/human_packet.jsonl`). Only DEV (opened) runs are accepted; a row
from any other split is refused. Selection is fixed in advance: J and X tiers
only, 12 Solana and 38 Solidity flags (languages are never mixed in one cell),
and within each language a round-robin over rules in the order of
sha256(SEED + ':' + id). No severity and no other label is carried.

Outputs:
  --pointers FILE   committed pointer rows (no snippet text, no severity)
  --packet  FILE    the full Markdown pack with snippets (kept outside the repo:
                    the snippets are third-party code)

Usage:
  python -I eval/labeling-pack/build_pack.py --runs tmp/dev-solana-poc-v3 tmp/dev-solana-poc-v4 \
      tmp/dev-solidity-poc-v1 tmp/dev-solidity-poc-v2 \
      --pointers eval/labeling-pack/dev-pack-50.jsonl --packet <outside-repo>/DEV_PACK_50.md
"""
from __future__ import annotations
import argparse, hashlib, json
from collections import defaultdict
from pathlib import Path

SEED = "rg-labeling-pack-dev-2026-11-27"
QUOTA = {"solana": 12, "solidity": 38}
BLIND = {"severity", "label", "reader", "reason"}
POINTER_FIELDS = ["id", "corpus", "unit", "tier", "rule", "language", "file", "line", "message"]

def lang_group(lang: str) -> str:
    return "solidity" if lang == "solidity" else "solana"

def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", nargs="+", required=True)
    ap.add_argument("--pointers", required=True)
    ap.add_argument("--packet", required=True)
    a = ap.parse_args()
    rows = []
    for run in a.runs:
        run = Path(run)
        meta = json.loads((run / "meta.json").read_text(encoding="utf-8"))
        splits = meta.get("split") or meta.get("splits")
        if splits != ["DEV"]:
            raise SystemExit(f"{run}: split {splits!r}; the practice pack takes DEV runs only")
        corpus = run.name.removeprefix("dev-")
        for line in (run / "human" / "human_packet.jsonl").read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            r = json.loads(line)
            if BLIND & r.keys():
                raise SystemExit(f"{run}: a blind field reached the packet: {sorted(BLIND & r.keys())}")
            if r.get("tier") not in ("J", "X"):
                continue
            r["corpus"] = corpus
            rows.append(r)
    by = defaultdict(lambda: defaultdict(list))
    for r in sorted(rows, key=lambda r: hashlib.sha256(f"{SEED}:{r['id']}".encode()).hexdigest()):
        by[lang_group(r["language"])][r["rule"]].append(r)
    pick = []
    for group, quota in QUOTA.items():
        rules = by[group]
        got = []
        while len(got) < quota and any(rules.values()):
            for rule in sorted(rules):
                if rules[rule] and len(got) < quota:
                    got.append(rules[rule].pop(0))
        pick += got
    Path(a.pointers).write_text("".join(json.dumps({k: r[k] for k in POINTER_FIELDS}, ensure_ascii=False) + "\n" for r in pick), encoding="utf-8", newline="\n")
    out = [f"# DEV practice pack: {len(pick)} flags (blind; DEV data only)\n",
           "Label each flag `true-positive`, `false-positive` or `unclear` with a one- or two-sentence reason, following `docs/labeling-rubric.md`. Write a CSV `id,label,reason`. These labels never enter a held-out cell.\n"]
    for r in pick:
        out.append(f"\n## `{r['id']}`: {r['rule']}, {r['corpus']} unit `{r['unit']}` ({r['tier']})\n\n`{r['file']}:{r['line']}`, rule page `docs/rules/{r['rule']}.md`\n\n**Claim:** {r['message']}\n\n```\n{r['snippet']}\n```\n")
    Path(a.packet).write_text("".join(out), encoding="utf-8", newline="\n")
    counts = defaultdict(int)
    for r in pick:
        counts[(lang_group(r["language"]), r["rule"])] += 1
    print(len(pick), dict(sorted(counts.items())))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())