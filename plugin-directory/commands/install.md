---
description: How to install the realism-gate CLI (you run the command; this plugin does not)
allowed-tools: Read, Grep, Glob, LS
---

This plugin does not install anything. Do not start a shell.

Tell the user to install the latest release themselves:

    cargo binstall realism-gate

or, if they do not have cargo-binstall:

    cargo install --locked realism-gate

Then `realism-gate --version` should report 1.5.3 or later.
