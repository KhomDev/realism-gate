---
description: Upgrade realism-gate — the plugin through the marketplace, or the CLI to the newest crates.io release
allowed-tools: Bash(${CLAUDE_PLUGIN_ROOT}/bin/realism-gate-ensure:*), Bash(realism-gate --version)
---

Explain the two upgrade paths, then do the one the user chose:

- **Recommended — upgrade the plugin:** `/plugin marketplace update realism-gate`, then
  `/plugin update realism-gate`. The new plugin version pins the matching realism-gate release, and the
  next `/realism-gate:check` installs it. Plugin and CLI stay in lockstep.
- **CLI only, newest crates.io release:** run `${CLAUDE_PLUGIN_ROOT}/bin/realism-gate-ensure --latest`,
  then `realism-gate --version`. This can run ahead of the plugin's skill text; say so.
