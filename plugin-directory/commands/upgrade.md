---
description: How to upgrade realism-gate — the plugin, or the CLI to the newest crates.io release. You run the commands; this plugin does not.
allowed-tools: Read, Grep, Glob, LS
---

This plugin does not upgrade or install anything. Do not start a shell. Explain the two paths, then give the user the exact commands for the one they chose:

- **Recommended — upgrade the plugin:** `/plugin marketplace update realism-gate`, then `/plugin update realism-gate`. If they installed this Directory build, the plugin path is `plugin-directory/` in KhomDev/realism-gate; the CLI is still upgraded separately.
- **CLI, newest crates.io release:** `cargo binstall realism-gate`, or `cargo install --locked --force realism-gate`. Then `realism-gate --version`. This can run ahead of the plugin's skill text; say so.
