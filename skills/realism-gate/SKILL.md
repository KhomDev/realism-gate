---
name: realism-gate
description: Gate a proof of concept and a draft finding before writing an audit report. Use when an agent is about to claim an exploit, draft a finding, or decide a bug is payable.
---

# realism-gate

Does the PoC show what an unprivileged attacker can actually do, or only what the harness made true?

Run this before writing a report. A passing test is not a payable finding.

## Before the report

1. Check the PoC. `realism-gate check <path> --fail-on warn`, or the MCP tool `check_poc`.
2. Check the draft. `vg check <finding.md> --json`, or the MCP tool `check_finding`.
3. Do not write the report while either gate is red.
   - realism-gate: any active finding at warn or error. A note is context, not a stop, unless the user asked to fail on notes.
   - viability-gate: outcome `KILL` or `HOLD`. `INCOMPLETE` means an input was missing; say which gate did not run. `CLEAR` means no known blocker, not that the finding is right.

If the user supplied a patch or a `poc-patched/` tree, also run `realism-gate verify-fix`. The PoC must fail on the patched code. A compile error is not a failed PoC.

## What to put in the report

- Quote the finding, the file and the line. Do not paraphrase a warning into a pass.
- If you silence a finding, the `allow` comment needs a reason, and the report should carry the same reason.
- A profit number that RG15 flagged is not the headline until the fee or the gas is in it.
- A setup that RG17 flagged needs a `deployment.evidence` entry whose `source` a reader can open, or the report says the behavior is one the test constructed.

## What this does not do

The checker reads the test. It does not run it, except `verify-fix`, which runs forge locally. It does not read the chain, so a pinned-block balance under its fixed bound is not checked. It does not read a protocol's documentation; RG17 is silent only when the author cites that documentation. It does not decide a payout. viability-gate scores the write-up against rejection patterns and can miss a duplicate it was not given a corpus for.

## Commands

```bash
realism-gate check test/ --fail-on warn
vg check finding.md --json
realism-gate verify-fix --poc test/Poc.t.sol --patched poc-patched/
```

MCP tools, from `realism-gate-mcp`: `check_poc` (`path`, optional `declaration`, `fail_on`) and `check_finding` (`finding`, optional `poc`).
