# Security policy

realism-gate is an offline, read-only checker. It runs no Solidity, makes no
network calls and writes only the files you ask it to. That leaves three kinds
of issue that matter.

## What counts

1. **A way to make it reach the network.** The PoCs it reads may be for
   embargoed findings, so "never sends anything anywhere" is a guarantee, not a
   preference. CI checks the dependency graph and the source for network code.
   A way around that is a security issue.
2. **A way to make it write or execute something.** Crafted Solidity or a
   crafted `realism.yaml` that makes it write outside `--output` or run code.
3. **Something published that should not be.** A fixture, doc or example that
   identifies a real, unfixed vulnerability or a private target.

## What does not count

- A wrong verdict. It is a heuristic and will be wrong; file a
  [false positive report](https://github.com/KhomDev/realism-gate/issues/new/choose).
- A crash on malformed input. That is a bug; file it as one.
- A dependency advisory. `cargo audit` and `cargo deny` run in CI.

## Reporting

Open a [private security advisory](https://github.com/KhomDev/realism-gate/security/advisories/new)
on the repository. Do not open a public issue for any of the three categories
above.

Include the version, the input that triggers it (synthetic, please), and what
happens.

## Response

This is a one-person project with no service-level commitment. I will confirm
or dispute a report, fix a confirmed one in a release, and credit the reporter
in the changelog unless asked not to.
