//! Connection presets. A preset only fills in defaults; the user can change the endpoint and billing type of
//! any connection. `wire` is the API the provider speaks: requests in the same format are passed through
//! unchanged (ROADMAP P0-1), and other pairs are translated only where a contract test exists.

use serde::Serialize;

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    /// `None` for the Custom presets, where the user must give one. Includes the version path (`…/v1`).
    pub endpoint: Option<&'static str>,
    pub needs_key: bool,
    /// What a new connection is assumed to be until the user says otherwise. `unknown` where it depends on
    /// the account (a Gemini key can be free tier or paid).
    pub default_billing: &'static str,
    /// `openai` (Chat Completions) or `anthropic` (Messages).
    pub wire: &'static str,
    /// Some providers list models as `models/<id>` but take `<id>` in requests.
    #[serde(skip)]
    pub strip_model_prefix: Option<&'static str>,
    /// The provider accepts Anthropic-style `cache_control` markers on OpenAI-format content parts, so a
    /// translated Anthropic request can keep its prompt-cache hints (OpenRouter documents this).
    #[serde(skip)]
    pub accepts_cache_control: bool,
    /// The provider account a preset belongs to, when one provider has presets for both API formats (DeepSeek's
    /// OpenAI and Anthropic endpoints take the same key). Connections of one family share a quota group by
    /// default; otherwise the preset's own id is the group.
    #[serde(skip)]
    pub family: Option<&'static str>,
    /// What the provider's own docs say about getting started without paying, for grouping in Connect:
    /// `free` (an ongoing free tier or free models), `credits` (trial or granted credit), `paid`, `local`,
    /// `custom`, or `web` (a site with no API, used through a signed-in Pygg window: "Web session (unofficial)",
    /// ADR-035). A hint for choosing, not a promise about any account (docs/PROVIDERS.md).
    pub offer: &'static str,
    /// The free tier or credit in a few words, as the docs state it.
    pub offer_note: Option<&'static str>,
    /// What is free is a set of models listed at $0 (OpenRouter's `:free`, Z.AI's Flash), not the account's calls:
    /// on such a connection set as free, a model with a price is paid, and starts off (store.rs, ADR-032).
    pub free_models: bool,
    /// Takes the model's earlier reasoning back as `reasoning_content` on assistant messages (ROADMAP B5). DeepSeek
    /// requires it in thinking mode with tools ("must be fully passed back", else HTTP 400); Qwen reads it with
    /// `preserve_thinking` and ignores it otherwise; Kimi and GLM document keeping it. Other providers do not
    /// document the field, so it is not sent to them.
    #[serde(skip)]
    pub takes_reasoning: bool,
    /// Serves OpenAI's Responses API (`/responses`) itself, so a Responses request passes through unchanged instead
    /// of being translated to Chat Completions (ADR-033). Only OpenAI documents it as its own API; a provider
    /// with a partial or beta version is translated.
    #[serde(skip)]
    pub serves_responses: bool,
    /// A web session (ADR-035): the site is driven in the connection's own Pygg window, which Pygg serves as a Chat
    /// Completions endpoint on loopback (`websession`). `endpoint` is then the site, shown to the user, not called.
    pub web: bool,
    /// A web session whose site's assistant can act for the user (Muse): the warning says so.
    pub web_acts: bool,
}

const fn preset(
    id: &'static str, name: &'static str, endpoint: Option<&'static str>, needs_key: bool,
    default_billing: &'static str, wire: &'static str,
) -> Preset {
    let offer = match (endpoint, default_billing.as_bytes()) {
        (None, _) => "custom",
        (_, b"local") => "local",
        _ => "paid",
    };
    Preset {
        id,
        name,
        endpoint,
        needs_key,
        default_billing,
        wire,
        strip_model_prefix: None,
        accepts_cache_control: false,
        family: None,
        offer,
        offer_note: None,
        free_models: false,
        takes_reasoning: false,
        serves_responses: false,
        web: false,
        web_acts: false,
    }
}

/// A free tier made of $0 models next to paid ones: the connection starts as free, and only those models are on.
const fn free_models(p: Preset, note: &'static str) -> Preset {
    Preset { offer: "free", offer_note: Some(note), free_models: true, default_billing: "free", ..p }
}

const fn free(p: Preset, note: &'static str) -> Preset {
    Preset { offer: "free", offer_note: Some(note), ..p }
}

const fn credits(p: Preset, note: &'static str) -> Preset {
    Preset { offer: "credits", offer_note: Some(note), ..p }
}

/// A site with no API, used through a Pygg window the user signs in to (ADR-035). Billed as the user's plan with the
/// site, so the cost is unknown, never $0; `note` names an official API where the provider has one.
const fn web(id: &'static str, name: &'static str, site: &'static str, note: &'static str) -> Preset {
    Preset {
        offer: "web",
        offer_note: Some(note),
        web: true,
        ..preset(id, name, Some(site), false, "subscription", "openai")
    }
}

/// Whether a connection of this kind is a web session (ADR-035).
pub fn is_web(kind: &str) -> bool {
    find(kind).is_some_and(|p| p.web)
}

/// The default quota group for a connection of this preset kind.
pub fn quota_family(kind: &str) -> &str {
    find(kind).and_then(|p| p.family).unwrap_or(kind)
}

/// Endpoints are from each provider's own documentation, checked 2026-11-11; the source of each is in
/// docs/PROVIDERS.md. An Anthropic-format endpoint is given as the base an Anthropic SDK takes plus `/v1`,
/// since the gateway appends `/messages`.
pub const PRESETS: &[Preset] = &[
    free(
        Preset {
            strip_model_prefix: Some("models/"),
            ..preset(
                "gemini",
                "Google Gemini",
                Some("https://generativelanguage.googleapis.com/v1beta/openai"),
                true,
                "unknown",
                "openai",
            )
        },
        "Free tier in Google AI Studio",
    ),
    preset("anthropic", "Anthropic", Some("https://api.anthropic.com/v1"), true, "paid", "anthropic"),
    free_models(
        Preset {
            accepts_cache_control: true,
            ..preset("openrouter", "OpenRouter", Some("https://openrouter.ai/api/v1"), true, "paid", "openai")
        },
        "Free models (:free)",
    ),
    Preset {
        serves_responses: true,
        ..preset("openai", "OpenAI", Some("https://api.openai.com/v1"), true, "paid", "openai")
    },
    free(preset("groq", "Groq", Some("https://api.groq.com/openai/v1"), true, "unknown", "openai"), "Free plan"),
    credits(
        Preset {
            family: Some("deepseek"),
            takes_reasoning: true,
            ..preset("deepseek", "DeepSeek", Some("https://api.deepseek.com"), true, "paid", "openai")
        },
        "Granted balance",
    ),
    credits(
        Preset {
            family: Some("deepseek"),
            ..preset(
                "deepseek-anthropic",
                "DeepSeek (Anthropic API)",
                Some("https://api.deepseek.com/anthropic/v1"),
                true,
                "paid",
                "anthropic",
            )
        },
        "Granted balance",
    ),
    // Alibaba Cloud Model Studio's OpenAI-compatible mode (international). Plans with their own endpoint, such
    // as a token plan, set it when connecting.
    Preset {
        takes_reasoning: true,
        ..preset(
            "qwen",
            "Qwen (Alibaba Model Studio)",
            Some("https://dashscope-intl.aliyuncs.com/compatible-mode/v1"),
            true,
            "unknown",
            "openai",
        )
    },
    free(
        preset("mistral", "Mistral", Some("https://api.mistral.ai/v1"), true, "unknown", "openai"),
        "Free mode, lowest limits",
    ),
    credits(
        preset("cerebras", "Cerebras", Some("https://api.cerebras.ai/v1"), true, "paid", "openai"),
        "$5 trial credit, card needed",
    ),
    preset("together", "Together AI", Some("https://api.together.ai/v1"), true, "paid", "openai"),
    credits(
        preset("fireworks", "Fireworks AI", Some("https://api.fireworks.ai/inference/v1"), true, "paid", "openai"),
        "Small starting credit",
    ),
    preset("xai", "xAI (Grok)", Some("https://api.x.ai/v1"), true, "paid", "openai"),
    free(
        preset(
            "nvidia",
            "NVIDIA (build.nvidia.com)",
            Some("https://integrate.api.nvidia.com/v1"),
            true,
            "unknown",
            "openai",
        ),
        "Developer Program access",
    ),
    free(
        preset("sambanova", "SambaNova Cloud", Some("https://api.sambanova.ai/v1"), true, "unknown", "openai"),
        "Free tier without a card",
    ),
    credits(
        preset(
            "huggingface",
            "Hugging Face (Inference Providers)",
            Some("https://router.huggingface.co/v1"),
            true,
            "paid",
            "openai",
        ),
        "Monthly credit with PRO",
    ),
    preset("deepinfra", "DeepInfra", Some("https://api.deepinfra.com/v1/openai"), true, "paid", "openai"),
    preset("siliconflow", "SiliconFlow", Some("https://api.siliconflow.com/v1"), true, "unknown", "openai"),
    Preset {
        family: Some("moonshot"),
        takes_reasoning: true,
        ..preset("moonshot", "Kimi (Moonshot)", Some("https://api.moonshot.ai/v1"), true, "paid", "openai")
    },
    Preset {
        family: Some("moonshot"),
        ..preset(
            "moonshot-anthropic",
            "Kimi (Anthropic API)",
            Some("https://api.moonshot.ai/anthropic/v1"),
            true,
            "paid",
            "anthropic",
        )
    },
    free_models(
        Preset {
            family: Some("zai"),
            takes_reasoning: true,
            ..preset("zai", "Z.AI (GLM)", Some("https://api.z.ai/api/paas/v4"), true, "unknown", "openai")
        },
        "Some free Flash models",
    ),
    free_models(
        Preset {
            family: Some("zai"),
            ..preset(
                "zai-anthropic",
                "Z.AI (Anthropic API)",
                Some("https://api.z.ai/api/anthropic/v1"),
                true,
                "unknown",
                "anthropic",
            )
        },
        "Some free Flash models",
    ),
    free(
        preset("cohere", "Cohere", Some("https://api.cohere.ai/compatibility/v1"), true, "unknown", "openai"),
        "Trial key, 1,000 calls a month",
    ),
    Preset {
        family: Some("minimax"),
        ..preset("minimax", "MiniMax", Some("https://api.minimax.io/v1"), true, "paid", "openai")
    },
    Preset {
        family: Some("minimax"),
        ..preset(
            "minimax-anthropic",
            "MiniMax (Anthropic API)",
            Some("https://api.minimax.io/anthropic/v1"),
            true,
            "paid",
            "anthropic",
        )
    },
    // Meta's API for its Muse models (dev.meta.ai/docs, checked 2026-11-12): OpenAI and Anthropic formats, pay as you go.
    preset("meta", "Meta (Muse API)", Some("https://api.meta.ai/v1"), true, "paid", "openai"),
    preset("nebius", "Nebius Token Factory", Some("https://api.tokenfactory.nebius.com/v1"), true, "paid", "openai"),
    preset("ollama", "Ollama", Some("http://127.0.0.1:11434/v1"), false, "local", "openai"),
    preset("lmstudio", "LM Studio", Some("http://localhost:1234/v1"), false, "local", "openai"),
    preset("llamacpp", "llama.cpp server", Some("http://127.0.0.1:9931/v1"), false, "local", "openai"),
    preset("vllm", "vLLM", Some("http://localhost:8000/v1"), false, "local", "openai"),
    preset(
        "docker-model-runner",
        "Docker Model Runner",
        Some("http://localhost:12434/engines/v1"),
        false,
        "local",
        "openai",
    ),
    preset("jan", "Jan", Some("http://127.0.0.1:1337/v1"), false, "local", "openai"),
    preset("custom", "Custom (OpenAI-compatible)", None, false, "unknown", "openai"),
    preset("custom-anthropic", "Custom (Anthropic-compatible)", None, false, "unknown", "anthropic"),
    // Meta's personal agent app; its models also have an API (the `meta` preset above), the agent itself has none.
    Preset {
        web_acts: true,
        ..web("muse-web", "Muse (muse.ai)", "https://muse.ai", "The Muse models also have an API: Meta (Muse API)")
    },
    web("chatgpt-web", "ChatGPT (chatgpt.com)", "https://chatgpt.com", "OpenAI's models also have an API: OpenAI"),
    web(
        "perplexity-web",
        "Perplexity (perplexity.ai)",
        "https://www.perplexity.ai",
        "Answers with web search; its Sonar API is paid",
    ),
    web(
        "deepseek-web",
        "DeepSeek (chat.deepseek.com)",
        "https://chat.deepseek.com",
        "DeepSeek's models also have an API: DeepSeek",
    ),
    web(
        "grok-web",
        "Grok (grok.com)",
        "https://grok.com",
        "Behind Cloudflare checks; xAI's models also have an API: xAI (Grok)",
    ),
    web(
        "gemini-web",
        "Gemini (gemini.google.com)",
        "https://gemini.google.com",
        "Google sign-in only; Google's models also have an API: Google Gemini",
    ),
];

pub fn find(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_consistent() {
        let mut ids: Vec<&str> = PRESETS.iter().map(|p| p.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), PRESETS.len(), "ids are unique");
        for p in PRESETS {
            assert!(["free", "credits", "paid", "local", "custom", "web"].contains(&p.offer), "{}", p.id);
            assert_eq!(p.offer == "local", p.default_billing == "local", "{}", p.id);
            assert_eq!(p.offer == "custom", p.endpoint.is_none(), "{}", p.id);
            assert_eq!(p.offer_note.is_some(), matches!(p.offer, "free" | "credits" | "web"), "{}", p.id);
            // A web session (ADR-035) is billed as the user's plan with the site, has a recipe, and needs no key.
            assert_eq!(p.offer == "web", p.web, "{}", p.id);
            if p.web {
                assert_eq!(p.default_billing, "subscription", "{}", p.id);
                assert!(!p.needs_key && !p.serves_responses && p.wire == "openai", "{}", p.id);
                let recipe = crate::websession::recipe(p.id);
                assert!(recipe.is_some(), "{} has a recipe", p.id);
                assert_eq!(recipe.unwrap().acts, p.web_acts, "{} says whether its assistant acts", p.id);
            }
            assert!(matches!(p.wire, "openai" | "anthropic"));
            if let Some(family) = p.family {
                assert!(PRESETS.iter().filter(|q| q.family == Some(family)).count() == 2, "{family} pairs two formats");
            }
        }
    }
}
