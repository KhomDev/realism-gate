#!/usr/bin/env python3
"""Corpus registry shared by the P2R measurement scripts.

`labels.py`, `measure.py`, `p2r_packet.py` and `p2r_score_human.py` serve
more than one sealed corpus. Everything corpus-specific lives here: the
inventory path, the default pin cache, the language vocabulary (statistics
are language-neutral, the vocabulary is not), and how a flag's file is found
on disk when a run did not copy it.

The rule this file exists to enforce: Solana and Solidity are never pooled.
`check_languages` raises when a flags file carries a language that belongs to
the other corpus.

Usage:
  from p2r_corpora import CORPORA, get, check_languages
"""

from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Repository directories under `<cache>/repos/` whose name is not the repo
# basename. Solana v3 pins three repos under their owner-qualified names.
SOLANA_REPOS = {
    "Frankcastleauditor/Solana-Audit-Arena": "Frankcastleauditor-Solana-Audit-Arena",
    "Zolldyk/anchor-pinochio-security-template": "Zolldyk-anchor-pinochio-security-template",
    "CodeHawks-Contests/2025-03-rustfund": "CodeHawks-Contests-2025-03-rustfund",
}

# Solidity repo-file pins whose checkout name is owner-qualified. Mirrors
# `eval/solidity-poc-v1/fetch.py` REPO_DIRS.
SOLIDITY_REPOS = {
    "https://github.com/SunWeb3Sec/DeFiHackLabs": "SunWeb3Sec-DeFiHackLabs",
    "https://github.com/immunefi-team/bugfix-reviews-pocs": "immunefi-team-bugfix-reviews-pocs",
    "https://github.com/faheelsattar/damn-vuln-defi-foundry": "faheelsattar-damn-vuln-defi-foundry",
    "https://github.com/catcatcat8/mixbytes-ctf": "catcatcat8-mixbytes-ctf",
}


class Corpus:
    """One sealed corpus and everything a measurement script needs from it."""

    def __init__(
        self,
        name: str,
        inventory: Path,
        cache: Path,
        languages: tuple[str, ...],
        cell_languages: tuple[str, ...],
        repos: dict[str, str] | None = None,
    ) -> None:
        self.name = name
        self.inventory = inventory
        self.cache = cache
        # Languages a flag in this corpus may carry. A flag outside this set
        # is a pooled cell and is refused.
        self.languages = languages
        # Language order in the measured/results cells. Solana v3 keeps the
        # original five-slot order so its committed outputs stay byte-identical.
        self.cell_languages = cell_languages
        self.repos = repos or {}

    def load(self) -> dict:
        return json.loads(self.inventory.read_text(encoding="utf-8"))

    def units(self) -> dict[str, dict]:
        return {u["id"]: u for u in self.load()["units"]}


CORPORA = {
    "solana-v3": Corpus(
        name="solana-v3",
        inventory=ROOT / "eval" / "solana-poc-v3" / "inventory.json",
        cache=ROOT / ".corpus" / "solana-poc-v3",
        languages=("rust", "typescript", "javascript", "other"),
        cell_languages=("rust", "typescript", "javascript", "solidity", "other"),
        repos=dict(SOLANA_REPOS),
    ),
    "solidity-v1": Corpus(
        name="solidity-v1",
        inventory=ROOT / "eval" / "solidity-poc-v1" / "inventory.json",
        cache=ROOT / ".corpus" / "solidity-poc-v1",
        languages=("solidity", "other"),
        cell_languages=("solidity", "other"),
        repos=dict(SOLIDITY_REPOS),
    ),
    # P2R-2 additions (Amendment 2). Same vocabularies as their language's
    # first corpus, so a Solana round may pool v3+v4 flags (one language
    # family, one frozen binary) and a Solidity round v1+v2.
    "solana-v4": Corpus(
        name="solana-v4",
        inventory=ROOT / "eval" / "solana-poc-v4" / "inventory.json",
        cache=ROOT / ".corpus" / "solana-poc-v4",
        languages=("rust", "typescript", "javascript", "other"),
        cell_languages=("rust", "typescript", "javascript", "solidity", "other"),
    ),
    "solidity-v2": Corpus(
        name="solidity-v2",
        inventory=ROOT / "eval" / "solidity-poc-v2" / "inventory.json",
        cache=ROOT / ".corpus" / "solidity-poc-v2",
        languages=("solidity", "other"),
        cell_languages=("solidity", "other"),
    ),    # 2026-11-27 extension (DATASET_EXTENSION_RULES_P2R2.md), sealed before the
    # Round-1 freeze. Pools with v3+v4 in the Solana cells
    # (eval/P2R2_ANALYSIS_PLAN_ADDENDUM_1.md).
    "solana-v5": Corpus(
        name="solana-v5",
        inventory=ROOT / "eval" / "solana-poc-v5" / "inventory.json",
        cache=ROOT / ".corpus" / "solana-poc-v5",
        languages=("rust", "typescript", "javascript", "other"),
        cell_languages=("rust", "typescript", "javascript", "solidity", "other"),
    ),
}


def names() -> list[str]:
    return sorted(CORPORA)


def get(name: str) -> Corpus:
    try:
        return CORPORA[name]
    except KeyError:
        raise SystemExit(
            f"unknown corpus `{name}`; choose one of {', '.join(names())}"
        ) from None


def check_languages(corpus: Corpus, flags: list[dict]) -> None:
    """Refuse a flags set that carries another corpus's languages.

    The statistics are language-neutral; the vocabulary is not. A Solidity
    flag in a Solana run (or the reverse) is a pooled cell and would silently
    corrupt every cell it touches.
    """
    seen = {f.get("language") or "other" for f in flags}
    outside = sorted(seen - set(corpus.languages))
    if outside:
        owners = sorted(
            {other.name for other in CORPORA.values() if set(outside) & set(other.languages)}
        )
        where = f" (owned by {', '.join(owners)})" if owners else ""
        raise SystemExit(
            f"{corpus.name}: refusing to pool flags in languages {outside}{where}. "
            "Solana and Solidity are never pooled; pass one corpus's run."
        )


def repo_dir(repo: str, cache: Path, corpus: Corpus) -> str:
    """The checkout directory name under `<cache>/repos/` for a pin's repo."""
    if repo in corpus.repos:
        return corpus.repos[repo]
    m = re.fullmatch(r"https://github\.com/code-423n4/(.+)", repo)
    if m:
        slug = m.group(1)
        if slug.endswith("-findings"):
            return "c4-" + slug[: -len("-findings")]
        if (cache / "repos" / ("c4-code-" + slug)).is_dir():
            return "c4-code-" + slug
        return "c4-" + slug
    m = re.fullmatch(r"https://github\.com/[^/]+/(.+)", repo)
    if m:
        base = m.group(1).rstrip("/")
        for candidate in (base, base.replace("/", "-")):
            if (cache / "repos" / candidate).is_dir():
                return candidate
        return base
    raise SystemExit(f"unknown repository URL {repo}")


def source_for_flag(
    corpus: Corpus, run_dir: Path, unit_id: str, rel: str, cache: Path
) -> Path | None:
    """The file a flag points into, for the labelling packet snippet.

    The run directory's `extract/<unit>/<file>` copy is the first choice: both
    runners write the exact file the binary read. The fallbacks read the pin
    cache for units the run did not copy (Solana repo files) and for Solidy
    repo-file and raw-file pins.
    """
    extracted = run_dir / "extract" / unit_id / rel
    if extracted.is_file():
        return extracted
    try:
        unit = corpus.units().get(unit_id)
    except (OSError, json.JSONDecodeError):
        return None
    if unit is None or not unit.get("files"):
        return None
    pin = unit["files"][0]
    kind = pin.get("kind")
    if kind == "repo-file":
        directory = repo_dir(pin["repo"], cache, corpus)
        path = cache / "repos" / directory / rel
        if path.is_file():
            return path
    if corpus.name == "solidity-v1" and kind == "raw-file":
        # The raw cache flattens `/` to `_`; the flag's file is the path the
        # runner gave the binary, which is the inventory path.
        path = cache / "raw" / "salty" / str(pin["path"]).replace("/", "_")
        if path.is_file():
            return path
    return None


def rule_definition(rule: str) -> str:
    """The first forty lines of the rule's published page, or a stub."""
    path = ROOT / "docs" / "rules" / f"{rule}.md"
    if not path.is_file():
        return f"{rule}: definition not found"
    lines = path.read_text(encoding="utf-8").splitlines()
    out: list[str] = []
    for line in lines[:60]:
        out.append(line)
        if len(out) >= 40 and line.startswith("#"):
            break
    return "\n".join(out).strip()
