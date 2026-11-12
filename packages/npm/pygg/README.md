# pygg

Pygg from the command line. **Developer preview: Windows x64 only, and the program is not code-signed**, so Windows SmartScreen or an antivirus may warn about it.

```
npm install -g pygg
pygg status        # is Pygg serving, on which store, and is its gateway answering
pygg doctor        # status, plus the store's schema, the agents found and whether Pygg is set up in each
pygg mcp           # the MCP server for coding agents (Claude Code, Codex CLI, OpenCode, Cursor)
pygg gateway …     # the headless gateway: serve, connections, clients, combos, usage
pygg agent …       # agent detection, one-off runs, smart tasks
```

`pygg` runs the native `pygg-cli` program from the package `@pygg/win32-x64`, which npm installs with it. Without that package it uses an installed Pygg app's `pygg-cli.exe` (`%LOCALAPPDATA%\Pygg`), or the program named by `PYGG_CLI`.

To let a coding agent start Pygg tasks, register `pygg mcp` as an MCP server with the agent, with `PYGG_TOKEN` set to a client token that may start tasks (Pygg → Clients). The Pygg app's Agents → From Your IDE does this for you.

MIT licence.
