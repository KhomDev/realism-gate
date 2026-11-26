#!/usr/bin/env python3
"""Build a volunteer label sheet (the v2 format) for a frozen run's sample.

`p2r_packet.py` writes the machine packet the scorer reads. This script
writes what a volunteer actually receives, in the format of the v2 sheet
(`_orchestration/research/human-labels/LABEL_SHEET_v2.*`):

  LABEL_SHEET.csv   row, rule, the rule's claim, a pinned link, where, the
                    flagged line's text, repo context, empty label/reason
  LABEL_SHEET.md    the same rows with the instructions
  <key>             PRIVATE answer key: row -> flag id, and the calibration
                    answers. It must be written outside the repository.

Rows carry **neutral ids** (L01, L02, ...) in a seeded shuffle: no flag id,
unit id, tier, source, severity or reader label reaches the volunteer.
Five **calibration rows** with known answers are mixed in. They come from
DEV flags (an opened run plus its resolved two-reader labels), never from
the sealed sample, and are chosen by a seeded draw that takes at least two
true positives and two false positives.

`--unblind` turns a returned sheet back into the scorer's `id,label,reason`
CSV (`p2r_score_human.py --human`), drops the calibration rows, and reports
the volunteer's calibration accuracy.

`--self-test` exercises every path on an in-memory fixture and reads no
corpus.

Usage:
  python -I eval/p2r_sheet.py --corpus solana-v3 --run-dir <final-run> \
      --cal-run-dir <dev-run> --cal-labels <dev labels-AB-adjudicated.jsonl> \
      --out-dir <dir> --key <path outside the repo> --seed <committed seed>
  python -I eval/p2r_sheet.py --unblind <returned.csv> --key <key> --out <human.csv>
  python -I eval/p2r_sheet.py --self-test
"""

from __future__ import annotations

import argparse
import csv
import io
import json
import random
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent
sys.path.insert(0, str(HERE))

LABEL_WORDS = {"tp": "true-positive", "fp": "false-positive", "unclear": "unclear"}
HEADER = [
    "row", "rule", "what the rule claims", "open this", "where", "flagged line",
    "repo context", "label (TP / FP / Unclear)", "one-line reason",
]
# Fields that must never appear in anything the volunteer receives.
FORBIDDEN = {"id", "unit", "tier", "source", "severity", "reader", "label_ai", "split"}
N_CAL = 5


def load_jsonl(path: Path) -> list[dict]:
    return [json.loads(x) for x in path.read_text(encoding="utf-8").splitlines() if x.strip()]


def github_repo(repo: str) -> str:
    repo = repo.rstrip("/")
    if repo.startswith("https://github.com/"):
        repo = repo[len("https://github.com/"):]
    return repo


def locate(pin: dict, line: int) -> tuple[str, str, str]:
    """(pinned link, where, repo context) for a flag's line in its unit's pin."""
    kind = pin.get("kind")
    if kind == "repo-file" or kind == "raw-file":
        repo = github_repo(pin["repo"])
        link = f"https://github.com/{repo}/blob/{pin['commit']}/{pin['path']}#L{line}"
        return link, f"line {line}", f"https://github.com/{repo}/tree/{pin['commit']}"
    if kind == "report-section-block":
        repo = github_repo(pin["repo"])
        link = f"https://github.com/{repo}/blob/{pin['commit']}/{pin['path']}"
        where = (f"section {pin['section']}, code block {int(pin['block']) + 1} (counting from 1) "
                 f"of that section; the flagged line is quoted in the next column")
        return link, where, pin.get("issue_url") or f"https://github.com/{repo}/tree/{pin['commit']}"
    if kind in ("issue-block", "gh-issue-block"):
        where = (f"code block {int(pin['block']) + 1} (counting from 1) of the issue; "
                 f"the flagged line is quoted in the next column")
        return pin["url"], where, pin["url"]
    if kind in ("page-json-block", "ch-page-json-block"):
        where = (f"submission {pin.get('issue_id', '?')}, code block {int(pin['block']) + 1} "
                 f"(counting from 1); the flagged line is quoted in the next column")
        return pin["url"], where, pin["url"]
    if kind == "page-pre-block":
        where = (f"finding {pin.get('finding', '?')}, code block {int(pin['pre_index']) + 1} "
                 f"(counting from 1); the flagged line is quoted in the next column")
        return pin["url"], where, pin["url"]
    raise SystemExit(f"unknown pin kind {kind!r}: add it to locate() before building a sheet")


def flagged_line(path: Path | None, line: int) -> str:
    if path is None:
        return "(source not found)"
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return "(source not readable)"
    if 1 <= line <= len(lines):
        return lines[line - 1].strip()[:160]
    return f"(line {line} out of range)"


def resolve_labels(rows: list[dict]) -> dict[str, dict]:
    """A and B agree, or an adjudicator decided. Anything else is unresolved."""
    by: dict[str, dict] = {}
    for r in rows:
        by.setdefault(r["id"], {})[r["reader"]] = r
    out = {}
    for fid, rd in by.items():
        if "adjudicator" in rd:
            out[fid] = rd["adjudicator"]["label"]
        elif "A" in rd and "B" in rd and rd["A"]["label"] == rd["B"]["label"]:
            out[fid] = rd["A"]["label"]
    return out


def choose_calibration(cands: list[dict], seed: str) -> list[dict]:
    """Seeded draw of N_CAL resolved DEV flags with >= 2 TP and >= 2 FP."""
    tp = sorted((c for c in cands if c["answer"] == "true-positive"), key=lambda c: c["flag"])
    fp = sorted((c for c in cands if c["answer"] == "false-positive"), key=lambda c: c["flag"])
    if len(tp) < 2 or len(fp) < 2 or len(tp) + len(fp) < N_CAL:
        raise SystemExit(f"calibration needs >=2 TP and >=2 FP resolved DEV flags; have {len(tp)}/{len(fp)}")
    rng = random.Random(seed + "/calibration")
    pick = rng.sample(tp, 2) + rng.sample(fp, 2)
    rest = [c for c in tp + fp if c not in pick]
    pick.append(rng.choice(rest))
    return pick


def build(packet: list[dict], cal: list[dict], claims: dict[str, str], seed: str):
    rows = [dict(r, kind="packet") for r in packet] + [dict(r, kind="calibration") for r in cal]
    rows.sort(key=lambda r: r["flag"])
    random.Random(seed).shuffle(rows)
    for i, r in enumerate(rows, 1):
        r["row"] = f"L{i:02d}"
    buf = io.StringIO()
    w = csv.writer(buf, lineterminator="\n")
    w.writerow(HEADER)
    for r in rows:
        w.writerow([r["row"], r["rule"], claims.get(r["rule"], r["rule"]), r["link"], r["where"],
                    r["line_text"], r["context"], "", ""])
    rules = sorted({r["rule"] for r in rows})
    md = [
        f"# realism-gate: {len(rows)} flags to label",
        "",
        "Each row is one place where realism-gate, an offline checker of exploit PoC tests, raised a",
        "flag. Your job is to say whether the flag is right.",
        "",
        "## How to label",
        "",
        "- **TP**: the PoC really has the weakness the rule claims, and it weakens what the PoC proves.",
        "- **FP**: the pattern is there, but the claim is wrong in context (the \"privileged\" key is the",
        "  attacker's own key or its own account's authority; the step is setup, not the exploit; the",
        "  test is a secure-path control; an assertion does exist).",
        "- **Unclear**: the code doesn't decide it. Add one line saying why. Use it sparingly.",
        "",
        "Open the link, find the flagged line (its text is in the sheet), and read the whole test",
        "around it. Use the repo-context link if you need the program or the finding.",
        "",
        "## Keep it blind",
        "",
        "- Label alone, without discussing rows with anyone until you're done.",
        "- Don't open realism-gate's docs, eval folders or articles until you've sent the sheet back.",
        "- Some rows are calibration checks with known answers.",
        "",
        "## Rules in this sheet",
        "",
    ] + [f"- **{k}**: {claims.get(k, k)}" for k in rules] + [
        "",
        "## Rows",
        "",
        "| row | rule | open this | where | flagged line |",
        "|---|---|---|---|---|",
    ] + [f"| {r['row']} | {r['rule']} | [link]({r['link']}) ([context]({r['context']})) | {r['where']} | "
         f"`{r['line_text'].replace('|', '/').replace('`', chr(39))}` |" for r in rows] + [
        "",
        "Send back the CSV with a label and a one-line reason per row, and whether you'd like to be",
        "credited by name or stay anonymous.",
    ]
    key = {"seed": seed, "rows": {r["row"]: {"kind": r["kind"], "flag_id": r["flag"], "rule": r["rule"],
           **({"answer": r["answer"]} if r["kind"] == "calibration" else {})} for r in rows}}
    sheet_csv = buf.getvalue()
    sheet_md = "\n".join(md) + "\n"
    for text in (sheet_csv, sheet_md):
        for r in rows:
            if r["flag"] in text:
                raise SystemExit(f"sheet would leak flag id {r['flag']}")
    return sheet_csv, sheet_md, json.dumps(key, indent=1, sort_keys=True) + "\n"


def unblind(returned: str, key: dict) -> tuple[str, dict]:
    rows = list(csv.DictReader(io.StringIO(returned)))
    label_col = next((c for c in (rows[0].keys() if rows else []) if c.startswith("label")), None)
    reason_col = next((c for c in (rows[0].keys() if rows else []) if "reason" in c), None)
    if label_col is None:
        raise SystemExit("returned sheet has no label column")
    out = io.StringIO()
    w = csv.writer(out, lineterminator="\n")
    w.writerow(["id", "label", "reason"])
    cal_ok = cal_n = 0
    seen = set()
    for r in rows:
        k = key["rows"].get(r["row"])
        if k is None:
            raise SystemExit(f"row {r['row']} is not in the key")
        seen.add(r["row"])
        word = LABEL_WORDS.get((r.get(label_col) or "").strip().lower())
        if word is None:
            raise SystemExit(f"row {r['row']}: label must be TP, FP or Unclear")
        if k["kind"] == "calibration":
            cal_n += 1
            cal_ok += word == k["answer"]
            continue
        w.writerow([k["flag_id"], word, (r.get(reason_col) or "").strip() if reason_col else ""])
    missing = sorted(set(key["rows"]) - seen)
    if missing:
        raise SystemExit(f"returned sheet is missing rows {missing}")
    return out.getvalue(), {"calibration_correct": cal_ok, "calibration_rows": cal_n}


def inside_repo(path: Path) -> bool:
    try:
        path.resolve().relative_to(ROOT.resolve())
        return True
    except ValueError:
        return False


def rows_from_run(corpus, run_dir: Path, cache: Path, flag_ids: set[str] | None):
    import p2r_corpora  # noqa: E402

    units = corpus.units()
    out = []
    for flag in load_jsonl(run_dir / "sample.jsonl"):
        if flag_ids is not None and flag["id"] not in flag_ids:
            continue
        unit = units[flag["unit"]]
        link, where, ctx = locate(unit["files"][0], int(flag["line"]))
        src = p2r_corpora.source_for_flag(corpus, run_dir, unit["id"], flag["file"], cache)
        out.append({"flag": flag["id"], "rule": flag["rule"], "link": link, "where": where,
                    "context": ctx, "line_text": flagged_line(src, int(flag["line"]))})
    return out


def claims_table() -> dict[str, str]:
    import p2r_corpora  # noqa: E402

    # The claim is the rule page's title sentence ("# RG02 `slug`: <claim>"). The
    # page body states the shipped severity, which the sheet must not carry.
    out = {}
    for n in range(1, 21):
        rule = f"RG{n:02d}"
        title = p2r_corpora.rule_definition(rule).splitlines()[0]
        claim = title.split(": ", 1)[1].strip() if title.startswith("# ") and ": " in title else rule
        if "severity" in claim.lower() or claim.startswith("<!--"):
            raise SystemExit(f"{rule}: rule title is not a neutral claim: {claim!r}")
        out[rule] = claim
    return out


def self_test() -> int:
    pins = [
        {"kind": "repo-file", "repo": "https://github.com/o/r", "commit": "c" * 40, "path": "t/a.t.sol"},
        {"kind": "issue-block", "url": "https://github.com/o/r/issues/7", "block": 2},
        {"kind": "report-section-block", "repo": "https://github.com/o/f", "commit": "d" * 40,
         "path": "report.md", "section": "H-01", "block": 0, "issue_url": "https://github.com/o/f/issues/3"},
        {"kind": "page-pre-block", "url": "https://web.archive.org/x", "finding": "M-2", "pre_index": 1},
    ]
    assert locate(pins[0], 12)[0].endswith("/blob/" + "c" * 40 + "/t/a.t.sol#L12")
    assert "code block 3" in locate(pins[1], 4)[1]
    assert "section H-01" in locate(pins[2], 4)[1]
    assert "code block 2" in locate(pins[3], 1)[1]
    try:
        locate({"kind": "mystery"}, 1)
        raise AssertionError("unknown pin kind accepted")
    except SystemExit:
        pass
    labels = [
        {"id": "a", "reader": "A", "label": "true-positive"}, {"id": "a", "reader": "B", "label": "true-positive"},
        {"id": "b", "reader": "A", "label": "false-positive"}, {"id": "b", "reader": "B", "label": "true-positive"},
        {"id": "b", "reader": "adjudicator", "label": "false-positive"},
        {"id": "c", "reader": "A", "label": "true-positive"}, {"id": "c", "reader": "B", "label": "false-positive"},
    ]
    res = resolve_labels(labels)
    assert res == {"a": "true-positive", "b": "false-positive"}, res
    mk = lambda f, rule, ans=None: {"flag": f, "rule": rule, "link": f"https://x/{len(f)}", "where": "line 1",
                                    "context": "https://x", "line_text": "vm.prank(owner);",
                                    **({"answer": ans} if ans else {})}
    cands = [mk(f"cal{i}", "RG08", "true-positive" if i < 3 else "false-positive") for i in range(6)]
    cal = choose_calibration(cands, "seed")
    assert len(cal) == N_CAL and sum(c["answer"] == "true-positive" for c in cal) >= 2
    assert sum(c["answer"] == "false-positive" for c in cal) >= 2
    assert choose_calibration(cands, "seed") == cal, "calibration draw is not deterministic"
    packet = [mk(f"flag{i:02d}abcdef", "RG02") for i in range(7)]
    a = build(packet, cal, {"RG02": "claim two", "RG08": "claim eight"}, "s1")
    b = build(packet, cal, {"RG02": "claim two", "RG08": "claim eight"}, "s1")
    assert a == b, "sheet is not deterministic"
    sheet_csv, sheet_md, key_text = a
    for leak in ("flag00abcdef", "true-positive", "false-positive"):
        assert leak not in sheet_csv and leak not in sheet_md, leak
    key = json.loads(key_text)
    assert len(key["rows"]) == 12 and sum(v["kind"] == "calibration" for v in key["rows"].values()) == 5
    # A volunteer fills every row TP, except calibration rows answered correctly.
    filled = list(csv.reader(io.StringIO(sheet_csv)))
    head, body = filled[0], filled[1:]
    for r in body:
        k = key["rows"][r[0]]
        r[7] = {"true-positive": "TP", "false-positive": "FP"}[k["answer"]] if k["kind"] == "calibration" else "TP"
        r[8] = "reason"
    buf = io.StringIO()
    csv.writer(buf, lineterminator="\n").writerows([head] + body)
    human, stats = unblind(buf.getvalue(), key)
    hrows = list(csv.DictReader(io.StringIO(human)))
    assert len(hrows) == 7 and {r["id"] for r in hrows} == {p["flag"] for p in packet}
    assert all(r["label"] == "true-positive" for r in hrows)
    assert stats == {"calibration_correct": 5, "calibration_rows": 5}, stats
    # A missing row and a bad label are refused.
    for broken in (buf.getvalue().rsplit("\n", 2)[0] + "\n", buf.getvalue().replace(",TP,", ",maybe,", 1)):
        try:
            unblind(broken, key)
            raise AssertionError("broken sheet accepted")
        except SystemExit:
            pass
    assert inside_repo(ROOT / "eval" / "x.json") and not inside_repo(ROOT.parent / "elsewhere.json")
    print("self-test ok: locate 4 kinds, resolve, calibration draw, determinism, no leak, unblind, refusals")
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--self-test", action="store_true")
    ap.add_argument("--corpus", default="solana-v3")
    ap.add_argument("--run-dir")
    ap.add_argument("--cache")
    ap.add_argument("--cal-run-dir")
    ap.add_argument("--cal-cache")
    ap.add_argument("--cal-labels")
    ap.add_argument("--out-dir")
    ap.add_argument("--key")
    ap.add_argument("--seed")
    ap.add_argument("--unblind")
    ap.add_argument("--out")
    args = ap.parse_args(argv[1:])
    if args.self_test:
        return self_test()
    if args.unblind:
        if not (args.key and args.out):
            ap.error("--unblind needs --key and --out")
        key = json.loads(Path(args.key).read_text(encoding="utf-8"))
        human, stats = unblind(Path(args.unblind).read_text(encoding="utf-8"), key)
        Path(args.out).write_text(human, encoding="utf-8", newline="\n")
        print(json.dumps(stats))
        return 0
    for need in ("run_dir", "cal_run_dir", "cal_labels", "out_dir", "key", "seed"):
        if not getattr(args, need):
            ap.error(f"--{need.replace('_', '-')} is required")
    key_path = Path(args.key)
    if inside_repo(key_path):
        raise SystemExit("the answer key must be written outside the repository")
    import p2r_corpora  # noqa: E402

    corpus = p2r_corpora.get(args.corpus)
    cache = Path(args.cache) if args.cache else corpus.cache
    packet = rows_from_run(corpus, Path(args.run_dir), cache, None)
    resolved = resolve_labels(load_jsonl(Path(args.cal_labels)))
    cal_cache = Path(args.cal_cache) if args.cal_cache else cache
    cands = rows_from_run(corpus, Path(args.cal_run_dir), cal_cache, set(resolved))
    for c in cands:
        c["answer"] = resolved[c["flag"]]
    cal = choose_calibration([c for c in cands if c["answer"] != "unclear"], args.seed)
    if {c["flag"] for c in cal} & {p["flag"] for p in packet}:
        raise SystemExit("a calibration row is also a packet row")
    sheet_csv, sheet_md, key_text = build(packet, cal, claims_table(), args.seed)
    out = Path(args.out_dir)
    out.mkdir(parents=True, exist_ok=True)
    (out / "LABEL_SHEET.csv").write_text(sheet_csv, encoding="utf-8", newline="\n")
    (out / "LABEL_SHEET.md").write_text(sheet_md, encoding="utf-8", newline="\n")
    key_path.parent.mkdir(parents=True, exist_ok=True)
    key_path.write_text(key_text, encoding="utf-8", newline="\n")
    print(f"wrote {out}/LABEL_SHEET.csv/.md ({len(packet)} flags + {len(cal)} calibration); key at {key_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
