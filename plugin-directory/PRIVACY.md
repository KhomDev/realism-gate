# Privacy policy

The realism-gate Anthropic Directory plugin (the `plugin-directory/` tree in
[KhomDev/realism-gate](https://github.com/KhomDev/realism-gate)) does not
collect, store or send any data.

It ships a skill and read-only commands. It does not run the realism-gate
CLI, does not open a network connection, and has no hooks or installer. There
is no telemetry and no account.

Claude Code may read files in your project when you ask it to, using only
Read, Grep, Glob and LS. Those reads stay in your local session. They are not
sent to the plugin author.

If you run `realism-gate` or `realism-gate-mcp` yourself, that process is
yours. This plugin does not start those programs and does not receive their
output. The MCP server, if you add it, is a program you configure; this
plugin does not declare or launch it.

Questions: [github.com/KhomDev/realism-gate/issues](https://github.com/KhomDev/realism-gate/issues).
