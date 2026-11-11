# Connection presets and their sources

Each preset in `apps/desktop/src-tauri/src/gateway/presets.rs` fills in a provider's endpoint, API format and default billing type. The user can change any of them when connecting, for example a token plan's or a regional endpoint. Every endpoint below comes from the provider's own documentation, checked on 2026-11-11. OmniRoute's provider reference was used only to decide which providers to look at (see [OMNIROUTE_FEATURE_RESEARCH.md](OMNIROUTE_FEATURE_RESEARCH.md#provider-reference-review-2026-11-11)).

"Live" means a real request went through the gateway (see [Implementation Status](IMPLEMENTATION-STATUS.md)). Everything else is configured from the docs and not yet tried.

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

An Anthropic-format endpoint is written as the base an Anthropic SDK takes plus `/v1`, because the gateway appends `/messages`. Presets for one provider in both formats (DeepSeek, Kimi, Z.AI, MiniMax) share a quota group by default, since they take the same key.

## Local

| Preset | Default endpoint | Notes | Source |
| --- | --- | --- | --- |
| Ollama | `http://127.0.0.1:11434/v1` | | Ollama docs |
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

## Left out

- **GitHub Models**: fully retired as of July 30, 2026, per GitHub's docs.
- **Perplexity**: Sonar chat completions support ended on September 27, 2026. The API now uses Responses-style endpoints, which the gateway does not speak.
- **Novita**: its docs do not make clear which base URL OpenAI clients should use. To be added once checked with a request.
- **Web cookie and session bridges, OAuth bridges into other products' subscriptions, and no-auth scrapers**: see [OMNIROUTE_FEATURE_RESEARCH.md](OMNIROUTE_FEATURE_RESEARCH.md#provider-reference-review-2026-11-11).

## Reusing OmniRoute code

OmniRoute is MIT-licensed ("Copyright (c) 2026 diegosouzapw", checked at `release/v3.8.54`). The license allows reading, porting and reusing its code, provided the copyright and license notice is kept with any substantial portion. If code is ported, its origin goes in a `THIRD_PARTY_NOTICES.md` with that notice. None has been ported so far. The license covers OmniRoute's code. It does not change what each provider's own terms allow.
