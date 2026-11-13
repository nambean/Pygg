# Getting started

> **Status:** Current · install Pygg, connect a provider and point a coding tool at it · reviewed 2026-11-13 · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

This page goes from the installer to "my coding tool uses the model `auto`". Everything on it is built and has been run at least once; where a step has only been tested against the fake provider, or not at all, the page says so. The in-app **Guide** repeats these steps with every command ready to copy. The full list of what is verified is in [Implementation Status](IMPLEMENTATION-STATUS.md).

## What you need

- **Windows 10 or 11, x64.** macOS and Linux builds do not exist yet (ADR-001).
- **WebView2.** It is built into Windows 11; Windows 10 may need it from Microsoft.
- **An account with at least one provider.** Gemini, OpenRouter and Groq have free tiers; Ollama runs on your own computer. The presets and their free tiers are listed in [PROVIDERS.md](PROVIDERS.md).
- **A coding tool:** Claude Code, Codex CLI, OpenCode, an editor extension that calls an OpenAI-compatible API, or an SDK.

## Install

There is no public release yet. The installer is built from source:

```powershell
pnpm install
pnpm --filter @smarttaskrouter/desktop tauri:build
```

That gives `Pygg_0.1.0_x64-setup.exe` (NSIS, per user) and `Pygg_0.1.0_x64_en-US.msi` under `apps/desktop/src-tauri/target/release/bundle`. The requirements for building are in the [README](../README.md#development).

Run the installer. It is not signed during development, so Windows may ask you to confirm before it runs. It installs for the current user:

- `%LOCALAPPDATA%\Pygg\Pygg.exe`, with a Start-menu shortcut;
- `pygg-cli.exe` next to it, the command line (see [Command line](#command-line));
- the headless programs the app uses.

To uninstall, use Settings → Apps in Windows, or `uninstall.exe` in the same folder. The installer has been tried on the development PC (install, uninstall, reinstall) and on the owner's PC; a clean machine is still an open release check (ROADMAP N9).

## First run

Open Pygg from the Start menu. The window opens on **Home**, and the gateway starts on this computer only:

```
OpenAI-compatible:  http://127.0.0.1:8741/v1
Anthropic:          http://127.0.0.1:8741
Model:              auto
```

Home shows three steps: connect a provider, set up your coding tool, work with `auto`. Nothing is sent to any provider until you connect one and a request comes through.

Pygg keeps its data in `%APPDATA%\app.smarttaskrouter.desktop` (the store, the request ledger and each web session's profile). Provider keys are not in that folder: they are in the Windows Credential Manager. `PYGG_DATA_DIR` points the app at another folder, for trying it without touching your own store.

## Connect a provider

**Providers → Connect Provider.** Choose a provider, paste its API key, and Pygg reads its model list. Nothing is billed for that. The key goes into the Credential Manager and never comes back to the interface.

Some notes:

- **Free first.** Gemini (AI Studio), OpenRouter (`:free` models) and Groq have free tiers. Free tiers have their own limits, set by the provider; when one is used up, `auto` moves on to the next model.
- **Local.** Ollama and five other local runtimes have presets. A local model costs $0 and is counted as such.
- **Prices and facts.** `auto` needs each model's tier, price, context and tool support. Providers → **Auto…** fills them from the models.dev catalog (also `pygg gateway lookup`). A model with a missing fact is not eligible for `auto` until the fact is set.
- **Which providers have answered live.** Gemini, Anthropic, OpenRouter, Ollama and Qwen's token plan have. The others are configured from their documentation and have not been tried; [PROVIDERS.md](PROVIDERS.md) says which.
- **Web sessions** connect an AI website that has no API (Muse, ChatGPT, Perplexity, DeepSeek, Grok) through a window Pygg owns, where you sign in yourself. They are **opt-in and unofficial**, behind an account-risk warning: one site suspended the owner's account for three days after a long benchmark run. Pygg keeps a person's pace (one request at a time, a rest between requests, a daily limit) and never answers a CAPTCHA, but the risk is yours. Details: [PROVIDERS.md](PROVIDERS.md) and ADR-035.

## Set up your coding tool

**CLI Tools** sets up each tool and shows its state. Each tool gets a Pygg API key of its own, so Usage can tell them apart and each can have its own limit. Under each tool, before Set Up, CLI Tools says whether `auto` has an eligible model for that tool's requests.

### Claude Code

Pointing Claude Code's own settings at another endpoint would stop it using your Claude plan. Pygg uses a launcher instead: set it up once, then start Claude Code through Pygg when you want it. Plain `claude` keeps your Claude sign-in.

```powershell
pygg setup claude-launcher --dry-run     # shows what would change
pygg setup claude-launcher
pygg run claude                          # Claude Code through Pygg, this launch only
pygg run --model auto/cheap claude
pygg run claude -p "Explain this repository"
```

Arguments after `claude` are Claude Code's own. Claude Code may say it does not recognize the model `auto` and assume a 200k context; that is expected.

Verified: a headless Claude Code session (`claude -p`) through the gateway answered every turn on 2026-11-11, including a sub-agent and `/compact`, with the session header on 12 of 12 requests. **Not verified yet:** the launcher's only live run (2026-11-12) ended at the gateway's 422 because no provider was connected, and an interactive terminal session has not been run.

### Codex CLI

Pygg adds a `pygg` provider and a profile to Codex's configuration, after a backup. Codex's default model is not changed; choose Pygg per run with the profile.

```powershell
pygg setup codex-models --dry-run
pygg setup codex-models
codex --profile pygg
codex --profile pygg -m auto/cheap
pygg setup codex-models --remove         # undo
```

Verified live on 2026-11-12: Codex through the Responses API, and the one-click set-up in CLI Tools.

### OpenCode

Pygg adds a provider named Pygg to `opencode.json`, after a backup, listing `auto`, `auto/cheap` and your enabled models, and keeps the list current when models change.

```powershell
pygg setup opencode-models
opencode
# then in OpenCode: /models → Pygg → auto
pygg setup opencode-models --remove      # undo
```

Verified live on 2026-11-11 with the generated snippet (`opencode run` completed; a combo fallback happened and the ledger said so). The one-click set-up calls the same code.

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

Use your tool as usual with the model `auto`. For each request Pygg builds the route from the models you connected, within the key's limits, and records what it cost. CLI Tools says **Working** under a tool once a request has come through it.

The model names you can use:

- `auto`: an eligible model at or above the key's minimum tier (T2 unless changed), local, free and subscription before paid, then the lowest estimated cost. It chooses within your rules; it does not judge how hard a prompt is.
- `auto/cheap`: the lowest estimated cost among the models that can take the request.
- `coding`, or any combo's name: your own ordered list, from **Combos**. The next model answers when one is out of quota or unavailable. Templates build a combo from rules (Free first, Use my plans first, Local only, and others); they never fill a gap with a paid model by themselves.
- `gemini/gemini-2.5-flash`, or any `connection/model`: one model by name.

Every response says why it chose what it chose. **How auto chooses**, in Combos, simulates a route without sending anything; so does `pygg gateway auto --alias auto --tools yes`.

Fallback happens only on errors, before any answer reaches the client. Pygg does not judge answers and does not retry a poor one.

## See cost and savings

**Usage** shows every request with its cost and that cost's source: **actual** (the provider reported usage and the model has a price), **estimated** (counted from tokens at the model's price), or **unknown**. Unknown is never shown as $0 and is left out of totals; give the model a price in Providers to estimate it. A subscription's use is counted as quota, not money.

**Estimated Savings** prices the same tokens at a model you choose and says what it left out. It is an estimate at list prices. It is unit-tested on a ledger built for the test and has not yet been checked against real usage.

Limits are per API key, in **Endpoint → API Keys → Limits**: a monthly limit (estimated or strict), a cap on requests in flight, a policy for models without a price, the connections the key may use, and a minimum tier for `auto`. **Policies** sets the defaults for new keys. A strict limit held every request of a 165-request benchmark.

## Command line

The installer puts `pygg-cli.exe` next to the app. From npm, the same program is called `pygg` (developer preview, Windows x64 only):

```powershell
npm install -g @pygg/cli@dev
pygg --version
pygg status                              # is Pygg serving, and does the gateway answer
pygg doctor                              # status, plus the store, the agents found and what is set up
pygg setup                               # the tools Pygg can set up, and their state
```

Without npm: `& "$env:LOCALAPPDATA\Pygg\pygg-cli.exe" doctor`. Keys are never taken on the command line: `pygg gateway add-connection` reads one from an environment variable (`--key-env`). `pygg <command> --help` lists every option.

## Troubleshooting

| You see | What to do |
| --- | --- |
| Connection refused, or "nothing is listening" | Start the Pygg app (it serves the gateway), or check with `pygg status`. The port is set in Endpoint → Configure. |
| HTTP 401 | The API key is wrong or revoked. Endpoint → API Keys → **Show** copies it again. Keys made before 2026-11-12 cannot be shown: create a new one. |
| HTTP 422 "No connected model is eligible for auto" | No model has every fact `auto` needs (tier, price, tools, context). Run `pygg gateway lookup`, set them in Providers → Auto…, lower the key's minimum tier, or name a model. |
| HTTP 429, or "cooling down" | A provider's rate limit or quota. `auto` and combos move on to the next model; Providers shows when it is tried again. |
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
- **What are Smart Tasks?** An experimental group where IDE agents hand subtasks to Pygg over MCP and agent CLIs run them in worktrees. They work but are on hold until the owner re-evaluates them; see [AGENTS.md](AGENTS.md).

## Where next

- [PROVIDERS.md](PROVIDERS.md): every preset, its endpoint, free tier and live status.
- [SECURITY.md](SECURITY.md): what Pygg protects, what you are trusted with, how to report a problem.
- [IMPLEMENTATION-STATUS.md](IMPLEMENTATION-STATUS.md): what is built and how each part was verified, including what is not verified yet.
- [docs/README.md](README.md): the full index.
