#!/usr/bin/env python3
"""Build a blind human labelling packet from a frozen run's sampled flags.

The packet is produced only for a sample that exists: `sample.jsonl` in the
run directory, written by `run.py`. Each flag appears with its unit, tier,
file and a code snippet. The packet deliberately carries **no rule, no shipped
severity and no other reader's label**: a human reader judges the code, not
the rule's claim.

Outputs (under `--out-dir`, default `<run>/human`):
  HUMAN_PACKET.md      the human-facing packet, snippets in fenced blocks
  human_packet.jsonl   the same rows, machine-readable, for the scorer

`--check` regenerates both files in memory and compares them byte-for-byte;
it names the packet defective if a forbidden field appears.

Usage:
  python eval/p2r_packet.py --run-dir <run>
  python eval/p2r_packet.py --corpus solidity-v1 --run-dir <run> --check
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))
import p2r_corpora  # noqa: E402

SNIPPET_BEFORE = 8
SNIPPET_AFTER = 8
BLIND_FIELDS = {"rule", "severity", "label", "reader", "reason", "rule_definition"}
FENCE = {"rust": "rust", "typescript": "ts", "javascript": "js", "solidity": "solidity", "other": ""}


def load_jsonl(path: Path) -> list[dict]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]


def snippet(path: Path | None, line: int) -> tuple[str, str]:
    if path is None:
        return "", "source-not-found"
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError as error:
        return "", f"read-error: {error}"
    if line < 1 or line > len(lines):
        return "", f"line-out-of-range: {line} of {len(lines)}"
    lo = max(0, line - 1 - SNIPPET_BEFORE)
    hi = min(len(lines), line - 1 + SNIPPET_AFTER + 1)
    body = "\n".join(f"{n + 1:>5} | {lines[n]}" for n in range(lo, hi))
    return body, "ok"


def packet_rows(corpus, run_dir: Path, cache: Path) -> list[dict]:
    sample_path = run_dir / "sample.jsonl"
    if not sample_path.is_file():
        raise SystemExit(f"{sample_path} does not exist; a packet is produced only for a run's sample")
    flags = load_jsonl(sample_path)
    if not flags:
        raise SystemExit(f"{sample_path} is empty; nothing to label")
    p2r_corpora.check_languages(corpus, flags)
    units = corpus.units()
    rows = []
    for flag in flags:
        unit = units[flag["unit"]]
        source = p2r_corpora.source_for_flag(corpus, run_dir, unit["id"], flag["file"], cache)
        text, status = snippet(source, flag["line"])
        row = {
            "packet": "human",
            "id": flag["id"],
            "unit": flag["unit"],
            "tier": flag["tier"],
            "language": flag["language"],
            "file": flag["file"],
            "line": flag["line"],
            "snippet_status": status,
            "snippet": text,
        }
        leaked = BLIND_FIELDS & set(row)
        if leaked:
            raise SystemExit(f"{flag['id']}: packet would leak {sorted(leaked)}")
        rows.append(row)
    return rows


def markdown(corpus_name: str, rows: list[dict]) -> str:
    lines = [
        f"# Human labelling packet — {corpus_name}",
        "",
        "This packet was produced from a frozen run's sampled flags, before any",
        "FINAL labels were revealed. It is **blind**: it does not name the rule,",
        "the rule's shipped severity, or another reader's label. The snippets are",
        "the exact files the frozen binary read; the line numbers are theirs.",
        "",
        "## How to label",
        "",
        "- **true-positive** — the flagged construct is a real weakness in the",
        "  evidence this PoC offers: read in context, it makes the test prove less",
        "  than it claims.",
        "- **false-positive** — the construct is present but legitimate in context",
        "  (a control test, the attacker's own account, a program-initialised",
        "  fixture, a baseline read, an assertion the test does make).",
        "- **unclear** — the code does not decide it, or the answer needs a fact",
        "  the repository does not contain. Use this sparingly and say why.",
        "",
        "## Output",
        "",
        "Write a CSV with the header `id,label,reason` and one row per flag below",
        "(`label` is exactly `true-positive`, `false-positive` or `unclear`).",
        "Then run:",
        "",
        "```",
        "python eval/p2r_score_human.py --run-dir <run> --human <your.csv>",
        "```",
        "",
        "The scorer joins your labels to the AI readers' labels, refuses to score",
        "a missing id, and reports raw agreement, Cohen's kappa, the disagreements",
        "that need adjudication, and Wilson 95% cells.",
        "",
        f"## The {len(rows)} flags",
        "",
    ]
    for row in rows:
        fence = FENCE.get(row["language"], "")
        lines.append(f"### `{row['id']}` — unit `{row['unit']}` ({row['tier']})")
        lines.append("")
        lines.append(f"`{row['file']}:{row['line']}`")
        lines.append("")
        if row["snippet_status"] != "ok":
            lines.append(f"snippet: {row['snippet_status']}")
            lines.append("")
            continue
        lines.append(f"```{fence}")
        lines.append(row["snippet"])
        lines.append("```")
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


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
    out = Path(args.out_dir) if args.out_dir else run_dir / "human"
    rows = packet_rows(corpus, run_dir, cache)
    jsonl = "".join(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n" for row in rows)
    md = markdown(corpus.name, rows)
    outputs = {out / "HUMAN_PACKET.md": md, out / "human_packet.jsonl": jsonl}
    if args.check:
        bad = 0
        for path, text in outputs.items():
            if not path.is_file() or path.read_text(encoding="utf-8") != text:
                print(f"{path} does not reproduce", file=sys.stderr)
                bad += 1
        print(f"{len(rows)} flags — " + ("check ok" if not bad else f"{bad} mismatch(es)"))
        return 1 if bad else 0
    out.mkdir(parents=True, exist_ok=True)
    for path, text in outputs.items():
        path.write_text(text, encoding="utf-8", newline="\n")
    print(f"wrote {out}/HUMAN_PACKET.md and human_packet.jsonl ({len(rows)} flags)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
