---
description: How to install the realism-gate CLI (you run the command; this plugin does not)
allowed-tools: Read, Grep, Glob, LS
---

This plugin does not install anything. Do not start a shell.

Tell the user to install the latest release themselves:

    cargo binstall realism-gate

or, if they do not have cargo-binstall:

    cargo install --locked realism-gate

Then `realism-gate --version` should report 1.5.3 or later. For the MCP server, after the CLI is on PATH, they add it to their own MCP config (`claude mcp add rgate -- realism-gate-mcp`), as in docs/agents.md. The 1.5.0 server frames responses with Content-Length, which current MCP clients do not read; they need 1.5.1 or later.
