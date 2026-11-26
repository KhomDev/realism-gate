# P2R preregistration — Amendment 2

**Amendment 2 was committed before any sealed slice was opened. V1, V2 and FINAL are untouched:
on 2026-11-26 a whole-id grep of all 436 sealed V1/V2/FINAL ids (`eval/solana-poc-v3`: 84;
`eval/solidity-poc-v1`: 352) over every run, label, packet and measurement file under
`_tmp/`, `.corpus/`, `worktrees/*/tmp` and `worktrees/*/.corpus` found 0 matches.**

It amends `eval/P2R_PREREGISTRATION.md` (§10 verdict criteria) and, in priorart, sits beside
`eval/P2R_PREREGISTRATION_AMENDMENT_1.md`. The amendment governs P2R-2
(`_orchestration/PROMPT_P2R2_FINISH_AND_MEASURE_2026-11-26.md`).

1. **Solana unit of analysis.** There are only dozens of public, judged, coded Solana PoCs
   (see the v3 search log `eval/solana-poc-v3/SEARCH_LOG.md` and the v4 log
   `eval/solana-poc-v4/SEARCH_LOG.md`). Public data cannot reach the original Solana §10 row
   (≥ 30 distinct J in FINAL): the v3 FINAL slice holds 3 distinct judged findings. The row is
   replaced:
   - **Solana pass condition:**
     - FINAL J+X pooled precision (rules shipped above `note`): Wilson lower bound ≥ 60%,
       with ≥ 60 labelled flags from ≥ 4 source clusters, and a source-clustered bootstrap
       reported;
     - FINAL J+X readability: ≥ 85% point, lower bound ≥ 70%;
     - RG02 and RG03 each have ≥ 10 FINAL flags with point precision ≥ 70%, or ship as `note`
       with the reason;
     - every rule at `warn` or `error` meets the floor (warn: lower bound ≥ 0.50, n ≥ 10;
       error: lower bound ≥ 0.80, n ≥ 16).
   - **J-only cells** are reported as counts with Wilson intervals, labelled "descriptive:
     insufficient public judged data", and do not enter the verdict.
2. **New tiers.**
   - **A** (auditor-verified: an audit firm's finding with code and a sponsor status) and
     **AJ** (judged by an AI judge, e.g. CodeHawks AI First Flights).
   - Both are reported only as separate cells and never pooled with J or X.
   - AJ is used only for the invalid-PoC negative class (RQ3/RQ4).
3. **Round-1 scope.** RG-A3 is accepted as partial (7 of 20 rules as queries; the rest
   procedural). It is a maintainability item, not a precision item. RG-A4 (`--execute`) moves
   to Round 2. RG-A5, RG-A6 and RG-A7 stay in Round 1.
4. **Solidity diversity.** "No platform > 30% of J" becomes "no platform > 60% of J". If it
   is not met at FINAL, the Solidity verdict can be at most PARTIAL, with the platform shares
   stated.
5. **New data sealed under this amendment.** Units added under P2R-2 §3 are assigned to
   DEV/V1/V2/FINAL by the committed salted hash before any builder reads them or any binary
   runs on them. Existing slice assignments do not change.

Everything else in the original §10 stays as written: priorart, Engineering, and the verdict
words (PRECISE AND VALUABLE / PARTIAL / NOT YET).
