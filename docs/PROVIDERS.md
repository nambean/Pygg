# Connection presets and their sources

> **Status:** Current · connection presets and their sources · reviewed 2026-11-11 · order: [ROADMAP](ROADMAP.md) · built: [Implementation Status](IMPLEMENTATION-STATUS.md)

Each preset in `apps/desktop/src-tauri/src/gateway/presets.rs` fills in a provider's endpoint, API format and default billing type. The user can change any of them when connecting, for example a token plan's or a regional endpoint. Every endpoint below comes from the provider's own documentation, checked on 2026-11-11. OmniRoute's provider reference was used only to decide which providers to look at (see [OMNIROUTE_FEATURE_RESEARCH.md](OMNIROUTE_FEATURE_RESEARCH.md#provider-reference-review-2026-11-11)).

"Live" means a real request went through the gateway (see [Implementation Status](IMPLEMENTATION-STATUS.md)). Everything else is configured from the docs and not yet tried; OpenRouter, OpenAI, Groq, DeepSeek and Mistral have only had their answer to an invalid key recorded, as error fixtures.

## Cloud, API key

| Preset | Endpoint | Format | `/models` lists | Free tier per the docs | Live | Source |
| --- | --- | --- | --- | --- | --- | --- |
| Google Gemini | `https://generativelanguage.googleapis.com/v1beta/openai` | OpenAI | ids only | yes (AI Studio) | yes | Google AI docs |
| Anthropic | `https://api.anthropic.com/v1` | Anthropic | input and output limits, image support | no | yes | Anthropic docs |
| OpenRouter | `https://openrouter.ai/api/v1` | OpenAI | context, output, tools, images, price | `:free` models | no | OpenRouter docs |
| OpenAI | `https://api.openai.com/v1` | OpenAI | ids | no | no | OpenAI docs |
| Groq | `https://api.groq.com/openai/v1` | OpenAI | context window | Free plan | no | https://console.groq.com/docs/openai |
| DeepSeek | `https://api.deepseek.com` | OpenAI | context, output | granted balance only | no | https://api-docs.deepseek.com/ |
| DeepSeek (Anthropic API) | `https://api.deepseek.com/anthropic/v1` | Anthropic | — | as above | no | https://api-docs.deepseek.com/ |
| Qwen (Alibaba Model Studio) | `https://dashscope-intl.aliyuncs.com/compatible-mode/v1` | OpenAI | ids | — | yes (token plan endpoint) | Alibaba Cloud docs |
| Mistral | `https://api.mistral.ai/v1` | OpenAI | context, tools, images | a free mode; the docs disagree on API access | no | https://docs.mistral.ai/api/endpoint/models |
| Cerebras | `https://api.cerebras.ai/v1` | OpenAI | ids | $5 trial credit with a payment method | no | https://inference-docs.cerebras.ai/resources/openai |
| Together AI | `https://api.together.ai/v1` | OpenAI | yes | not stated | no | https://docs.together.ai/docs/openai-api-compatibility |
| Fireworks AI | `https://api.fireworks.ai/inference/v1` | OpenAI | not documented here (add models by hand) | small credit, not confirmed | no | https://docs.fireworks.ai/tools-sdks/openai-compatibility |
| xAI (Grok) | `https://api.x.ai/v1` | OpenAI (labeled legacy; Responses preferred) | context | prepaid only | no | https://docs.x.ai/developers/rest-api-reference/inference/chat-completions |
| NVIDIA (build.nvidia.com) | `https://integrate.api.nvidia.com/v1` | OpenAI | not stated | Developer Program | no | https://docs.api.nvidia.com/nim/reference/llm-apis |
| SambaNova Cloud | `https://api.sambanova.ai/v1` | OpenAI | per guides | free tier without a payment method | no | https://docs.sambanova.ai/docs/en/get-started/api-keys-urls |
| Hugging Face (Inference Providers) | `https://router.huggingface.co/v1` | OpenAI | per-provider details | PRO credit only | no | https://huggingface.co/docs/inference-providers/index |
| DeepInfra | `https://api.deepinfra.com/v1/openai` | OpenAI | elsewhere (add models by hand) | not stated | no | https://docs.deepinfra.com/chat/overview |
| SiliconFlow | `https://api.siliconflow.com/v1` | OpenAI | ids | not stated | no | https://docs.siliconflow.com/en/api-reference/chat-completions/chat-completions |
| Kimi (Moonshot) | `https://api.moonshot.ai/v1` | OpenAI | context, images | not stated | no | https://platform.kimi.ai/docs/api/list-models.md |
| Kimi (Anthropic API) | `https://api.moonshot.ai/anthropic/v1` | Anthropic | — | as above | no | as above |
| Z.AI (GLM) | `https://api.z.ai/api/paas/v4` (Coding Plan: `…/api/coding/paas/v4`) | OpenAI | not stated | some free Flash models | no | https://docs.z.ai/devpack/tool/others.md |
| Z.AI (Anthropic API) | `https://api.z.ai/api/anthropic/v1` | Anthropic | — | as above | no | as above |
| Cohere | `https://api.cohere.ai/compatibility/v1` | OpenAI | elsewhere | trial key, 1,000 calls a month | no | https://docs.cohere.com/docs/compatibility-api |
| MiniMax | `https://api.minimax.io/v1` | OpenAI | not stated | not stated | no | https://platform.minimax.io/docs/api-reference/text-openai-api |
| MiniMax (Anthropic API) | `https://api.minimax.io/anthropic/v1` | Anthropic | — | as above | no | as above |
| Nebius Token Factory | `https://api.tokenfactory.nebius.com/v1` | OpenAI | context (verbose) | not stated | no | https://docs.tokenfactory.nebius.com/ |
| Meta (Muse API) | `https://api.meta.ai/v1` | OpenAI (also Anthropic Messages and Responses) | yes | pay as you go; no free tier stated | no | https://dev.meta.ai/docs/ (checked 2026-11-12) |

An Anthropic-format endpoint is written as the base an Anthropic SDK takes plus `/v1`, because the gateway appends `/messages`. Presets for one provider in both formats (DeepSeek, Kimi, Z.AI, MiniMax) share a quota group by default, since they take the same key.

## Local

| Preset | Default endpoint | Notes | Source |
| --- | --- | --- | --- |
| Ollama | `http://127.0.0.1:11434/v1` | **Live** (2026-11-11, Ollama 0.40.16 in Docker on an RTX 3060, `qwen3:8b`, through the Anthropic surface for OpenCode). Agents send prompts of 20k tokens or more: start Ollama with a larger context (`OLLAMA_CONTEXT_LENGTH=32768`), or the prompt is cut. `qwen2.5-coder:7b` did not produce tool calls OpenCode could use | Ollama docs |
| LM Studio | `http://localhost:1234/v1` | key only if "Require Authentication" is on | https://lmstudio.ai/docs/developer/openai-compat |
| llama.cpp server | `http://127.0.0.1:9931/v1` | the README's default port; Docker examples use 8080 | https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md |
| vLLM | `http://localhost:8000/v1` | optional `--api-key` | https://docs.vllm.ai/en/latest/serving/online_serving/openai_compatible_server/ |
| Docker Model Runner | `http://localhost:12434/engines/v1` | TCP access must be enabled | https://docs.docker.com/ai/model-runner/api-reference/ |
| Jan | `http://127.0.0.1:1337/v1` | optional key | https://docs.jan.ai/desktop/api-server |

## Providers outside the list

**Custom (OpenAI-compatible)** and **Custom (Anthropic-compatible)** take any endpoint the user enters, for a provider that has no preset. Billing starts as unknown and the user sets it; `/models` is read when the endpoint offers it, otherwise model names are added by hand. Headless, the key comes from an environment variable, for example:

```
gateway add-connection --preset custom --label "My provider" --endpoint https://api.example.com/v1 --key-env CUSTOM_API_KEY
```

## Web session (unofficial)

A site with no API, used through a Pygg window the user signs in to (ADR-035). Off until the user acknowledges the account-risk warning for that connection and signs in; billed as the user's plan with the site, so every cost is unknown; never chosen by `auto`; no tools or images.

| Preset | Site | Models | Recipe from | Live |
| --- | --- | --- | --- | --- |
| Muse (muse.ai) | `https://muse.ai`, a new chat at `/thread/new` | `muse` | 9router's `muse-web` executor (MIT), see THIRD_PARTY_NOTICES.md | no: written without a signed-in session; to be confirmed by the owner |

Meta's Muse *models* also have an API (Meta (Muse API), above); the muse.ai agent app has none.

## Left out

- **GitHub Models**: fully retired as of July 30, 2026, per GitHub's docs.
- **Perplexity**: Sonar chat completions support ended on September 27, 2026. The API now uses Responses-style endpoints, which the gateway does not speak.
- **Novita**: its docs do not make clear which base URL OpenAI clients should use. To be added once checked with a request.
- **Cookie bridges (pasted or imported cookies), OAuth bridges into other products' subscriptions, and no-auth scrapers**: see [OMNIROUTE_FEATURE_RESEARCH.md](OMNIROUTE_FEATURE_RESEARCH.md#provider-reference-review-2026-11-11). A site with no API can be connected as a web session instead (above, ADR-035), where the user signs in themselves and no cookie leaves its window.

## Reusing OmniRoute code

OmniRoute is MIT-licensed ("Copyright (c) 2026 diegosouzapw", checked at `release/v3.8.54`). The license allows reading, porting and reusing its code, provided the copyright and license notice is kept with any substantial portion. If code is ported, its origin goes in a `THIRD_PARTY_NOTICES.md` with that notice. None has been ported so far. The license covers OmniRoute's code. It does not change what each provider's own terms allow.
