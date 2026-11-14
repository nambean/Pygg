# @pygg/cli

`pygg`: route your AI coding tools (Claude Code, Codex, OpenCode) through Pygg, a local gateway that picks a model for each request from the providers you connected, and records what each request cost.

**npm installs this command only**, on Windows x64, macOS (Apple silicon and Intel) and Linux x64 (glibc): npm fetches the native program for your computer only. On Windows the [Pygg desktop app](https://github.com/nambean/smarttaskrouter) runs the gateway and is where you connect providers; on macOS and Linux, where there is no app yet, `pygg gateway add-connection` and `pygg gateway serve` do it from the terminal. Your coding tools are installed as usual. **`@dev` is a developer preview**: the native program is not code-signed, so Windows SmartScreen, macOS Gatekeeper or an antivirus may warn about it.

## Start here

1. Install and open the Pygg desktop app, and connect a provider (Providers > Connect Provider).
2. Install the command, and see where you are:

```
npm install -g @pygg/cli@dev
pygg
```

`pygg` alone shows whether the gateway is running, your providers, which tools are set up, and the next thing to type.

3. Set up a tool and start it through Pygg:

```
pygg setup claude          # Claude Code: a key of its own, kept in the system's credential store; no file changes
pygg run claude            # Claude Code through Pygg, with the model auto; plain `claude` keeps your Claude sign-in

pygg setup codex           # Codex: Pygg's provider and profile in config.toml, after a backup
pygg run codex

pygg setup opencode        # OpenCode: Pygg's provider in opencode.json, after a backup
pygg run opencode          # then choose a Pygg model with /models (pygg/auto lets Pygg choose)
```

`--dry-run` previews a setup and writes nothing; `--remove` undoes it and revokes the key Pygg made. Setup sends no prompt; requests your tool makes later may cost money, as the providers you connected charge them.

Each `pygg run` launch gets its own routing session: the gateway keeps it on the model that answered it while that model stays eligible, so the provider's prompt cache is kept. Pygg's own options go before the tool's name (`pygg run --model auto/cheap claude`); everything after it is the tool's (`pygg run claude --help` is Claude Code's help).

## See what Pygg has and does

These only read Pygg's store; they send nothing to any provider. Each takes `--json` (a document with `schemaVersion: 1`) for scripts.

```
pygg status                # is the gateway answering, and on which store
pygg doctor                # status, plus your tools and the providers that need attention
pygg providers             # saved connections: on, last check, health over 24 hours, models
pygg models --search flash # the models to ask for (CONNECTION/MODEL), with billing and price
pygg logs --failed         # recent request attempts, newest first (--limit N, --client NAME)
pygg usage --days 7        # requests, known and unknown cost, token mix, by client and model
```

Exit codes: 0 done, 1 failed, 2 wrong usage, 3 a check found a problem (`status`, `doctor`); `run` returns the tool's own.

`pygg --help` lists every command; `pygg <command> --help` says more.

## Try it without installing globally

```
npx --yes --package=@pygg/cli@dev pygg
npx --yes --package=@pygg/cli@dev pygg setup claude
npx --yes --package=@pygg/cli@dev pygg run claude
```

npx downloads the package into npm's cache and runs it. Its `--yes` only approves that download: it does not approve any change Pygg makes, and `setup` still changes your tool's configuration as the installed command does.

## If `pygg` is not found

Open a new terminal first. Then see where npm put it, and add that folder to your user PATH if it is missing:

```
where.exe pygg
npm prefix -g
```

If PowerShell refuses to run scripts, call `pygg.cmd` instead of `pygg`. `pygg --launcher-info` prints which native program the command runs and where it looked.

## How it works

`pygg` is a small Node launcher. It runs the native `pygg-cli` program from the package for your computer (`@pygg/win32-x64`, `@pygg/darwin-arm64`, `@pygg/darwin-x64` or `@pygg/linux-x64`), which npm installs with it as an optional dependency, with the same arguments and standard streams, and exits with its code. On Windows, without that package, it uses an installed Pygg app's `pygg-cli.exe` (`%LOCALAPPDATA%\Pygg`). `PYGG_CLI`, set to the full path of a `pygg-cli` program, takes precedence. Where no program is published (Alpine and other musl Linux, Windows on Arm with an Arm Node, other systems), or npm skipped it (`--omit=optional`), `pygg` says so and what to do.

On macOS and Linux the API keys Pygg makes are kept in the Keychain or the desktop keyring (Secret Service); the store is in `~/Library/Application Support/app.smarttaskrouter.desktop` or `~/.local/share/app.smarttaskrouter.desktop` (`PYGG_DATA_DIR` changes it).

Two things the command does not do: it never starts a gateway by itself (the app serves it, or `pygg gateway serve` does, on purpose), and it never takes an API key on the command line.

## Experimental: tasks from your IDE

`pygg mcp setup claude-code`, `pygg mcp setup codex` and `pygg mcp setup opencode` register Pygg's MCP server (`pygg mcp`) in the tool, so it can hand coding tasks to Pygg. That is the experimental task integration, on hold; it is not how a tool's requests go through the gateway (see Start here).

MIT licence.
