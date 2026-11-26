#!/usr/bin/env python3
"""Build two blind labelling packets for a frozen run's sampled flags.

Reader A and reader B receive the same flags, the rule's definition and a code
snippet around the flag. Neither packet contains the rule's shipped severity or
the other reader's labels; the only difference is the packet id. The readers
write `labels.jsonl` rows: {id, reader, label, rule, reason}.

`--corpus` selects the sealed corpus the run belongs to (default
`solana-v3`). The packet content is language-neutral; the corpus only says
where the inventory and the pin cache live and how a flag's file is found.
A Solana run and a Solidity run are never mixed in one packet.

Usage:
  python eval/solana-poc-v3/labels.py --run-dir <run> --out-dir labels/v1
  python eval/solana-poc-v3/labels.py --corpus solidity-v1 --run-dir <run> --check
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
CACHE = ROOT / ".corpus" / "solana-poc-v3"
SNIPPET_BEFORE = 8
SNIPPET_AFTER = 8

sys.path.insert(0, str(ROOT / "eval"))
import p2r_corpora  # noqa: E402

# Kept for callers that imported the mapping; the registry is the source.
KNOWN_REPOS = p2r_corpora.SOLANA_REPOS


def load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def unit_file(unit: dict, flag: dict, run_dir: Path, cache: Path, corpus) -> Path | None:
    return p2r_corpora.source_for_flag(corpus, run_dir, unit["id"], flag["file"], cache)


def snippet(path: Path | None, line: int) -> tuple[str, str]:
    if path is None:
        return "", "source-not-found"
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError as e:
        return "", f"read-error: {e}"
    if line < 1 or line > len(lines):
        return "", f"line-out-of-range: {line} of {len(lines)}"
    lo = max(0, line - 1 - SNIPPET_BEFORE)
    hi = min(len(lines), line - 1 + SNIPPET_AFTER + 1)
    body = "\n".join(
        f"{n + 1:>5} | {lines[n]}" for n in range(lo, hi)
    )
    return body, "ok"


def rule_definition(rule: str) -> str:
    return p2r_corpora.rule_definition(rule)


def packet(flags: list[dict], inv: dict[str, dict], run_dir: Path, cache: Path, reader: str, corpus) -> list[dict]:
    rows = []
    for f in flags:
        unit = inv[f["unit"]]
        path = unit_file(unit, f, run_dir, cache, corpus)
        text, status = snippet(path, f["line"])
        rows.append({
            "packet": reader,
            "id": f["id"],
            "unit": f["unit"],
            "tier": f["tier"],
            "rule": f["rule"],
            "language": f["language"],
            "file": f["file"],
            "line": f["line"],
            "rule_definition": rule_definition(f["rule"]),
            "snippet_status": status,
            "snippet": text,
        })
    return rows


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--corpus", default="solana-v3", choices=p2r_corpora.names())
    ap.add_argument("--run-dir", required=True)
    ap.add_argument("--out-dir", default=None)
    ap.add_argument("--cache", default=None)
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args(argv[1:])
    corpus = p2r_corpora.get(args.corpus)
    cache = Path(args.cache) if args.cache else corpus.cache
    run_dir = Path(args.run_dir)
    flags = load_jsonl(run_dir / "sample.jsonl")
    p2r_corpora.check_languages(corpus, flags)
    inv = corpus.units()
    a = packet(flags, inv, run_dir, cache, "A", corpus)
    b = packet(flags, inv, run_dir, cache, "B", corpus)
    if [r["id"] for r in a] != [r["id"] for r in b]:
        raise SystemExit("packets differ in flags")
    if args.check:
        for reader in ("A", "B"):
            rows = [r for r in (a if reader == "A" else b)]
            if any(r["snippet_status"] != "ok" for r in rows):
                print(f"reader {reader}: {sum(r['snippet_status'] != 'ok' for r in rows)} flags without a snippet", file=sys.stderr)
        print(f"{len(flags)} flags in two packets; {len({r['rule'] for r in a})} rules")
        return 0
    out = Path(args.out_dir) if args.out_dir else run_dir / "labels"
    out.mkdir(parents=True, exist_ok=True)
    for reader, rows in (("A", a), ("B", b)):
        with open(out / f"packet-{reader}.jsonl", "w", encoding="utf-8", newline="\n") as fh:
            for row in rows:
                fh.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")
    print(f"wrote {out}/packet-A.jsonl and packet-B.jsonl ({len(flags)} flags)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
