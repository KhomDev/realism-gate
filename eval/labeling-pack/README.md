# DEV practice pack (50 flags)

`dev-pack-50.jsonl` lists 50 flags from the **DEV (opened)** runs of the release binary
(`d0d5e64`, sha256 `c6df0c3d…682775e`): 12 Solana and 38 Solidity, J and X tiers only. They are a
round-robin over rules in the order of `sha256("rg-labeling-pack-dev-2026-11-27:" + id)`. The rows
are pointers (flag ID, corpus, unit, tier, rule, file, line, the flag's message). They carry no
severity, no other label and no snippet text: snippets are third-party code, so the full pack is
rebuilt locally.

Purpose: calibrating external labellers with `docs/labeling-rubric.md`, and timing a label. These labels
**never** enter a V1, V2 or FINAL cell.

Rebuild the full pack (with snippets) from the DEV runs:
```bash
python -I eval/<corpus>/run.py target/release/realism-gate --version p2r2-dev-d0d5e64 --split DEV \
    --cache <cache> --out-dir tmp/dev-<corpus>          # solana-poc-v3, solana-poc-v4, solidity-poc-v1, solidity-poc-v2
python -I eval/p2r_packet.py --corpus <solana-v3|solana-v4|solidity-v1|solidity-v2> --run-dir tmp/dev-<corpus> --cache <cache>
python -I eval/labeling-pack/build_pack.py --runs tmp/dev-solana-poc-v3 tmp/dev-solana-poc-v4 \
    tmp/dev-solidity-poc-v1 tmp/dev-solidity-poc-v2 \
    --pointers eval/labeling-pack/dev-pack-50.jsonl --packet <outside-repo>/DEV_PACK_50.md
```
The rebuilt `dev-pack-50.jsonl` must be byte-identical to the committed one.