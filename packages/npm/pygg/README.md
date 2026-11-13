# @pygg/cli

Pygg from the command line: `pygg`. Install `@pygg/cli`; run `pygg`. **`@dev` is the Windows x64 developer preview**: the native program is not code-signed, so Windows SmartScreen or an antivirus may warn about it.

npm installs the command only. The [Pygg app](https://github.com/nambean/smarttaskrouter) still runs the gateway, and your coding tool is installed as before.

## Start here

Install the command. Open Pygg and connect a provider. Then:

```
npm install -g @pygg/cli@dev
pygg doctor                  # is Pygg serving, and what is set up in your tools
pygg setup claude-launcher   # route Claude Code's requests through Pygg
pygg run claude              # start Claude Code through Pygg, with the model auto
```

Plain `claude` keeps your Claude sign-in. For Codex: `pygg setup codex-models`, then `codex --profile pygg`. For OpenCode: `pygg setup opencode-models`, then choose Pygg in `/models`. `--dry-run` previews a setup and `--remove` undoes it.

`pygg` alone prints these steps; `pygg --help` lists every command.

## Try it without installing globally

```
npx --yes --package=@pygg/cli@dev pygg doctor
npx --yes --package=@pygg/cli@dev pygg setup claude-launcher
npx --yes --package=@pygg/cli@dev pygg run claude
```

npx downloads the package into npm's cache and runs it. `setup` still makes the same changes to your tool's configuration as the installed command.

## If `pygg` is not found

Open a new terminal first. Then see where npm put it, and add that folder to your user PATH if it is missing:

```
where.exe pygg
npm prefix -g
```

If PowerShell refuses to run scripts, call `pygg.cmd` instead of `pygg`. `pygg --launcher-info` prints which native program the command runs and where it looked.

## How it works

`pygg` is a small Node launcher. It runs the native `pygg-cli` program from the package `@pygg/win32-x64`, which npm installs with it, with the same arguments and standard streams, and exits with its code. Without that package it uses an installed Pygg app's `pygg-cli.exe` (`%LOCALAPPDATA%\Pygg`). `PYGG_CLI`, set to the full path of a `pygg-cli` program, takes precedence over both.

Two things the command does not do: it never starts a gateway by itself (the app serves it, or `pygg gateway serve` does, on purpose), and it never takes an API key on the command line.

## Experimental: tasks from your IDE

`pygg setup claude-code`, `pygg setup codex` and `pygg setup opencode` register Pygg's MCP server (`pygg mcp`) in the tool, so it can hand coding tasks to Pygg. That is the experimental task integration, on hold; it is not how a tool's requests go through the gateway (see Start here).

MIT licence.
