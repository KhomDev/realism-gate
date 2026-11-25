# Human labelling packet: the v2 held-out flags

This packet exists because every label in this study was written by an AI
reader. Nothing here claims a human has labelled the corpus. The goal is an
independent human read of the 32 held-out flags, scored against the primary
labels after the fact.

**Blind rule.** Do not open `labels/heldout.jsonl` or the round-2 table in
`RESULTS.md` until your CSV is finished. The flags below are the whole input.

## What to read

1. Reproduce the corpus with `fetch.sh` and `extract.py` (pinned in
   `corpus.json`; no result depends on the network at scoring time).
2. Read the rule's definition in `docs/rules/` and the rubric decisions in
   `PREREGISTRATION.md` §"Rubric decisions" (RG08's scope, fabricated
   preconditions, program-initialised fixtures, control tests, the attacker's
   own key, the attacker taking the role through the bug, RG10 baselines).
3. Open the file and line for each flag and read the surrounding test in
   context.

## How to label

- **true-positive** — the flag names a real defect in this PoC given the
  rule's claim: the PoC does what the rule says, and that is a weakness in the
  evidence the PoC offers.
- **false-positive** — the flagged pattern is present but the rule's claim is
  wrong (a control test, the attacker's own key, a program-initialised
  fixture, a baseline read, an assertion the test does make, and so on).
- **unclear** — the code does not decide it, or the answer depends on a fact
  the repository does not contain. Use this sparingly and say why.

## The 32 flags

| id | rule | tier | unit | file:line |
|---|---|---|---|---|
| 3e203943e856d576 | RG03 | J | J-arena-236 | x402-Meridian/tests/arena-236.ts:441 |
| 321ec47c716be2b7 | RG03 | J | J-arena-244 | x402-Meridian/tests/arena-244.ts:43 |
| df2c7c020bd22978 | RG09 | J | J-arena-244 | x402-Meridian/tests/arena-244.ts:96 |
| 2b936939a4c645bf | RG03 | J | J-arena-265 | stake-flow/tests/arena-265.ts:39 |
| 6a69f2212fd0c2be | RG03 | X | X-amm-sandwich-program | programs/amm/tests/sandwich_e2e.rs:143 |
| 25e19e0d3e87b49d | RG03 | X | X-arena-12 | stake-flow/tests/arena-12.ts:261 |
| 217402db22417a8d | RG02 | X | X-arena-99 | Zenon/tests/arena-99.ts:380 |
| 4e0f7f2672e598cc | RG08 | X | X-halborn-poc1 | pocs/src/bin/poc1.rs:18 |
| 3d93948736b2cb62 | RG08 | X | X-halborn-poc2 | pocs/src/bin/poc2.rs:19 |
| bd2020a66b55bdec | RG08 | X | X-jet-adv-02 | os-jet-adv-02/src/main.rs:8 |
| 510e58eb7b16aae5 | RG08 | X | X-jet-adv-03 | os-jet-adv-03/src/main.rs:13 |
| bc8d5364f45abca8 | RG08 | X | X-merov-account-data-match | account-data-match/poc/src/main.rs:48 |
| f2d61a9cb088c49c | RG02 | X | X-merov-account-data-match | account-data-match/poc/src/main.rs:173 |
| 4b32ac08aabcdf7f | RG08 | X | X-merov-cast_truncation | cast_truncation/poc/src/bin/poc.rs:40 |
| a558b0291fb5e400 | RG02 | X | X-merov-cast_truncation | cast_truncation/poc/src/bin/poc.rs:106 |
| 0ca33d77f98787b3 | RG02 | X | X-merov-close-account | close-account/poc/src/main.rs:72 |
| d2a9d6e673f29b82 | RG02 | X | X-merov-cosplay | cosplay/poc/src/main.rs:160 |
| a46ee4ba3b5ffc2a | RG02 | X | X-merov-cosplay | cosplay/poc/src/main.rs:190 |
| 981077a41a30d2f7 | RG08 | X | X-merov-duplicated | duplicated/poc/src/bin/poc.rs:84 |
| f0893a6bdfec9b22 | RG02 | X | X-merov-duplicated | duplicated/poc/src/bin/poc.rs:175 |
| 43185b53caffdba5 | RG02 | X | X-merov-duplicated | duplicated/poc/src/bin/poc.rs:210 |
| b087af6bbd32c491 | RG08 | X | X-merov-error-not-handled | error-not-handled/poc/src/main.rs:23 |
| 4bbfe6fbb030004a | RG02 | X | X-merov-error-not-handled | error-not-handled/poc/src/main.rs:127 |
| 72c5386187cde60c | RG02 | X | X-merov-error-not-handled | error-not-handled/poc/src/main.rs:149 |
| 1937523bb371df3f | RG08 | X | X-merov-other-bugs | other-bugs/poc/src/main.rs:27 |
| 300cc3467e9a7276 | RG02 | X | X-merov-other-bugs | other-bugs/poc/src/main.rs:128 |
| dd94a153c99fa9d0 | RG08 | X | X-merov-reinitialization | reinitialization/poc/src/main.rs:55 |
| 6e27fc44ab8215ca | RG02 | X | X-merov-reinitialization | reinitialization/poc/src/main.rs:98 |
| 4358160b56fc7177 | RG02 | X | X-merov-reinitialization | reinitialization/poc/src/main.rs:120 |
| 78e652810d26fcd0 | RG02 | X | X-merov-reinitialization | reinitialization/poc/src/main.rs:182 |
| e4242404d428b2ac | RG02 | X | X-zolldyk-p03 | patterns/03-unsafe-arithmetic/tests/exploit-demo.ts:378 |
| 5bdcde8895c5d1aa | RG02 | X | X-zolldyk-p06 | patterns/06-token-validation/tests/exploit-demo.ts:482 |

## Output

Write `labels/human_heldout.csv` with the header `id,label,reason` and one row
per flag above (`label` is exactly `true-positive`, `false-positive` or
`unclear`). Then, and only then, run:

```
python eval/solana-poc-v2/score_human.py
```

It prints raw agreement, Cohen's kappa and per-rule agreement, and refuses to
score if an id is missing. `--self-test` checks the arithmetic without reading
the corpus.
