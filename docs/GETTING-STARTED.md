# Getting started

> **Status:** Current · install Pygg, connect a provider and point a coding tool at it · reviewed 2026-11-14

This page goes from the installer to "my coding tool uses the model `auto`". Where a step has not been verified, the page says so. The in-app **Guide** repeats these steps with every command ready to copy. What is built and how each part was verified is in [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md).

## What you need

- **Windows 10 or 11, x64**, for the desktop app (ADR-001).
- **WebView2.** It is built into Windows 11; Windows 10 may need it from Microsoft.
- **An account with at least one provider.** Gemini, OpenRouter and Groq have free tiers; Ollama runs on your own computer. [PROVIDERS.md](PROVIDERS.md) lists every preset and its free tier.
- **A coding tool:** Claude Code, Codex CLI, OpenCode, an editor extension that calls an OpenAI-compatible API, or an SDK.

## Install

### The desktop app (Windows)

Download `Pygg_0.1.0_x64-setup.exe` (NSIS, per user) or the MSI, with `SHA256SUMS.txt`, from the [`v0.1.0-dev.1` prerelease](https://github.com/nambean/Pygg/releases/tag/v0.1.0-dev.1). Check the download against the checksum before running it:

```powershell
Get-FileHash .\Pygg_0.1.0_x64-setup.exe -Algorithm SHA256   # compare with the line in SHA256SUMS.txt
```

To build the installers yourself: `pnpm install`, then `pnpm --filter @pygg/desktop tauri:build`; they land under `apps/desktop/src-tauri/target/release/bundle`. The build requirements are in the [README](../README.md).

Run the installer. It is not code-signed, so SmartScreen says "Windows protected your PC": choose **More info → Run anyway**. It installs for the current user:

- `%LOCALAPPDATA%\Pygg\Pygg.exe`, with a Start-menu shortcut;
- `pygg-cli.exe` next to it, the command line (see [Command line](#command-line));
- the headless programs the app uses.

To uninstall, use Settings → Apps in Windows, or `uninstall.exe` in the same folder.

Not verified yet: installing on a clean machine.

### The command line only (Windows and Linux)

The `pygg` command line is published on npm as a developer preview, version `0.1.0-dev.1`, for Windows x64 and Linux x64 (ADR-037). The macOS packages are not published yet; they wait for CI.

```bash
npm install -g @pygg/cli@dev
pygg --version
```

On Windows, the command line works with the desktop app, which runs the gateway. On Linux there is no app: connect a provider and run the gateway from the terminal. The key is read from an environment variable, never from the command line.

```bash
pygg gateway add-connection --preset openrouter --key-env OPENROUTER_API_KEY
pygg gateway create-client --name my-tool     # prints a Pygg API key once
pygg gateway serve                            # in a terminal of its own
```

Not verified yet: a provider answering through `pygg gateway serve` on Linux. The Linux package has been installed from npm and run (`pygg --version`).

## First run

Open Pygg from the Start menu. The window opens on **Home**, and the gateway starts on this computer only:

```
OpenAI-compatible:  http://127.0.0.1:8741/v1
Anthropic:          http://127.0.0.1:8741
Model:              auto
```

Home shows three steps: connect a provider, set up your coding tool, work with `auto`. Nothing is sent to any provider until you connect one and a request comes through.

Pygg keeps its data in `%APPDATA%\app.smarttaskrouter.desktop`: the store, the request ledger and each web session's profile. Provider keys are not in that folder; they are in the Windows Credential Manager. Set `PYGG_DATA_DIR` to point the app at another folder, for example to try it without touching your own store.

## Connect a provider

**Providers → Connect Provider.** Choose a provider and paste its API key; Pygg reads its model list. Nothing is billed for that. The key goes into the Credential Manager and never comes back to the interface.

- **Free first.** Gemini (AI Studio), OpenRouter (`:free` models) and Groq have free tiers, with limits set by each provider. When one is used up, `auto` moves on to the next model.
- **Local.** Ollama and five other local runtimes have presets. A local model costs $0 and is counted as such.
- **Prices and facts.** `auto` needs each model's tier, price, context size and tool support. Providers → **Auto…** fills them from the models.dev catalog (also `pygg gateway lookup`). A model with a missing fact is not eligible for `auto` until the fact is set.
- **More than one account.** A connection can hold API keys of several accounts, used in turn when one is rate-limited, out of credit or rejected (Providers → a connection → API keys, ADR-038). Add only keys of separate accounts.
- **Which providers have answered live.** Gemini, Anthropic, OpenRouter, Ollama and Qwen's token plan. The others are configured from their documentation and have not answered a real request yet; [PROVIDERS.md](PROVIDERS.md) says which.
- **Web sessions** connect an AI website that has no API (Muse, ChatGPT, Perplexity, DeepSeek, Grok) through a window Pygg owns, where you sign in yourself. They are **opt-in and unofficial**, behind an account-risk warning: a site can suspend an account it takes for automated use, and one site has done so after a run of long prompts. Pygg keeps a person's pace and never answers a CAPTCHA, but the risk is yours. Use web sessions for short questions; longer work belongs on an API connection. Details: [PROVIDERS.md](PROVIDERS.md) and ADR-035.

## Set up your coding tool

**CLI Tools** sets up each tool and shows its state. Each tool gets a Pygg API key of its own, so Usage can tell the tools apart and each can have its own limit. Before you press Set Up, CLI Tools says whether `auto` has an eligible model for that tool's requests.

### Claude Code

Pointing Claude Code's own settings at another endpoint would stop it using your Claude plan. Pygg uses a launcher instead: set it up once, then start Claude Code through Pygg when you want it. Plain `claude` keeps your Claude sign-in.

```powershell
pygg setup claude --dry-run     # shows what would change
pygg setup claude
pygg run claude                 # Claude Code through Pygg, this launch only
pygg run --model auto/cheap claude
pygg run claude -p "Explain this repository"
```

Pygg's options go before `claude`; arguments after it are Claude Code's own. Claude Code may say it does not recognize the model `auto` and assume a 200k context; that is expected.

Verified: a headless Claude Code session (`claude -p`) through the gateway, including a sub-agent and `/compact`. Not verified yet: the launcher with a provider answering, and an interactive Claude Code session through the gateway.

### Codex CLI

Pygg adds a `pygg` provider and a profile to Codex's configuration, after a backup. Codex's default model is not changed; choose Pygg per run with the profile.

```powershell
pygg setup codex --dry-run
pygg setup codex
pygg run codex
codex --profile pygg -m auto/cheap
pygg setup codex --remove       # undo
```

Verified live: Codex through the Responses API, and the one-click set-up in CLI Tools.

### OpenCode

Pygg adds a provider named Pygg to `opencode.json`, after a backup. It lists `auto`, `auto/cheap` and your enabled models, and Pygg keeps the list current when models change.

```powershell
pygg setup opencode
pygg run opencode
# then in OpenCode: /models → Pygg → auto
pygg setup opencode --remove    # undo
```

Verified live: an OpenCode run through the gateway, including a combo fallback that the ledger recorded.

`pygg setup` runs the same code as the app's one-click set-up. Not verified yet: `pygg setup` itself against a real tool's configuration.

### Editors and SDKs

Cline, Continue, Roo Code and other tools that call the API from this computer: choose an OpenAI-compatible provider and fill in these fields. The API key comes from **Endpoint → API Keys**.

```
Provider:  OpenAI Compatible
Base URL:  http://127.0.0.1:8741/v1
API key:   a Pygg API key (Endpoint → API Keys)
Model:     auto
```

Tools that send requests from their own servers cannot reach `127.0.0.1`. The gateway serves three formats:

```
OpenAI Chat Completions:  POST /v1/chat/completions
OpenAI Responses:         POST /v1/responses
Anthropic Messages:       POST /v1/messages
```

A Python example (it sends a real request, recorded in Usage):

```python
from openai import OpenAI

client = OpenAI(base_url="http://127.0.0.1:8741/v1", api_key="<your Pygg API key>")
reply = client.chat.completions.create(model="auto", messages=[{"role": "user", "content": "Hello"}])
print(reply.choices[0].message.content)
```

## Work with `auto`

Use your tool as usual with the model `auto`. For each request Pygg picks the route from the models you connected, within the key's limits, and records what it cost. CLI Tools says **Working** under a tool once a request has come through it.

The model names you can use:

- `auto`: an eligible model at or above the key's minimum tier (T2 unless changed); local, free and subscription models before paid ones, then the lowest estimated cost. It chooses within your rules; it does not judge how hard a prompt is.
- `auto/cheap`: the lowest estimated cost among the models that can take the request.
- `coding`, or any combo's name: your own ordered list, from **Combos**. The next model answers when one is out of quota or unavailable. Templates build a combo from rules (Free first, Use my plans first, Local only, and others); they never fill a gap with a paid model by themselves.
- `gemini/gemini-2.5-flash`, or any `connection/model`: one model by name.

Every response says why it chose what it chose. **How auto chooses**, in Combos, simulates a route without sending anything; so does `pygg gateway auto --alias auto --tools yes`.

Fallback happens only on errors, before any answer reaches the client. Pygg does not judge answers and does not retry a poor one.

## See cost and savings

**Usage** shows every request with its cost and that cost's source: **actual** (the provider reported usage and the model has a price), **estimated** (counted from tokens at the model's price), or **unknown**. Unknown is never shown as $0 and is left out of totals; give the model a price in Providers to estimate it. A subscription's use is counted as quota, not money.

**Estimated Savings** prices the same tokens at a model you choose and says what it left out. It is an estimate at list prices. Not verified yet: Estimated Savings against real usage (it is unit-tested on a ledger built for the test).

Limits are per API key, in **Endpoint → API Keys → Limits**: a monthly limit (estimated or strict), a cap on requests in flight, a policy for models without a price, the connections the key may use, and a minimum tier for `auto`. A strict limit holds each request at the most it could cost, so the limit cannot be passed. **Policies** sets the defaults for new keys.

## Command line

The installer puts `pygg-cli.exe` next to the app. From npm, the same program is called `pygg`:

```powershell
pygg                    # where you are, and what to type next
pygg status             # is the gateway answering
pygg doctor             # status, plus your tools and the providers that need attention
pygg setup              # the tools Pygg can set up, and their state
pygg providers          # connections, their health and models
pygg logs               # recent requests
pygg usage              # requests and cost
```

Without npm: `& "$env:LOCALAPPDATA\Pygg\pygg-cli.exe" doctor`. The read commands take `--json`. Keys are never taken on the command line: `pygg gateway add-connection` reads one from an environment variable (`--key-env`). `pygg <command> --help` lists every option, and [design/CLI.md](design/CLI.md) describes every command.

## Troubleshooting

| You see | What to do |
| --- | --- |
| Connection refused, or "nothing is listening" | Start the Pygg app (it serves the gateway), or check with `pygg status`. The port is set in Endpoint → Configure. |
| HTTP 401 | The API key is wrong or revoked. Endpoint → API Keys → **Show** copies it again. A key that cannot be shown (an older key, or one made by `pygg gateway create-client` or a tool's set-up) was shown once only: create a new one. |
| HTTP 422 "No connected model is eligible for auto" | No model has every fact `auto` needs (tier, price, tools, context). Run `pygg gateway lookup`, set them in Providers → Auto…, lower the key's minimum tier, or name a model. |
| HTTP 429, or "cooling down" | A provider's rate limit or quota. `auto` and combos move on to the next model; Providers shows when it is tried again. |
| A web session says "Not sent" | Its guard is keeping the account at a person's pace, or the prompt is too long for a site. Wait for the time it gives, or use an API connection. |
| A cost shows as unknown | The model has no price. Set one in Providers, or run `pygg gateway lookup`. |
| Claude Code says it does not know the model `auto` | Expected. It assumes a 200k context and carries on. |
| Gemini CLI: "client no longer supported" | Google no longer accepts its free Google-account login from Gemini CLI. Use a Gemini API key, or connect Gemini in Providers and use it through Pygg. |

Before asking for help, run these and attach the bundle. It is redacted: no keys, tokens or prompts.

```powershell
pygg doctor
pygg diagnostics --out "$env:USERPROFILE\Desktop\pygg-diagnostics.zip"
```

Bugs go to the repository's issues. A security problem goes privately, as [SECURITY.md](SECURITY.md) says.

## FAQ

- **Does Pygg provide models?** No. It routes your requests to the providers you connect, with your own keys or local runtimes.
- **Is free unlimited?** No. Free tiers have their own limits, set by each provider; Pygg moves on to the next model when one is used up.
- **Why is a cost unknown?** The model has no price, the provider reported no usage, or a failed request may have been billed. Unknown is never counted as $0.
- **Why did it stop after a server error?** The provider may already have run and billed the request. Pygg tries another model only if the API key allows paid retries.
- **Does fallback retry a poor answer?** No. Fallback happens on errors before the answer starts; Pygg does not judge answers.
- **Can another computer use my Pygg?** No. The gateway listens on `127.0.0.1` only.
- **Does using Claude Code through Pygg use my Claude plan?** No. Through Pygg, Claude Code uses your connected providers. Plain `claude`, started without `pygg run`, keeps your plan.
- **Does Pygg have a chat of its own?** No (ADR-028). It is a gateway for the tools you already use.
- **What are Smart Tasks?** An experimental feature where IDE agents hand subtasks to Pygg over MCP and agent CLIs run them in worktrees. It is on hold; see [AGENTS.md](AGENTS.md).

## Where next

- [PROVIDERS.md](PROVIDERS.md): every preset, its endpoint, free tier and live status.
- [SECURITY.md](SECURITY.md): what Pygg protects, what you are trusted with, how to report a problem.
- [BENCHMARKS.md](BENCHMARKS.md): what Pygg changes, measured.
- [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md): what is built and how each part was verified.
- [README.md](README.md): the index of every document.
