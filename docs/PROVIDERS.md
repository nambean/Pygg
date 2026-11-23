# Providers

> **Status:** Current · connection presets, which have answered live, and web sessions · reviewed 2026-11-20

Presets: `apps/desktop/src-tauri/src/gateway/presets.rs`; any field can be changed when connecting. Endpoints from each provider's docs.

**Live** = a provider answered a real request through the gateway.

## Cloud, API key

| Preset | Endpoint | Live |
| --- | --- | --- |
| Google Gemini | `https://generativelanguage.googleapis.com/v1beta/openai` | yes |
| Anthropic | `https://api.anthropic.com/v1` | yes |
| OpenRouter | `https://openrouter.ai/api/v1` | yes |
| OpenAI | `https://api.openai.com/v1` | no |
| Groq | `https://api.groq.com/openai/v1` | no |
| DeepSeek | `https://api.deepseek.com` | no |
| DeepSeek (Anthropic API) | `https://api.deepseek.com/anthropic/v1` | no |
| Qwen (Alibaba Model Studio) | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` | yes (token plan) |
| Mistral | `https://api.mistral.ai/v1` | no |
| Cerebras | `https://api.cerebras.ai/v1` | no |
| Together AI | `https://api.together.ai/v1` | no |
| Fireworks AI | `https://api.fireworks.ai/inference/v1` | no |
| xAI (Grok) | `https://api.x.ai/v1` | no |
| NVIDIA | `https://integrate.api.nvidia.com/v1` | no |
| SambaNova Cloud | `https://api.sambanova.ai/v1` | no |
| Hugging Face | `https://router.huggingface.co/v1` | no |
| DeepInfra | `https://api.deepinfra.com/v1/openai` | no |
| SiliconFlow | `https://api.siliconflow.com/v1` | no |
| Kimi (Moonshot) | `https://api.moonshot.ai/v1` | no |
| Kimi (Anthropic API) | `https://api.moonshot.ai/anthropic/v1` | no |
| Z.AI (GLM) | `https://api.z.ai/api/paas/v4` | no |
| Z.AI (Anthropic API) | `https://api.z.ai/api/anthropic/v1` | no |
| Cohere | `https://api.cohere.ai/compatibility/v1` | no |
| MiniMax | `https://api.minimax.io/v1` | no |
| MiniMax (Anthropic API) | `https://api.minimax.io/anthropic/v1` | no |
| Nebius Token Factory | `https://api.tokenfactory.nebius.com/v1` | no |
| Meta (Muse API) | `https://api.meta.ai/v1` | no |
| Custom (OpenAI- or Anthropic-compatible) | any | — |

- **Key ring** (ADR-038): keys of separate accounts on one connection; a limited key cools down, a rejected one is blocked. Not for web sessions or local runtimes.

## Local

| Preset | Endpoint | Live |
| --- | --- | --- |
| Ollama | `http://127.0.0.1:11434/v1` | yes; start with `OLLAMA_CONTEXT_LENGTH=32768`. Pygg reads each model's tools and served context from `/api/show` (a model's `num_ctx`, else 4,096: set the context with `pygg gateway model --model ollama/M --context 32768` after raising it); embedding models are not listed; local models are T0, so let auto use lower tiers |
| LM Studio | `http://localhost:1234/v1` | no |
| llama.cpp server | `http://127.0.0.1:9931/v1` | no |
| vLLM | `http://localhost:8000/v1` | no |
| Docker Model Runner | `http://localhost:12434/engines/v1` | no |
| Jan | `http://127.0.0.1:1337/v1` | no |

## Web sessions (unofficial)

An AI site with no API, through a Pygg window you sign in to yourself (ADR-035; desktop app only).

- **Off until you turn it on** and acknowledge the risk: a site can limit or ban an account for automated use.
- **Never chosen by `auto`**; no tools, no images; $0 or subscription quota.
- **One WebView2 profile per connection**; fingerprint never changed.
- **Driven windows keep to the recipe's sign-in hosts**; others are blocked.

| Preset | Site | Live |
| --- | --- | --- |
| `muse-web` | `https://muse.ai/thread/new` | yes (8 / 8) |
| `chatgpt-web` | `https://chatgpt.com/` | yes (15 / 16) |
| `perplexity-web` | `https://www.perplexity.ai/` | yes (8 / 8) |
| `deepseek-web` | `https://chat.deepseek.com/` | yes (8 / 8) |
| `grok-web` | `https://grok.com/` | yes (8 / 8) |
| `gemini-web` | `https://gemini.google.com/app` | yes (sign-in, text, image) |

Scores: [web sessions benchmark](../benchmark/reports/2026-11-13-web-sessions.md).

### The guard

Always on (`websession/guard.rs`); ceilings cannot be raised from the app.

| Rule | All sites | DeepSeek |
| --- | --- | --- |
| Longest prompt | 12,000 characters | 4,000 |
| Per hour | 15 requests | 6 |
| Per 10 minutes | 5 requests | 3 |
| Per day (UTC) | 50 (settable 1–100) | 30 |
| Rest after a reply | from 10 s, at most 3 min | from 30 s |
| Refused | 429 with Retry-After; nothing sent | same |
| Trouble sign | rest 5 min, doubling, up to 6 h | same |
| Suspended | connection turned off | same |

## Left out

- GitHub Models (retired), Perplexity's API (Sonar ended), Novita (unchecked).
- Cookie or OAuth bridges, sign-in-free scrapers (ADR-026).
