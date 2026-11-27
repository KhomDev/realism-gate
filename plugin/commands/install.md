---
description: Install or repair the realism-gate CLI pinned to this plugin's version (from crates.io)
allowed-tools: Bash(${CLAUDE_PLUGIN_ROOT}/bin/realism-gate-ensure:*), Bash(realism-gate --version)
---

Run `${CLAUDE_PLUGIN_ROOT}/bin/realism-gate-ensure`, then `realism-gate --version`, and report the
installed version and path. This downloads from crates.io through cargo; say so before running it.
