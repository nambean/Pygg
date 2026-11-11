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
}

const fn preset(id: &'static str, name: &'static str, endpoint: Option<&'static str>, needs_key: bool,
    default_billing: &'static str, wire: &'static str) -> Preset {
    Preset { id, name, endpoint, needs_key, default_billing, wire, strip_model_prefix: None, accepts_cache_control: false,
        family: None }
}

/// The default quota group for a connection of this preset kind.
pub fn quota_family(kind: &str) -> &str {
    find(kind).and_then(|p| p.family).unwrap_or(kind)
}

/// Endpoints are from each provider's own documentation, checked 2026-11-11; the source of each is in
/// docs/PROVIDERS.md. An Anthropic-format endpoint is given as the base an Anthropic SDK takes plus `/v1`,
/// since the gateway appends `/messages`.
pub const PRESETS: &[Preset] = &[
    Preset { strip_model_prefix: Some("models/"),
        ..preset("gemini", "Google Gemini", Some("https://generativelanguage.googleapis.com/v1beta/openai"), true, "unknown", "openai") },
    preset("anthropic", "Anthropic", Some("https://api.anthropic.com/v1"), true, "paid", "anthropic"),
    Preset { accepts_cache_control: true,
        ..preset("openrouter", "OpenRouter", Some("https://openrouter.ai/api/v1"), true, "paid", "openai") },
    preset("openai", "OpenAI", Some("https://api.openai.com/v1"), true, "paid", "openai"),
    preset("groq", "Groq", Some("https://api.groq.com/openai/v1"), true, "unknown", "openai"),
    Preset { family: Some("deepseek"), ..preset("deepseek", "DeepSeek", Some("https://api.deepseek.com"), true, "paid", "openai") },
    Preset { family: Some("deepseek"),
        ..preset("deepseek-anthropic", "DeepSeek (Anthropic API)", Some("https://api.deepseek.com/anthropic/v1"), true, "paid", "anthropic") },
    // Alibaba Cloud Model Studio's OpenAI-compatible mode (international). Plans with their own endpoint, such
    // as a token plan, set it when connecting.
    preset("qwen", "Qwen (Alibaba Model Studio)", Some("https://dashscope-intl.aliyuncs.com/compatible-mode/v1"), true, "unknown", "openai"),
    preset("mistral", "Mistral", Some("https://api.mistral.ai/v1"), true, "unknown", "openai"),
    preset("cerebras", "Cerebras", Some("https://api.cerebras.ai/v1"), true, "paid", "openai"),
    preset("together", "Together AI", Some("https://api.together.ai/v1"), true, "paid", "openai"),
    preset("fireworks", "Fireworks AI", Some("https://api.fireworks.ai/inference/v1"), true, "paid", "openai"),
    preset("xai", "xAI (Grok)", Some("https://api.x.ai/v1"), true, "paid", "openai"),
    preset("nvidia", "NVIDIA (build.nvidia.com)", Some("https://integrate.api.nvidia.com/v1"), true, "unknown", "openai"),
    preset("sambanova", "SambaNova Cloud", Some("https://api.sambanova.ai/v1"), true, "unknown", "openai"),
    preset("huggingface", "Hugging Face (Inference Providers)", Some("https://router.huggingface.co/v1"), true, "paid", "openai"),
    preset("deepinfra", "DeepInfra", Some("https://api.deepinfra.com/v1/openai"), true, "paid", "openai"),
    preset("siliconflow", "SiliconFlow", Some("https://api.siliconflow.com/v1"), true, "unknown", "openai"),
    Preset { family: Some("moonshot"), ..preset("moonshot", "Kimi (Moonshot)", Some("https://api.moonshot.ai/v1"), true, "paid", "openai") },
    Preset { family: Some("moonshot"),
        ..preset("moonshot-anthropic", "Kimi (Anthropic API)", Some("https://api.moonshot.ai/anthropic/v1"), true, "paid", "anthropic") },
    Preset { family: Some("zai"), ..preset("zai", "Z.AI (GLM)", Some("https://api.z.ai/api/paas/v4"), true, "unknown", "openai") },
    Preset { family: Some("zai"),
        ..preset("zai-anthropic", "Z.AI (Anthropic API)", Some("https://api.z.ai/api/anthropic/v1"), true, "unknown", "anthropic") },
    preset("cohere", "Cohere", Some("https://api.cohere.ai/compatibility/v1"), true, "unknown", "openai"),
    Preset { family: Some("minimax"), ..preset("minimax", "MiniMax", Some("https://api.minimax.io/v1"), true, "paid", "openai") },
    Preset { family: Some("minimax"),
        ..preset("minimax-anthropic", "MiniMax (Anthropic API)", Some("https://api.minimax.io/anthropic/v1"), true, "paid", "anthropic") },
    preset("nebius", "Nebius Token Factory", Some("https://api.tokenfactory.nebius.com/v1"), true, "paid", "openai"),
    preset("ollama", "Ollama", Some("http://127.0.0.1:11434/v1"), false, "local", "openai"),
    preset("lmstudio", "LM Studio", Some("http://localhost:1234/v1"), false, "local", "openai"),
    preset("llamacpp", "llama.cpp server", Some("http://127.0.0.1:9931/v1"), false, "local", "openai"),
    preset("vllm", "vLLM", Some("http://localhost:8000/v1"), false, "local", "openai"),
    preset("docker-model-runner", "Docker Model Runner", Some("http://localhost:12434/engines/v1"), false, "local", "openai"),
    preset("jan", "Jan", Some("http://127.0.0.1:1337/v1"), false, "local", "openai"),
    preset("custom", "Custom (OpenAI-compatible)", None, false, "unknown", "openai"),
    preset("custom-anthropic", "Custom (Anthropic-compatible)", None, false, "unknown", "anthropic"),
];

pub fn find(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}
