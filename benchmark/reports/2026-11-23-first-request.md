# From install to a first routed request, 2026-11-23

> **Status:** Record · run 2026-11-23 (UTC+7) by the maintainer · ROADMAP focus item 6, the install half · $0 (a local Ollama) · raw results: [`results/2026-11-23-first-request`](../results/2026-11-23-first-request/)

**In one line:** on the command line, a newcomer with only a local Ollama never reached a routed request by following Pygg's own messages and guide. After four fixes, the message given when the key is made leads to the first answer at 4.7 s, or about 9 s with the npm install.

## Method

[`scripts/proof-first-request.py`](../../scripts/proof-first-request.py) times the command-line path in a clean profile: its own home, AppData and npm prefix in a temporary folder, with a local Ollama as the only provider and the gateway on a spare port. It plays a newcomer who does what Pygg says:

1. Install `@pygg/cli`, add the Ollama connection, start the gateway, make a key.
2. Run every `pygg gateway …` command Pygg prints, when the key is made and in each error.
3. With nothing left to follow, run the guide's two commands (`refresh-models`, `lookup`).
4. Ask `auto` "Say ok." after each step.

```text
python -I scripts/proof-first-request.py OUT.jsonl [--exe pygg-cli.exe]   (--exe: a local build instead of npm)
```

## Results

| Build | First routed request | What happened |
| --- | --- | --- |
| npm `@pygg/cli@0.1.0-dev.8` | **never** | 422 "Set tiers … in Pygg → Providers" (the app's screen, not the CLI's); `refresh-models` and `lookup` changed nothing; 422 three times |
| This commit, local build | **4.7 s** (≈ 9 s with the npm install) | The key's hint lowered its minimum tier; the first request was answered by `ollama/qwen2.5-coder:7b` |
| npm `@pygg/cli@0.1.0-dev.9`, as published | **17.4 s**, from `npm install` (Ollama loading the model took about 10 s) | The same path on the release |

The four things that stood in the way, in the order they were met:

| Blocker | Fix |
| --- | --- |
| `add-connection` listed no model: the list waited for `refresh-models` | It lists the connection's models at once, as Providers does |
| Ollama's list has names only: no context, no tool support, so nothing was eligible | Pygg asks Ollama's `/api/show`: tools, vision, and the context Ollama serves. That is `num_ctx`, or its default of 4,096 (not the model's 40,960, since a longer prompt is cut short silently). A model that only embeds is left out |
| A local model is tier T0, a new key's floor is T2, and the 422 said only "set tiers in Providers" | The 422 names the next step in the app and on the command line. Making a key says so at once when no connected model meets its floor |
| Ollama's maximum output unknown, and the request set one | Ollama writes until its served context is full, so that is its maximum output |

- **Not covered:** the desktop app's path, which has the same blockers behind Providers' own screens, and a clean Windows machine. Above all, people: the ROADMAP's measure is 8 of 10 fresh users reaching a routed request in 15 minutes (with the study, [benchmark/study](../study/README.md)).
- **Ollama's 4,096-token default** is too small for a coding agent's prompt. To use a local model for that, raise it (`OLLAMA_CONTEXT_LENGTH`) and tell Pygg (`pygg gateway model --model ollama/M --context N`).
