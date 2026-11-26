#!/usr/bin/env python3
"""Run a frozen realism-gate binary over the Solidity v1 corpus.

The corpus stores pointers, not text. This script extracts each unit's block
from the pinned cache, verifies every hash and commit against the inventory,
writes the unit as a Foundry test file into a work directory, and runs
`realism-gate check` on it. Nothing here labels a flag or edits the corpus.

J units are Code4rena report sections, CodeHawks result-group submissions and
Hats issues. The block is selected by the content-based selector in
`build_inventory.py` (`select_sol_block` for CodeHawks/Hats; the first
`POCTEST` block of the section for Code4rena), and the stored `block_sha256`
must match before anything runs. A block that is a bare test function is
wrapped in `contract P2R_<unit>_Test { ... }`; file-level contracts, imports
and pragmas stay at file level. A block that already holds a test contract is
run as extracted.

X and N units are real test files: repo files run from their pinned checkout,
raw files from a per-unit work directory. A J unit with a pinned codebase and
the checkout present at its commit is placed in a copy of that codebase under
`test/p2r_<unit>.t.sol`, so relative imports resolve against the snapshot, and
the pristine checkout is passed as `--model-target`. The cache is never
written to. Import specs that do not resolve are recorded on the unit's row
(`unresolved_imports`), not treated as failures: the binary parses each file
independently and does not follow imports, so an absent `forge-std` cannot
change what is measured.

Outputs under `--out-dir` (default `<repo>/tmp/solidity-<version>`):
  runs/<version>.jsonl    one row per unit (readability, parse errors, model target)
  flags/<version>.jsonl   every active finding (no evidence text, only its sha256)
  sample.jsonl            the flags to label: at most --cap per rule per source,
                          ordered by sha256(seed + ':' + flag_id)
  meta.json               binary sha256, split sizes, counts, model-target coverage
  extract/<unit>/         the exact files the binary was given
  reports/<unit>.json     the full binary report per unit
  work/                   unit and codebase trees the binary ran in

Nothing here labels a flag or changes the corpus. `--check` re-extracts and
re-verifies every pin of the requested split offline and runs no binary; it
writes nothing outside a temporary directory.

Sealed slices: `--split V1|V2|FINAL` is refused unless `--round`, `--version`
and `--binary-sha256` are all given, and the binary's actual sha256 matches.

Usage:
  python -I eval/solidity-poc-v1/run.py BINARY --version p2-dev --split DEV \
      --cache <worktrees>/research-rg/.corpus/solidity-poc-v1
  python -I eval/solidity-poc-v1/run.py --check --split DEV --cache <same>
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
DEFAULT_CACHE = ROOT / ".corpus" / "solidity-poc-v1"
SEED = "rg-solidity-poc-v1-2026-11-26"
CAP = 30
SEALED = {"V1", "V2", "FINAL"}
# Directories never copied into a work tree. `lib`, `node_modules` and the
# build outputs are also what the program model skips; hidden directories and
# `.git` are never part of a source snapshot.
SKIP_DIRS = {".git", "lib", "node_modules", "out", "cache", "broadcast", "artifacts", "target"}

sys.path.insert(0, str(ROOT / "eval"))
sys.path.insert(0, str(HERE))
import build_inventory as bi  # noqa: E402  (the corpus's own selector and regexes)
import p2r_corpora  # noqa: E402  (shared repo-directory mapping)

IMPORT_OR_PRAGMA = re.compile(r"(?ms)^[ \t]*(?:import\b.*?;|pragma\s+[^;]*;)")
IMPORT_STATEMENT = re.compile(r"(?ms)^[ \t]*import\b(.*?);")
IMPORT_SPEC = re.compile(r"""["']([^"']+)["']""")
CONTRACT_HEAD = re.compile(r"(?:abstract\s+contract|contract|interface|library)\b")
TEST_FN = re.compile(r"function\s+(?:test|invariant_)\w*\s*\(")


class PinError(Exception):
    """A pinned artefact is missing or drifted from the inventory."""


# ---------------------------------------------------------------------------
# Hashing and git helpers (identical normalisation to build_inventory/fetch)
# ---------------------------------------------------------------------------


def norm_text(text: str) -> str:
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    lines = [line.rstrip() for line in text.split("\n")]
    while lines and not lines[0]:
        lines.pop(0)
    while lines and not lines[-1]:
        lines.pop()
    return "\n".join(lines)


def norm_hash(text: str) -> str:
    return hashlib.sha256(norm_text(text).encode("utf-8")).hexdigest()


def sha_file(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sha256_text(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def git_head(directory: Path) -> str | None:
    proc = subprocess.run(
        ["git", "-C", str(directory), "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    return proc.stdout.strip() if proc.returncode == 0 else None


def load_json(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


# ---------------------------------------------------------------------------
# Comment/string-aware top-level scanner for Solidity fragments
# ---------------------------------------------------------------------------


def strip_comments_strings(source: str) -> str:
    """Blank comments and string bodies while preserving offsets and newlines."""
    out: list[str] = []
    i = 0
    n = len(source)
    while i < n:
        c = source[i]
        if c == "/" and i + 1 < n and source[i + 1] == "/":
            j = source.find("\n", i)
            if j < 0:
                break
            out.append(" " * (j - i))
            i = j
        elif c == "/" and i + 1 < n and source[i + 1] == "*":
            j = source.find("*/", i + 2)
            j = n if j < 0 else j + 2
            out.append("".join("\n" if ch == "\n" else " " for ch in source[i:j]))
            i = j
        elif c in "\"'":
            quote = c
            j = i + 1
            while j < n:
                if source[j] == "\\":
                    j += 2
                    continue
                if source[j] == quote:
                    break
                j += 1
            j = min(j + 1, n)
            out.append(quote + "".join(
                "\n" if ch == "\n" else " " for ch in source[i + 1 : j - 1]
            ) + quote)
            i = j
        else:
            out.append(c)
            i += 1
    return "".join(out)


def top_items(source: str) -> list[tuple[int, int, str]]:
    """File-level items of a Solidity fragment: `;` or a balanced `{...}`."""
    stripped = strip_comments_strings(source)
    items: list[tuple[int, int, str]] = []
    depth = 0
    start: int | None = None
    for index, ch in enumerate(stripped):
        if depth == 0 and start is None and ch not in " \t\r\n":
            start = index
        if ch == "{":
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0 and start is not None:
                items.append((start, index + 1, source[start : index + 1]))
                start = None
            elif depth < 0:
                # A stray closing brace (a quoted or truncated block). Treat it
                # as an item boundary rather than poisoning every later item.
                depth = 0
                start = None
        elif ch == ";" and depth == 0 and start is not None:
            items.append((start, index + 1, source[start : index + 1]))
            start = None
    if start is not None:
        items.append((start, len(source), source[start:]))
    return items


def contract_holds_test(code: str) -> bool:
    """Whether a file-level contract/interface/library contains a test function."""
    for _, _, item in top_items(code):
        if CONTRACT_HEAD.match(item.lstrip()) and TEST_FN.search(item):
            return True
    return False


def safe_name(unit_id: str) -> str:
    return re.sub(r"[^A-Za-z0-9]+", "_", unit_id).strip("_")


def wrap_fragment(code: str, unit_id: str) -> str:
    """Wrap a bare-function block in a test contract.

    Imports and pragmas are hoisted to file level; file-level contract-like
    declarations stay outside the wrapper, so helper contracts keep working.
    Everything else (bare functions, state variables) moves inside.
    """
    hoisted: list[str] = []

    def grab(match: re.Match[str]) -> str:
        hoisted.append(match.group(0).strip())
        return ""

    rest = IMPORT_OR_PRAGMA.sub(grab, code)
    contracts: list[str] = []
    body: list[str] = []
    for _, _, item in top_items(rest):
        if CONTRACT_HEAD.match(item.lstrip()):
            contracts.append(item.strip())
        else:
            body.append(item.strip())
    parts: list[str] = []
    if hoisted:
        parts.append("\n".join(hoisted))
    if contracts:
        parts.append("\n\n".join(contracts))
    parts.append("contract P2R_" + safe_name(unit_id) + "_Test {\n" + "\n".join(body) + "\n}")
    return "\n\n".join(parts) + "\n"


def import_specs(code: str) -> list[str]:
    specs: list[str] = []
    for match in IMPORT_STATEMENT.finditer(code):
        specs.extend(IMPORT_SPEC.findall(match.group(1)))
    return specs


def unresolved_imports(code: str, base: Path, rel: str) -> tuple[int, list[str]]:
    """Resolve each import spec against the snapshot; return (total, unresolved)."""
    file_dir = (base / rel).parent
    specs = import_specs(code)
    missing: list[str] = []
    for spec in specs:
        if spec.startswith("."):
            found = (file_dir / spec).is_file()
        else:
            found = any(
                (base / prefix / spec).is_file()
                for prefix in ("", "lib", "node_modules", "src")
            )
        if not found:
            missing.append(spec)
    return len(specs), sorted(set(missing))


# ---------------------------------------------------------------------------
# Pin extraction (every function verifies the inventory's hashes)
# ---------------------------------------------------------------------------


def c4_block(unit: dict, pin: dict, cache: Path) -> str:
    directory = cache / "repos" / ("c4-" + unit["source"])
    if not directory.is_dir():
        raise PinError("missing findings checkout c4-" + unit["source"])
    head = git_head(directory)
    if head != pin["commit"]:
        raise PinError(f"findings repo HEAD {head} != pin {pin['commit']}")
    report_path = directory / pin["path"]
    if not report_path.is_file():
        raise PinError(f"missing {pin['path']}")
    if sha_file(report_path) != pin["report_sha256"]:
        raise PinError("report.md hash drift")
    text = report_path.read_text(encoding="utf-8", errors="replace")
    sections = [m for m in bi.SEC.finditer(text)
                if f"{m.group(1)}-{m.group(2)}" == pin["section"]]
    if not sections:
        raise PinError(f"section {pin['section']} missing")
    blocks = bi.FENCE.findall(sections[0].group(5))
    if pin["block"] >= len(blocks):
        raise PinError(f"block {pin['block']} missing ({len(blocks)} blocks)")
    code = blocks[pin["block"]]
    if norm_hash(code) != pin["block_sha256"]:
        raise PinError("PoC block hash mismatch")
    # Reproduce the builder's content-based choice; the stored index must be
    # the one the selector would pick from the same pinned report.
    chosen = next((i for i, block in enumerate(blocks) if bi.POCTEST.search(block)), None)
    if chosen != pin["block"]:
        raise PinError(f"selector picks block {chosen}, inventory pins {pin['block']}")
    return code


def codehawks_block(unit: dict, pin: dict, cache: Path) -> str:
    slug = unit["source"][len("codehawks-"):]
    page_path = cache / "codehawks" / f"{slug}.html"
    if not page_path.is_file():
        raise PinError(f"missing CodeHawks page {slug}.html")
    if sha_file(page_path) != pin["page_sha256"]:
        raise PinError("CodeHawks page hash drift")
    text = page_path.read_text(encoding="utf-8", errors="replace")
    marker = text.find("findings.getFindingOverviewsForCompetition")
    if marker < 0:
        raise PinError("CodeHawks results payload not found")
    start = text.find(">", marker)
    end = text.find("</script>", start)
    result = json.loads(json.loads(text[start + 1 : end])["body"])[0]["result"]["data"]
    groups = (result.get("highs") or []) + (result.get("mediums") or []) + (result.get("lows") or [])
    group = next((g for g in groups if g["id"] == pin["group"]), None)
    if group is None:
        raise PinError(f"group {pin['group']} missing")
    issue = next((x for x in group.get("issues", []) if x["id"] == pin["issue_id"]), None)
    if issue is None:
        raise PinError(f"issue {pin['issue_id']} missing")
    selected = bi.select_sol_block(issue.get("description") or "")
    if not selected or selected["block"] != pin["block"]:
        raise PinError("content-based selector no longer selects the pinned block")
    if norm_hash(selected["code"]) != pin["block_sha256"]:
        raise PinError("CodeHawks block hash mismatch")
    return selected["code"]


def hats_block(unit: dict, pin: dict, cache: Path) -> str:
    found = None
    for path in sorted((cache / "hats").glob("*.json")):
        for issue in json.loads(path.read_text(encoding="utf-8")):
            if issue.get("number") == pin["issue"]:
                found = issue
    if found is None:
        raise PinError(f"Hats issue {pin['issue']} missing")
    body = found.get("body") or ""
    if hashlib.sha256(body.encode("utf-8")).hexdigest() != pin["body_sha256"]:
        raise PinError("Hats issue body drift")
    selected = bi.select_sol_block(body)
    if not selected or selected["block"] != pin["block"]:
        raise PinError("content-based selector no longer selects the pinned block")
    if norm_hash(selected["code"]) != pin["block_sha256"]:
        raise PinError("Hats block hash mismatch")
    return selected["code"]


def repo_file(pin: dict, cache: Path) -> tuple[Path, str, str]:
    directory = cache / "repos" / p2r_corpora.repo_dir(pin["repo"], cache, p2r_corpora.get("solidity-v1"))
    if not directory.is_dir():
        raise PinError(f"missing checkout {directory.name}")
    head = git_head(directory)
    if head != pin["commit"]:
        raise PinError(f"repo HEAD {head} != pin {pin['commit']}")
    path = directory / pin["path"]
    if not path.is_file():
        raise PinError(f"missing {pin['path']}")
    text = path.read_text(encoding="utf-8", errors="replace")
    if norm_hash(text) != pin["sha256_norm"]:
        raise PinError("file hash mismatch")
    return directory, pin["path"], text


def raw_file(pin: dict, cache: Path) -> tuple[str, str]:
    path = cache / "raw" / "salty" / str(pin["path"]).replace("/", "_")
    if not path.is_file():
        raise PinError(f"missing raw file {path.name}")
    text = path.read_text(encoding="utf-8", errors="replace")
    if norm_hash(text) != pin["sha256_norm"]:
        raise PinError("raw file hash mismatch")
    return pin["path"], text


def extract_code(unit: dict, cache: Path) -> dict:
    """Extract one unit's pinned content. Raises PinError on any drift."""
    pin = unit["files"][0]
    kind = pin["kind"]
    code: str
    if kind == "report-section-block":
        code = c4_block(unit, pin, cache)
    elif kind == "ch-page-json-block":
        code = codehawks_block(unit, pin, cache)
    elif kind == "gh-issue-block":
        code = hats_block(unit, pin, cache)
    elif kind == "repo-file":
        base, rel, code = repo_file(pin, cache)
        return {"pin_kind": kind, "code": code, "repo_base": base, "repo_rel": rel}
    elif kind == "raw-file":
        rel, code = raw_file(pin, cache)
        return {"pin_kind": kind, "code": code, "raw_rel": rel}
    else:
        raise PinError(f"unknown pin kind {kind}")
    return {"pin_kind": kind, "code": code}


# ---------------------------------------------------------------------------
# Codebase checkouts (the program model's --model-target)
# ---------------------------------------------------------------------------


def codebase_dir(repo: str, cache: Path) -> str:
    """The sponsor-snapshot directory for a codebase repo URL.

    Code4rena sponsor repos live under `c4-code-<slug>`; unlike a findings
    repo they never share the `c4-<slug>` findings directory, whose HEAD is a
    different commit.
    """
    match = re.fullmatch(r"https://github\.com/code-423n4/(.+)", repo)
    if match:
        return "c4-code-" + match.group(1)
    match = re.fullmatch(r"https://github\.com/[^/]+/(.+)", repo)
    if match:
        base = match.group(1).rstrip("/")
        for candidate in (base, base.replace("/", "-")):
            if (cache / "repos" / candidate).is_dir():
                return candidate
        return base
    return repo


def codebase_status(inv: dict, unit: dict, cache: Path) -> tuple[Path | None, str]:
    """(checkout, status). Status is passed/checkout-absent/... for the record."""
    cid = unit.get("codebase_id")
    if not cid:
        return None, "not-pinned"
    codebase = inv["codebases"].get(cid)
    if codebase is None:
        return None, "codebase-id-not-in-map"
    directory = cache / "repos" / codebase_dir(codebase["repo"], cache)
    if not directory.is_dir():
        return None, "checkout-absent"
    head = git_head(directory)
    if head != codebase["commit"]:
        return None, "checkout-drift"
    return directory, "passed"


def copy_codebase(source: Path, destination: Path) -> None:
    """Copy a source snapshot once per codebase into the work tree."""

    def ignore(directory: str, names: list[str]) -> set[str]:
        return {n for n in names if n in SKIP_DIRS or n.startswith(".")}

    if destination.is_dir():
        return
    destination.parent.mkdir(parents=True, exist_ok=True)
    try:
        shutil.copytree(source, destination, ignore=ignore)
    except OSError:
        shutil.rmtree(destination, ignore_errors=True)
        raise


# ---------------------------------------------------------------------------
# Per-unit preparation and execution
# ---------------------------------------------------------------------------


class Runner:
    def __init__(self, inv: dict, cache: Path, out_dir: Path, binary: str) -> None:
        self.inv = inv
        self.cache = cache
        self.out = out_dir
        self.binary = binary
        self.copies: dict[str, Path | None] = {}
        self.copy_errors: dict[str, str] = {}

    def codebase_copy(self, cid: str, source: Path) -> Path | None:
        if cid in self.copies:
            return self.copies[cid]
        destination = self.out / "work" / "codebases" / cid
        try:
            copy_codebase(source, destination)
            self.copies[cid] = destination
        except OSError as error:
            self.copies[cid] = None
            self.copy_errors[cid] = str(error)
        return self.copies[cid]

    def prepare(self, unit: dict) -> dict:
        extracted = extract_code(unit, self.cache)
        kind = extracted["pin_kind"]
        prepared: dict = {
            "fragment": unit["kind"] == "test-fragment",
            "wrapped": False,
            "model": None,
            "model_id": None,
            "model_status": "not-attempted",
            "model_error": None,
            "imports_total": 0,
            "unresolved_imports": [],
            "copy_error": None,
        }
        if kind in ("report-section-block", "ch-page-json-block", "gh-issue-block"):
            code = extracted["code"]
            checkout, status = codebase_status(self.inv, unit, self.cache)
            prepared["model_status"] = status
            cid = unit.get("codebase_id")
            base: Path | None = None
            if checkout is not None and cid:
                tree = self.codebase_copy(cid, checkout)
                if tree is not None:
                    base = tree
                    prepared["model"] = checkout  # pristine snapshot, read-only
                    prepared["model_id"] = cid
                else:
                    prepared["copy_error"] = self.copy_errors.get(cid)
                    prepared["model_status"] = "copy-error"
            if base is None:
                base = self.out / "work" / unit["id"]
            rel = (
                "test/p2r_" + safe_name(unit["id"]) + ".t.sol"
                if prepared["model"] is not None
                else "p2r_" + safe_name(unit["id"]) + ".t.sol"
            )
            if not contract_holds_test(code):
                code = wrap_fragment(code, unit["id"])
                prepared["wrapped"] = True
            path = base / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(code, encoding="utf-8", newline="\n")
            prepared.update({"base": base, "rel": rel, "code": code})
        elif kind == "repo-file":
            base = extracted["repo_base"]
            rel = extracted["repo_rel"]
            code = extracted["code"]
            prepared.update({"base": base, "rel": rel, "code": code})
            checkout, status = codebase_status(self.inv, unit, self.cache)
            prepared["model_status"] = status
            if checkout is not None and unit.get("codebase_id"):
                prepared["model"] = checkout
                prepared["model_id"] = unit["codebase_id"]
        elif kind == "raw-file":
            base = self.out / "work" / unit["id"]
            rel = extracted["raw_rel"]
            code = extracted["code"]
            path = base / rel
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(code, encoding="utf-8", newline="\n")
            prepared.update({"base": base, "rel": rel, "code": code})
            checkout, status = codebase_status(self.inv, unit, self.cache)
            prepared["model_status"] = status
        else:  # pragma: no cover - extract_code raises first
            raise PinError(f"unknown pin kind {kind}")
        total, missing = unresolved_imports(code, prepared["base"], prepared["rel"])
        prepared["imports_total"] = total
        prepared["unresolved_imports"] = missing
        # The exact file the binary is given, for the labelling snippet.
        copy_path = self.out / "extract" / unit["id"] / prepared["rel"]
        copy_path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(prepared["base"] / prepared["rel"], copy_path)
        return prepared

    def invoke(self, prepared: dict, model: Path | None) -> subprocess.CompletedProcess:
        args = [
            self.binary,
            "check",
            prepared["rel"],
            "--no-declaration",
            "--no-config",
            "--format",
            "json",
            "--fail-on",
            "never",
        ]
        if prepared["fragment"]:
            args.append("--fragment")
        if model is not None:
            args += ["--model-target", str(model)]
        return subprocess.run(args, cwd=str(prepared["base"]), capture_output=True)

    def run(self, unit: dict) -> tuple[dict, list[dict]]:
        try:
            prepared = self.prepare(unit)
        except (PinError, OSError, ValueError, KeyError, TypeError) as error:
            return self.error_row(unit, f"extraction error: {error}"), []
        model = prepared["model"]
        proc = self.invoke(prepared, model)
        stderr = proc.stderr.decode("utf-8", "replace")
        if model is not None and proc.returncode == 2 and "--model-target" in stderr:
            message = next(
                (line for line in stderr.splitlines() if "--model-target" in line),
                "--model-target failed",
            )
            prepared["model_error"] = message.replace(str(model), "<model-target>").strip()
            proc = self.invoke(prepared, None)
            stderr = proc.stderr.decode("utf-8", "replace")
            prepared["model"] = None
            prepared["model_id"] = None
            prepared["model_status"] = "resolve-error"
        report = None
        if proc.stdout:
            try:
                report = json.loads(proc.stdout.decode("utf-8"))
            except (UnicodeDecodeError, json.JSONDecodeError):
                report = None
        # rc 2 with a report is the "no test suite found" path (the report
        # still carries parse errors); any other non-zero exit is an error.
        if report is None or proc.returncode not in (0, 2):
            stale = self.out / "reports" / f"{unit['id']}.json"
            if stale.is_file():
                stale.unlink()
            return self.error_row(unit, f"realism-gate exited {proc.returncode}: {stderr.strip()[:300]}"), []
        report_path = self.out / "reports" / f"{unit['id']}.json"
        report_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.write_bytes(proc.stdout)
        if stderr.strip():
            (self.out / "reports" / f"{unit['id']}.stderr.txt").write_text(
                stderr, encoding="utf-8", newline="\n"
            )
        summary = report["summary"]
        flags = []
        for finding in report.get("findings", []):
            if "suppressed" in finding:
                continue
            flags.append(
                {
                    "id": flag_id(unit["id"], finding),
                    "unit": unit["id"],
                    "source": unit["source"],
                    "tier": unit["tier"],
                    "split": unit.get("split"),
                    "rule": finding["rule"],
                    "slug": finding.get("slug"),
                    "severity": finding.get("severity"),
                    "language": infer_lang(finding["file"]),
                    "file": finding["file"],
                    "line": finding["line"],
                    "column": finding["column"],
                    "suite": finding.get("suite"),
                    "tests": finding.get("tests", []),
                    "evidence_sha256": sha256_text(finding["evidence"]),
                }
            )
        flags.sort(key=lambda x: (x["rule"], x["file"], x["line"], x["column"], x["id"]))
        row = {
            "unit": unit["id"],
            "source": unit["source"],
            "tier": unit["tier"],
            "split": unit.get("split"),
            "kind": unit["kind"],
            "language": unit_lang(unit),
            "files_given": [prepared["rel"]],
            "files_read": summary["files"],
            "tests_checked": summary["tests"],
            "suites_checked": summary["suites"],
            "parse_errors": len(report.get("parse_errors", [])),
            "readable": summary["tests"] > 0,
            "active_flags": len(flags),
            "wrapped": prepared["wrapped"],
            "model_target": prepared["model_id"],
            "model_target_status": "passed" if prepared["model"] is not None else prepared["model_status"],
            "model_target_error": prepared["model_error"],
            "imports_total": prepared["imports_total"],
            "unresolved_imports": prepared["unresolved_imports"],
            "copy_error": prepared["copy_error"],
        }
        return row, flags

    def error_row(self, unit: dict, message: str) -> dict:
        return {
            "unit": unit["id"],
            "source": unit["source"],
            "tier": unit["tier"],
            "split": unit.get("split"),
            "kind": unit["kind"],
            "language": unit_lang(unit),
            "files_given": [],
            "files_read": 0,
            "tests_checked": 0,
            "suites_checked": 0,
            "parse_errors": 0,
            "readable": False,
            "active_flags": 0,
            "wrapped": False,
            "model_target": None,
            "model_target_status": "not-attempted",
            "model_target_error": None,
            "imports_total": 0,
            "unresolved_imports": [],
            "copy_error": None,
            "error": message.replace(str(self.cache), "<cache>"),
        }


# ---------------------------------------------------------------------------
# Row helpers and sampling
# ---------------------------------------------------------------------------


def infer_lang(name: str) -> str:
    return "solidity" if name.lower().endswith(".sol") else "other"


def unit_lang(unit: dict) -> str:
    pin = unit["files"][0]
    if pin.get("kind") in ("repo-file", "raw-file"):
        return infer_lang(pin.get("path", ""))
    return "solidity"


def flag_id(unit: str, finding: dict) -> str:
    key = "|".join(
        [unit, finding["rule"], finding["file"], str(finding["line"]),
         str(finding["column"]), finding["evidence"]]
    )
    return sha256_text(key)[:16]


def sample(seed: str, flags: list[dict]) -> list[dict]:
    groups: dict[tuple[str, str], list[dict]] = {}
    for f in flags:
        groups.setdefault((f["source"], f["rule"]), []).append(f)
    picked: list[dict] = []
    for _, rows in sorted(groups.items()):
        rows = sorted(rows, key=lambda f: sha256_text(seed + ":" + f["id"]))
        picked.extend(rows[:CAP])
    picked.sort(key=lambda x: (x["rule"], x["unit"], x["file"], x["line"], x["column"], x["id"]))
    return picked


def write_jsonl(path: Path, rows: list[dict]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w", encoding="utf-8", newline="\n") as handle:
        for row in rows:
            handle.write(json.dumps(row, ensure_ascii=False, sort_keys=True) + "\n")


def split_sizes(inv: dict) -> str:
    text = []
    for split in ("DEV", "V1", "V2", "FINAL"):
        units = [u for u in inv["units"] if (u.get("split") or "").upper() == split]
        counted = [u for u in units if not u.get("duplicate_of")]
        tiers = {t: sum(1 for u in counted if u["tier"] == t) for t in ("J", "X", "N")}
        text.append(
            f"{split} {len(counted)}"
            + (f" (J {tiers['J']}, X {tiers['X']}, N {tiers['N']})" if counted else "")
        )
    return " | ".join(text)


def codebase_coverage(inv: dict, units: list[dict], cache: Path) -> dict:
    counts = {"pinned_units": 0, "passed": 0, "checkout-absent": 0,
              "checkout-drift": 0, "codebase-id-not-in-map": 0}
    seen: dict[str, str] = {}
    for unit in units:
        if not unit.get("codebase_id"):
            continue
        counts["pinned_units"] += 1
        cid = unit["codebase_id"]
        if cid not in seen:
            _, status = codebase_status(inv, unit, cache)
            seen[cid] = status
        status = seen[cid]
        if status in counts:
            counts[status] += 1
        else:
            counts["codebase-id-not-in-map"] += 1
    counts["codebases"] = len(seen)
    counts["codebases_present"] = sum(1 for s in seen.values() if s == "passed")
    return counts


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("binary", nargs="?", help="the frozen realism-gate binary")
    parser.add_argument("--version", default="unfrozen")
    parser.add_argument("--cache", default=str(DEFAULT_CACHE),
                        help="pin cache (default %(default)s; the research cache is "
                             "worktrees/research-rg/.corpus/solidity-poc-v1)")
    parser.add_argument("--out-dir", default=None)
    parser.add_argument("--split", default="DEV",
                        help="comma-separated slices to touch (default DEV). "
                             "V1/V2/FINAL are opened only by a frozen round run.")
    parser.add_argument("--round", default=None,
                        help="round id; required with --version and --binary-sha256 "
                             "before a sealed slice may be touched")
    parser.add_argument("--binary-sha256", default=None,
                        help="expected sha256 of BINARY; required for a sealed slice")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args(argv[1:])

    cache = Path(args.cache).resolve()
    inventory = load_json(HERE / "inventory.json")
    print(f"split sizes: {split_sizes(inventory)}")

    wanted = {part.strip().upper() for part in args.split.split(",") if part.strip()}
    unknown = wanted - {"DEV", "V1", "V2", "FINAL"}
    if unknown:
        parser.error(f"unknown split(s) {sorted(unknown)}")
    units = [u for u in inventory["units"] if (u.get("split") or "").upper() in wanted]
    if not units:
        raise SystemExit(f"no units in split {sorted(wanted)}")
    counted = [u for u in units if not u.get("duplicate_of")]
    if len(counted) != len(units):
        print(f"skipping {len(units) - len(counted)} duplicate_of unit(s)")

    sealed = wanted & SEALED
    if sealed:
        missing = [name for name, value in (
            ("--round", args.round),
            ("--version", args.version if args.version != "unfrozen" else None),
            ("--binary-sha256", args.binary_sha256),
        ) if not value]
        if missing:
            raise SystemExit(
                f"refusing to touch sealed slice(s) {sorted(sealed)}: "
                f"{', '.join(missing)} required (round protocol, §8)"
            )

    if args.check:
        # Extract and verify every pin offline; no binary runs and nothing is
        # written.
        errors = 0
        for unit in counted:
            try:
                extract_code(unit, cache)
            except (PinError, OSError, ValueError, KeyError, TypeError) as error:
                errors += 1
                print(f"{unit['id']}: {error}", file=sys.stderr)
        coverage = codebase_coverage(inventory, counted, cache)
        print(
            f"{','.join(sorted(wanted))}: {len(counted)} units extracted, "
            f"{errors} hash or pin error(s)"
        )
        print(
            f"codebase pins: {coverage['pinned_units']} units "
            f"({coverage['passed']} present at pin, "
            f"{coverage['checkout-absent']} checkout absent, "
            f"{coverage['checkout-drift']} drift, "
            f"{coverage['codebase-id-not-in-map']} id not in map); "
            f"{coverage['codebases_present']}/{coverage['codebases']} checkouts present"
        )
        return 1 if errors else 0

    if not args.binary:
        parser.error("BINARY is required unless --check")
    binary = str(Path(args.binary).resolve())
    binary_sha = sha_file(Path(binary))
    if args.binary_sha256 and binary_sha != args.binary_sha256:
        raise SystemExit(
            f"binary sha256 {binary_sha} != --binary-sha256 {args.binary_sha256}"
        )
    out_dir = Path(args.out_dir) if args.out_dir else ROOT / "tmp" / f"solidity-{args.version}"
    out_dir.mkdir(parents=True, exist_ok=True)

    coverage = codebase_coverage(inventory, counted, cache)
    print(
        f"running {sorted(wanted)}: {len(counted)} units, binary sha256 {binary_sha}, "
        f"model target present for {coverage['passed']}/{coverage['pinned_units']} pinned units"
    )

    runner = Runner(inventory, cache, out_dir, binary)
    rows: list[dict] = []
    flags: list[dict] = []
    for unit in counted:
        row, unit_flags = runner.run(unit)
        rows.append(row)
        flags.extend(unit_flags)
    rows.sort(key=lambda r: r["unit"])
    flags.sort(key=lambda f: (f["rule"], f["file"], f["line"], f["column"], f["id"]))
    ids = [f["id"] for f in flags]
    if len(ids) != len(set(ids)):
        raise SystemExit("duplicate flag ids")

    sampled = sample(SEED, [f for f in flags if f["tier"] in ("J", "X")])
    write_jsonl(out_dir / "runs" / f"{args.version}.jsonl", rows)
    write_jsonl(out_dir / "flags" / f"{args.version}.jsonl", flags)
    write_jsonl(out_dir / "sample.jsonl", sampled)

    by_tier: dict[str, int] = {}
    for row in rows:
        by_tier[row["tier"]] = by_tier.get(row["tier"], 0) + 1
    readable = sum(1 for row in rows if row["readable"])
    errors = [row["unit"] for row in rows if "error" in row]
    wrapped = sum(1 for row in rows if row.get("wrapped"))
    with_parse = sum(1 for row in rows if row["parse_errors"])
    import_missing = sum(1 for row in rows if row.get("unresolved_imports"))
    meta = {
        "schema": "realism-gate.corpus-run/v1",
        "corpus": "solidity-poc-v1",
        "version": args.version,
        "round": args.round,
        "splits": sorted(wanted),
        "binary_sha256": binary_sha,
        "seed": SEED,
        "cap": CAP,
        "units": len(rows),
        "skipped_duplicates": len(units) - len(counted),
        "by_tier": dict(sorted(by_tier.items())),
        "readable": readable,
        "flags": len(flags),
        "sampled": len(sampled),
        "wrapped_units": wrapped,
        "units_with_parse_errors": with_parse,
        "units_with_unresolved_imports": import_missing,
        "codebases": coverage,
        "errors": errors,
    }
    (out_dir / "meta.json").write_text(
        json.dumps(meta, indent=2, sort_keys=True) + "\n", encoding="utf-8", newline="\n"
    )
    print(
        f"{len(rows)} units, {readable} readable, {len(flags)} flags, "
        f"{len(sampled)} sampled, {wrapped} wrapped, {with_parse} with parse errors, "
        f"{import_missing} with unresolved imports, {len(errors)} run error(s) "
        f"-> {out_dir}"
    )
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
